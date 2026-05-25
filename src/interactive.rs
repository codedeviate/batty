use crate::cli::{Encoding, LineNumberStyle};
use crate::highlight::Highlighter;
use crate::input::{LineRange, decode};
use crate::printer::{PrinterConfig, StyleFlags, print};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Attribute, Print, ResetColor, SetAttribute},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, size,
    },
};
use std::io::{Write, stdout};

/// RAII guard that puts the terminal into the interactive state on construction
/// and reliably restores it on Drop, even if a panic unwinds through the loop.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort; don't propagate errors during unwind.
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Compute the viewport_top that keeps `cursor` visible within `body_rows`.
/// Pure function for testing.
pub fn scroll_viewport(
    cursor: usize,
    current_top: usize,
    body_rows: usize,
    total_lines: usize,
) -> usize {
    let body = body_rows.max(1);
    let mut top = current_top.max(1);
    if cursor < top {
        top = cursor;
    } else if cursor >= top + body {
        top = cursor.saturating_sub(body - 1);
    }
    // Don't scroll past EOF: keep at least one line visible.
    let max_top = total_lines.saturating_sub(body - 1).max(1);
    top.min(max_top)
}

#[allow(clippy::too_many_arguments)]
pub fn run<'a>(
    file_label: &str,
    contents: String,
    syntax: &'a syntect::parsing::SyntaxReference,
    syntax_set: &'a syntect::parsing::SyntaxSet,
    theme: &'a syntect::highlighting::Theme,
    line_numbers: LineNumberStyle,
    tabs: usize,
    show_all: bool,
    top_pad: u16,
    initial_markdown: bool,
    can_toggle_markdown: bool,
    initial_gutter_visible: bool,
    autoreload: Option<&Path>,
    _encoding: Encoding,
) -> Result<()> {
    let contents = contents;
    let total_lines = contents.lines().count().max(1);
    let mut cursor: usize = 1;
    let mut viewport_top: usize = 1;
    let mut markdown_view: bool = initial_markdown;
    // Independent scroll position for the rendered markdown view: counted in
    // *rendered* rows, not source lines, since termimad transforms structure.
    // Clamped to a valid range inside render_frame each frame.
    let mut markdown_scroll: usize = 0;
    // top_pad is live-adjustable via `+` / `-`. Some terminals (notably Warp)
    // overlay UI on the alt-screen's top rows, and the right padding can vary
    // tab-to-tab and after pane resizes — so let users tune it without exiting.
    let mut top_pad: u16 = top_pad;
    // Runtime gutter visibility. Initial value comes from --gutter / --no-gutter
    // (or the resolved config value); `n` flips it live.
    let mut gutter_visible: bool = initial_gutter_visible;
    // Cache of (rendered_row, source_line) tuples for the current markdown
    // render. Built when entering markdown view so m-toggles can preserve
    // scroll position between raw and rendered views. Cleared on resize so
    // we re-render at the new width.
    let mut markdown_map: Option<Vec<(usize, usize)>> = None;
    let mut last_term_w: usize = 0;

    let mut _watch = autoreload.map(WatchState::seed).transpose()?;
    let mut _reload_flash: Option<std::time::Instant> = None;

    let _guard = TerminalGuard::enter()?;

    loop {
        let (term_w, term_h) = size().unwrap_or((80, 24));
        let term_w = term_w as usize;
        let term_h = term_h as usize;
        // If the terminal width changed, invalidate the cached markdown map
        // so it re-builds at the new width on next entry / use.
        if term_w != last_term_w {
            markdown_map = None;
            last_term_w = term_w;
        }
        // Reserve last row for the status bar, and `top_pad` rows at the top
        // (e.g., for Warp's overlay).
        let body_rows = term_h
            .saturating_sub(1 + top_pad as usize)
            .max(1);
        viewport_top = scroll_viewport(cursor, viewport_top, body_rows, total_lines);
        let viewport_bot = (viewport_top + body_rows - 1).min(total_lines);

        render_frame(
            file_label,
            &contents,
            syntax,
            syntax_set,
            theme,
            line_numbers,
            tabs,
            show_all,
            cursor,
            viewport_top,
            viewport_bot,
            term_w,
            term_h,
            total_lines,
            top_pad,
            markdown_view,
            &mut markdown_scroll,
            gutter_visible,
            &mut markdown_map,
        )?;

        match event::read()? {
            Event::Key(KeyEvent { code, modifiers, .. }) => {
                if matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
                if matches!(code, KeyCode::Char('c')) && modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                match code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if markdown_view {
                            markdown_scroll = markdown_scroll.saturating_add(1);
                        } else if cursor < total_lines {
                            cursor += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if markdown_view {
                            markdown_scroll = markdown_scroll.saturating_sub(1);
                        } else if cursor > 1 {
                            cursor -= 1;
                        }
                    }
                    KeyCode::Char('g') | KeyCode::Home => {
                        if markdown_view {
                            markdown_scroll = 0;
                        } else {
                            cursor = 1;
                        }
                    }
                    KeyCode::Char('G') | KeyCode::End => {
                        if markdown_view {
                            markdown_scroll = usize::MAX; // clamped in render_frame
                        } else {
                            cursor = total_lines;
                        }
                    }
                    KeyCode::PageDown => {
                        if markdown_view {
                            markdown_scroll = markdown_scroll.saturating_add(body_rows);
                        } else {
                            cursor = (cursor + body_rows).min(total_lines);
                        }
                    }
                    KeyCode::PageUp => {
                        if markdown_view {
                            markdown_scroll = markdown_scroll.saturating_sub(body_rows);
                        } else {
                            cursor = cursor.saturating_sub(body_rows).max(1);
                        }
                    }
                    KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                        if markdown_view {
                            markdown_scroll = markdown_scroll.saturating_add(body_rows / 2);
                        } else {
                            cursor = (cursor + body_rows / 2).min(total_lines);
                        }
                    }
                    KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                        if markdown_view {
                            markdown_scroll = markdown_scroll.saturating_sub(body_rows / 2);
                        } else {
                            cursor = cursor.saturating_sub(body_rows / 2).max(1);
                        }
                    }
                    KeyCode::Char('m') => {
                        // Allow toggling either when the file is markdown-detected
                        // OR we're already in markdown view (so the user can flip
                        // back even if they forced --markdown on a non-md file).
                        if can_toggle_markdown || markdown_view {
                            if markdown_view {
                                // Going markdown → raw: pull the source line from
                                // the current rendered scroll position.
                                if let Some(map) = markdown_map.as_ref() {
                                    let src = crate::markdown::source_line_for_rendered(
                                        map,
                                        markdown_scroll,
                                    );
                                    cursor = src.clamp(1, total_lines);
                                }
                                markdown_view = false;
                            } else {
                                // Going raw → markdown: build (or reuse) the map,
                                // scroll to the rendered row of the block at the
                                // current source-line cursor.
                                if markdown_map.is_none() {
                                    let r = crate::markdown::render_with_map(
                                        &contents,
                                        last_term_w.max(20),
                                    );
                                    markdown_map = Some(r.map);
                                }
                                if let Some(map) = markdown_map.as_ref() {
                                    markdown_scroll = crate::markdown::rendered_row_for_source(
                                        map, cursor,
                                    );
                                } else {
                                    markdown_scroll = 0;
                                }
                                markdown_view = true;
                            }
                        }
                    }
                    KeyCode::Char('n') => {
                        // Toggle the gutter (line numbers + cursor glyph) live.
                        gutter_visible = !gutter_visible;
                    }
                    // Live top-pad adjustment for terminals that overlay UI on
                    // the alt-screen's top rows (e.g. Warp). `+` / `=` grow
                    // the pad by 1 row; `-` shrinks (saturating at 0).
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        top_pad = top_pad.saturating_add(1);
                    }
                    KeyCode::Char('-') => {
                        top_pad = top_pad.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            Event::Resize(_, _) => {
                // Loop will recompute body_rows and re-render.
            }
            _ => {}
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_frame(
    file_label: &str,
    contents: &str,
    syntax: &syntect::parsing::SyntaxReference,
    syntax_set: &syntect::parsing::SyntaxSet,
    theme: &syntect::highlighting::Theme,
    line_numbers: LineNumberStyle,
    tabs: usize,
    show_all: bool,
    cursor: usize,
    viewport_top: usize,
    viewport_bot: usize,
    term_w: usize,
    term_h: usize,
    total_lines: usize,
    top_pad: u16,
    markdown_view: bool,
    markdown_scroll: &mut usize,
    gutter_visible: bool,
    markdown_map: &mut Option<Vec<(usize, usize)>>,
) -> Result<()> {
    let body_rows = term_h
        .saturating_sub(1 + top_pad as usize)
        .max(1);

    // Build the body buffer + position label.
    // - Markdown view renders the whole document via termimad (with source-line
    //   map), then slices a visible window of *rendered rows* using
    //   markdown_scroll.
    // - Raw view goes through the standard printer with line_range applied.
    let (body_bytes, position_label): (Vec<u8>, String) = if markdown_view {
        let line_no_width = total_lines.to_string().len().max(4);
        let rendered = crate::markdown::render_with_gutter(
            contents,
            term_w,
            line_no_width,
            gutter_visible, // numbers track the n-key toggle
            false,          // no grid in interactive mode
            true,           // interactive is always color
        );
        // Cache the map for m-toggle scroll preservation.
        *markdown_map = Some(rendered.map.clone());
        let rows: Vec<&str> = rendered.text.split('\n').collect();
        let total = rows.len().max(1);
        let max_scroll = total.saturating_sub(body_rows);
        if *markdown_scroll > max_scroll {
            *markdown_scroll = max_scroll;
        }
        let end = (*markdown_scroll + body_rows).min(total);
        let visible = &rows[*markdown_scroll..end];
        let body = visible.join("\n");
        let src_line = crate::markdown::source_line_for_rendered(&rendered.map, *markdown_scroll);
        let label = format!(
            "rendered {}/{} ↔ src {}",
            *markdown_scroll + 1,
            total,
            src_line
        );
        (body.into_bytes(), label)
    } else {
        let mut highlighter = Highlighter::new(syntax, theme, syntax_set);
        let mut highlight_lines = std::collections::HashSet::new();
        highlight_lines.insert(cursor);
        let style = StyleFlags {
            header: false,
            grid: false,
            // Gutter visibility: numbers shown only when the user wants the
            // gutter (the `n` key flips this live). Markdown is handled
            // via the early branch above.
            numbers: gutter_visible,
            rule: false,
            changes: false,
            snip: false,
        };
        let cfg = PrinterConfig {
            style,
            line_range: Some(LineRange {
                start: viewport_top,
                end: viewport_bot,
            }),
            highlight_lines,
            tabs,
            // Interactive mode forces wrap=Never. Wrapping in raw-mode
            // alt-screen would let one source line span multiple visual
            // rows, breaking the viewport math (cursor / scroll / status
            // bar). Long lines truncate at the terminal edge; documented.
            wrap: crate::cli::WrapMode::Never,
            show_all,
            use_color: true,
            width: term_w,
            language_name: &syntax.name,
            // Cursor glyph (▶) lives in the gutter — hide it alongside line
            // numbers when the gutter is toggled off.
            cursor: if gutter_visible { Some(cursor) } else { None },
            line_numbers,
            markdown: false,
        };
        let mut buf: Vec<u8> = Vec::with_capacity(term_w * term_h);
        let stub_input = crate::input::InputKind::Stdin;
        print(&mut buf, &stub_input, contents, &mut highlighter, &cfg)?;
        (buf, format!("line {}/{}", cursor, total_lines))
    };

    // Status bar — shows position, mode tag, current top-pad (when nonzero),
    // and key hints.
    let mode_tag = if markdown_view { "  [md]" } else { "" };
    let gutter_tag = if !gutter_visible { "  no-gutter" } else { "" };
    let pad_tag = if top_pad > 0 {
        format!("  pad={}", top_pad)
    } else {
        String::new()
    };
    let status_label = format!(
        "  {}  {}  ({}){}{}{}  j/k g/G ^d/^u m n +/- q",
        file_label,
        position_label,
        match line_numbers {
            LineNumberStyle::Absolute => "abs",
            LineNumberStyle::Relative => "rel",
        },
        mode_tag,
        gutter_tag,
        pad_tag,
    );
    let status_truncated: String = status_label.chars().take(term_w).collect();
    let pad = term_w.saturating_sub(status_truncated.chars().count());

    // Atomic-ish write: clear screen, move to (0,top_pad), write body, then status bar.
    // In raw mode, '\n' only moves the cursor down without returning to column 0,
    // which causes a staircase. Translate '\n' → '\r\n' so each line starts fresh.
    let mut crlf_buf: Vec<u8> = Vec::with_capacity(body_bytes.len() + 64);
    for &b in &body_bytes {
        if b == b'\n' {
            crlf_buf.push(b'\r');
        }
        crlf_buf.push(b);
    }
    let mut out = stdout().lock();
    execute!(out, Clear(ClearType::All), MoveTo(0, top_pad))?;
    out.write_all(&crlf_buf)?;
    execute!(
        out,
        MoveTo(0, term_h.saturating_sub(1) as u16),
        SetAttribute(Attribute::Reverse),
        Print(&status_truncated),
        Print(" ".repeat(pad)),
        SetAttribute(Attribute::Reset),
        ResetColor,
    )?;
    out.flush()?;
    Ok(())
}

use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Live-mode file watcher state. Holds enough of the last-seen file to
/// (a) cheaply gate on mtime+len and (b) confirm content actually changed
/// via byte comparison when the gate trips.
pub(crate) struct WatchState {
    mtime: Option<SystemTime>,
    len: u64,
    bytes: Vec<u8>,
}

pub(crate) enum WatchTick {
    /// File didn't change (or is briefly unreadable).
    Unchanged,
    /// mtime advanced but bytes are identical (e.g. `touch`, vim `:w` on
    /// an unchanged buffer). Caller should NOT redraw.
    MetadataOnly,
    /// File contents changed; here are the freshly decoded contents.
    Reloaded(String),
}

impl WatchState {
    /// Stat + read the file to capture an initial baseline. main::run
    /// already read once for the initial render, but threading the raw
    /// bytes through `interactive::run`'s signature is more invasive than
    /// one extra startup read.
    pub(crate) fn seed(path: &Path) -> anyhow::Result<Self> {
        let meta = fs::metadata(path)?;
        let bytes = fs::read(path)?;
        Ok(Self {
            mtime: meta.modified().ok(),
            len: meta.len(),
            bytes,
        })
    }

    /// Check the file. Cheap mtime+len fast path; on suspected change,
    /// re-read and byte-compare to suppress no-op metadata bumps.
    pub(crate) fn poll(&mut self, path: &Path, encoding: Encoding) -> WatchTick {
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return WatchTick::Unchanged,
        };
        let new_mtime = meta.modified().ok();
        let new_len = meta.len();
        if new_mtime == self.mtime && new_len == self.len {
            return WatchTick::Unchanged;
        }
        let new_bytes = match fs::read(path) {
            Ok(b) => b,
            Err(_) => return WatchTick::Unchanged,
        };
        if new_bytes == self.bytes {
            // mtime/len drifted but contents identical — silence future
            // ticks for the same state.
            self.mtime = new_mtime;
            self.len = new_len;
            return WatchTick::MetadataOnly;
        }
        let decoded = match decode(&new_bytes, encoding) {
            Ok(s) => s,
            Err(_) => return WatchTick::Unchanged,
        };
        self.mtime = new_mtime;
        self.len = new_len;
        self.bytes = new_bytes;
        WatchTick::Reloaded(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_keeps_cursor_above_top() {
        // Cursor 5, viewport top 10 -> top scrolls up to 5.
        assert_eq!(scroll_viewport(5, 10, 20, 100), 5);
    }

    #[test]
    fn scroll_keeps_cursor_below_bottom() {
        // body_rows=10, cursor=15 with top=1 means visible 1..10; cursor 15 not visible.
        // Top should become 15 - 10 + 1 = 6.
        assert_eq!(scroll_viewport(15, 1, 10, 100), 6);
    }

    #[test]
    fn scroll_clamps_top_at_eof() {
        // total=20, body_rows=10. max_top = 20-9 = 11.
        // cursor=20, current_top=15: top would compute to 20-9=11 which is ok.
        assert_eq!(scroll_viewport(20, 15, 10, 20), 11);
    }

    #[test]
    fn scroll_when_file_smaller_than_viewport() {
        // total=3, body_rows=10. max_top = 3-9 = saturated to 0 → max(1) = 1.
        assert_eq!(scroll_viewport(2, 1, 10, 3), 1);
    }

    #[test]
    fn scroll_no_change_when_cursor_in_range() {
        // cursor=5, top=1, body=10. Visible 1..10. cursor in range, top unchanged.
        assert_eq!(scroll_viewport(5, 1, 10, 100), 1);
    }

    use std::io::Write as _;
    use std::thread::sleep;
    use std::time::Duration;

    fn write_file(path: &std::path::Path, contents: &[u8]) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(contents).unwrap();
        f.sync_all().unwrap();
    }

    #[test]
    fn watch_first_poll_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        write_file(&p, b"hello\n");
        let mut w = WatchState::seed(&p).unwrap();
        assert!(matches!(w.poll(&p, Encoding::Auto), WatchTick::Unchanged));
    }

    #[test]
    fn watch_mtime_bump_only_returns_metadata_only() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        write_file(&p, b"hello\n");
        let mut w = WatchState::seed(&p).unwrap();
        // Force mtime to advance without changing content.
        sleep(Duration::from_millis(50));
        write_file(&p, b"hello\n");
        match w.poll(&p, Encoding::Auto) {
            WatchTick::MetadataOnly => {}
            WatchTick::Unchanged => panic!("expected MetadataOnly, got Unchanged"),
            WatchTick::Reloaded(_) => panic!("expected MetadataOnly, got Reloaded"),
        }
    }

    #[test]
    fn watch_append_returns_reloaded() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        write_file(&p, b"hello\n");
        let mut w = WatchState::seed(&p).unwrap();
        sleep(Duration::from_millis(50));
        write_file(&p, b"hello\nworld\n");
        match w.poll(&p, Encoding::Auto) {
            WatchTick::Reloaded(s) => assert_eq!(s, "hello\nworld\n"),
            _ => panic!("expected Reloaded"),
        }
    }

    #[test]
    fn watch_truncate_returns_reloaded() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        write_file(&p, b"hello\nworld\n");
        let mut w = WatchState::seed(&p).unwrap();
        sleep(Duration::from_millis(50));
        write_file(&p, b"hi\n");
        match w.poll(&p, Encoding::Auto) {
            WatchTick::Reloaded(s) => assert_eq!(s, "hi\n"),
            _ => panic!("expected Reloaded"),
        }
    }

    #[test]
    fn watch_missing_file_returns_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        write_file(&p, b"hello\n");
        let mut w = WatchState::seed(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(matches!(w.poll(&p, Encoding::Auto), WatchTick::Unchanged));
    }
}
