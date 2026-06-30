use crate::cli::{Encoding, LineNumberStyle, WrapMode};
use crate::highlight::Highlighter;
use crate::input::{LineRange, decode};
use crate::printer::{PrinterConfig, StyleFlags, print};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Attribute, Print, ResetColor, SetAttribute},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, size,
    },
};
use std::fs;
use std::io::{Write, stdout};
use std::path::Path;
use std::time::SystemTime;

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

/// Visual-row-aware version of `scroll_viewport` for when wrapping is on.
/// `viewport_top` stays a source line, but the window holds `body_rows`
/// *visual* rows, so we advance the top until the cursor's source line fits.
/// `rows_of(line)` returns the visual-row count for a 1-based source line.
/// Because the top is never advanced past the cursor, it cannot over-scroll
/// past EOF — no separate EOF clamp is needed.
pub fn scroll_viewport_wrapped(
    cursor: usize,
    current_top: usize,
    body_rows: usize,
    rows_of: impl Fn(usize) -> usize,
) -> usize {
    let body = body_rows.max(1);
    let cursor = cursor.max(1);
    let mut top = current_top.max(1);
    if cursor < top {
        top = cursor;
    }
    while top < cursor {
        let rows: usize = (top..=cursor).map(|l| rows_of(l)).sum();
        if rows <= body {
            break;
        }
        top += 1;
    }
    top
}

/// Move the cursor by roughly `budget` visual rows (one screen, or a half for
/// Ctrl-d/u) when wrapping is on, so paging advances about one screenful even
/// though source lines span several rows. Walks source lines accumulating
/// `rows_of` until the budget is met, clamped to `[1, total_lines]`.
pub fn step_by_rows(
    cursor: usize,
    budget: usize,
    total_lines: usize,
    down: bool,
    rows_of: impl Fn(usize) -> usize,
) -> usize {
    let total = total_lines.max(1);
    let budget = budget.max(1);
    let mut acc = 0usize;
    let mut c = cursor.clamp(1, total);
    loop {
        acc += rows_of(c);
        if acc >= budget {
            break;
        }
        if down {
            if c >= total {
                break;
            }
            c += 1;
        } else {
            if c <= 1 {
                break;
            }
            c -= 1;
        }
    }
    c
}

/// Visual-row count for a 1-based source line at the current body width.
/// Pulls the line out of `contents` (cheap for viewport-sized ranges) and
/// defers to the printer's measurement so it matches the real render.
fn rows_of_line(
    contents: &str,
    line: usize,
    body_width: usize,
    tabs: usize,
    show_all: bool,
    mode: WrapMode,
) -> usize {
    let text = contents.lines().nth(line.saturating_sub(1)).unwrap_or("");
    crate::printer::visual_row_count(text, body_width, tabs, show_all, mode)
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
    initial_pretty: bool,
    can_toggle_pretty: bool,
    initial_gutter_visible: bool,
    autoreload: Option<&Path>,
    encoding: Encoding,
    wrap: WrapMode,
) -> Result<()> {
    let mut contents = contents;
    let mut total_lines = contents.lines().count().max(1);
    let mut cursor: usize = 1;
    let mut viewport_top: usize = 1;
    let mut markdown_view: bool = initial_markdown;
    // Pretty (JSONL) view: a transformed view parallel to markdown. The two are
    // mutually exclusive; if both were forced, markdown wins at startup.
    let mut pretty_view: bool = initial_pretty && !initial_markdown;
    // Independent scroll position for the current transformed view (markdown
    // or pretty): counted in *rendered* rows, not source lines, since the
    // transform reshapes structure. Clamped to a valid range inside
    // render_frame each frame.
    let mut view_scroll: usize = 0;
    // top_pad is live-adjustable via `+` / `-`. Some terminals (notably Warp)
    // overlay UI on the alt-screen's top rows, and the right padding can vary
    // tab-to-tab and after pane resizes — so let users tune it without exiting.
    let mut top_pad: u16 = top_pad;
    // Runtime gutter visibility. Initial value comes from --gutter / --no-gutter
    // (or the resolved config value); `n` flips it live.
    let mut gutter_visible: bool = initial_gutter_visible;
    // Soft-wrap state. Initial on/off keys off the *raw* --wrap value (not the
    // TTY-resolved one): character/word start on, auto/never start off. The
    // `w` key flips it live. wrap_mode is fixed for the session — word only
    // when the user passed --wrap=word, otherwise character.
    let mut wrap_on: bool = matches!(wrap, WrapMode::Character | WrapMode::Word);
    let wrap_mode: WrapMode = if matches!(wrap, WrapMode::Word) {
        WrapMode::Word
    } else {
        WrapMode::Character
    };
    // Cache of (rendered_row, source_line) tuples for the current transformed
    // view (markdown or pretty). Built when entering that view so toggles can
    // preserve scroll position between raw and rendered views. Cleared on
    // resize so we re-render at the new width.
    let mut view_map: Option<Vec<(usize, usize)>> = None;
    let mut last_term_w: usize = 0;

    let mut watch = autoreload.map(WatchState::seed).transpose()?;
    let mut reload_flash: Option<std::time::Instant> = None;
    // Redraw only when state changed. Re-rendering on every 200ms idle tick
    // (the previous behavior) caused visible flicker because each frame does
    // a full Clear+redraw. Set this true after any key/resize/reload, or when
    // the "[live · reloaded]" flash expires and the status bar needs to drop
    // the tag.
    let mut needs_render = true;

    let _guard = TerminalGuard::enter()?;

    loop {
        let (term_w, term_h) = size().unwrap_or((80, 24));
        let term_w = term_w as usize;
        let term_h = term_h as usize;
        // If the terminal width changed, invalidate the cached view map
        // (markdown or pretty) so it re-builds at the new width on next
        // entry / use.
        if term_w != last_term_w {
            view_map = None;
            last_term_w = term_w;
            needs_render = true;
        }
        // Reserve last row for the status bar, and `top_pad` rows at the top
        // (e.g., for Warp's overlay).
        let body_rows = term_h
            .saturating_sub(1 + top_pad as usize)
            .max(1);
        // Gutter / body-width geometry, mirrored from the printer so wrap
        // measurement matches the real render. Interactive raw view shows a
        // line-number cell + cursor-glyph cell when the gutter is visible.
        let line_no_width = total_lines.to_string().len().max(4);
        let gutter_w = if gutter_visible { line_no_width + 1 + 2 } else { 0 };
        let body_width = term_w.saturating_sub(gutter_w);
        viewport_top = if wrap_on && !markdown_view && !pretty_view {
            scroll_viewport_wrapped(cursor, viewport_top, body_rows, |l| {
                rows_of_line(&contents, l, body_width, tabs, show_all, wrap_mode)
            })
        } else {
            scroll_viewport(cursor, viewport_top, body_rows, total_lines)
        };
        let viewport_bot = (viewport_top + body_rows - 1).min(total_lines);

        if needs_render {
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
                pretty_view,
                &mut view_scroll,
                gutter_visible,
                &mut view_map,
                autoreload.is_some(),
                reload_flash,
                wrap_on,
                wrap_mode,
            )?;
            needs_render = false;
        }

        if event::poll(std::time::Duration::from_millis(200))? {
            // Any event we handle below is a state change; the catch-all
            // sets this back to false for events we ignore.
            needs_render = true;
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
                            if markdown_view || pretty_view {
                                view_scroll = view_scroll.saturating_add(1);
                            } else if cursor < total_lines {
                                cursor += 1;
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if markdown_view || pretty_view {
                                view_scroll = view_scroll.saturating_sub(1);
                            } else if cursor > 1 {
                                cursor -= 1;
                            }
                        }
                        KeyCode::Char('g') | KeyCode::Home => {
                            if markdown_view || pretty_view {
                                view_scroll = 0;
                            } else {
                                cursor = 1;
                            }
                        }
                        KeyCode::Char('G') | KeyCode::End => {
                            if markdown_view || pretty_view {
                                view_scroll = usize::MAX; // clamped in render_frame
                            } else {
                                cursor = total_lines;
                            }
                        }
                        KeyCode::PageDown => {
                            if markdown_view || pretty_view {
                                view_scroll = view_scroll.saturating_add(body_rows);
                            } else if wrap_on {
                                cursor = step_by_rows(cursor, body_rows, total_lines, true, |l| {
                                    rows_of_line(&contents, l, body_width, tabs, show_all, wrap_mode)
                                });
                            } else {
                                cursor = (cursor + body_rows).min(total_lines);
                            }
                        }
                        KeyCode::PageUp => {
                            if markdown_view || pretty_view {
                                view_scroll = view_scroll.saturating_sub(body_rows);
                            } else if wrap_on {
                                cursor = step_by_rows(cursor, body_rows, total_lines, false, |l| {
                                    rows_of_line(&contents, l, body_width, tabs, show_all, wrap_mode)
                                });
                            } else {
                                cursor = cursor.saturating_sub(body_rows).max(1);
                            }
                        }
                        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                            if markdown_view || pretty_view {
                                view_scroll = view_scroll.saturating_add(body_rows / 2);
                            } else if wrap_on {
                                cursor = step_by_rows(cursor, body_rows / 2, total_lines, true, |l| {
                                    rows_of_line(&contents, l, body_width, tabs, show_all, wrap_mode)
                                });
                            } else {
                                cursor = (cursor + body_rows / 2).min(total_lines);
                            }
                        }
                        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                            if markdown_view || pretty_view {
                                view_scroll = view_scroll.saturating_sub(body_rows / 2);
                            } else if wrap_on {
                                cursor = step_by_rows(cursor, body_rows / 2, total_lines, false, |l| {
                                    rows_of_line(&contents, l, body_width, tabs, show_all, wrap_mode)
                                });
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
                                    if let Some(map) = view_map.as_ref() {
                                        let src = crate::markdown::source_line_for_rendered(
                                            map,
                                            view_scroll,
                                        );
                                        cursor = src.clamp(1, total_lines);
                                    }
                                    markdown_view = false;
                                } else {
                                    // Going raw → markdown: build (or reuse) the map,
                                    // scroll to the rendered row of the block at the
                                    // current source-line cursor.
                                    pretty_view = false;
                                    view_map = None; // ensure a markdown map, not a stale JSON map
                                    if view_map.is_none() {
                                        let r = crate::markdown::render_with_map(
                                            &contents,
                                            last_term_w.max(20),
                                        );
                                        view_map = Some(r.map);
                                    }
                                    if let Some(map) = view_map.as_ref() {
                                        view_scroll = crate::markdown::rendered_row_for_source(
                                            map, cursor,
                                        );
                                    } else {
                                        view_scroll = 0;
                                    }
                                    markdown_view = true;
                                }
                            }
                        }
                        KeyCode::Char('n') => {
                            // Toggle the gutter (line numbers + cursor glyph) live.
                            gutter_visible = !gutter_visible;
                        }
                        KeyCode::Char('w') => {
                            // Toggle soft-wrap (raw view only; markdown already wraps).
                            if !markdown_view {
                                wrap_on = !wrap_on;
                            }
                        }
                        KeyCode::Char('p') => {
                            // Toggle the prettified JSONL view. Mirrors `m`:
                            // preserve scroll position via the source map, and
                            // exit markdown view if it was somehow active.
                            if can_toggle_pretty || pretty_view {
                                if pretty_view {
                                    if let Some(map) = view_map.as_ref() {
                                        let src = crate::markdown::source_line_for_rendered(
                                            map, view_scroll,
                                        );
                                        cursor = src.clamp(1, total_lines);
                                    }
                                    pretty_view = false;
                                } else {
                                    markdown_view = false;
                                    // Build a JSON map now so we can scroll to the
                                    // object at the current cursor line.
                                    let mut hl = Highlighter::new(syntax, theme, syntax_set);
                                    let r = crate::json::render_with_map(
                                        &contents, &mut hl, true,
                                    )?;
                                    view_scroll = crate::markdown::rendered_row_for_source(
                                        &r.map, cursor,
                                    );
                                    view_map = Some(r.map);
                                    pretty_view = true;
                                }
                            }
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
        } else {
            // Timer tick: 200 ms passed with no key event. If autoreload
            // is enabled, check the file for changes. Only flag a redraw
            // when something actually changed — idle ticks must not touch
            // the screen, or the per-frame Clear flickers.
            if let (Some(w), Some(p)) = (watch.as_mut(), autoreload) {
                match w.poll(p, encoding) {
                    WatchTick::Unchanged | WatchTick::MetadataOnly => {}
                    WatchTick::Reloaded(new_contents) => {
                        contents = new_contents;
                        total_lines = contents.lines().count().max(1);
                        cursor = cursor.min(total_lines).max(1);
                        viewport_top = viewport_top.min(total_lines).max(1);
                        view_map = None;
                        reload_flash = Some(std::time::Instant::now());
                        needs_render = true;
                    }
                }
            }
            // The "[live · reloaded]" tag is shown for 1500 ms after a
            // reload. When that window elapses, do one final redraw to
            // swap it back to "[live]" — otherwise the stale tag lingers
            // until the next key event.
            if let Some(t) = reload_flash {
                if t.elapsed() >= std::time::Duration::from_millis(1500) {
                    reload_flash = None;
                    needs_render = true;
                }
            }
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
    pretty_view: bool,
    view_scroll: &mut usize,
    gutter_visible: bool,
    view_map: &mut Option<Vec<(usize, usize)>>,
    live_mode: bool,
    reload_flash: Option<std::time::Instant>,
    wrap_on: bool,
    wrap_mode: WrapMode,
) -> Result<()> {
    let body_rows = term_h
        .saturating_sub(1 + top_pad as usize)
        .max(1);

    // Build the body buffer + position label.
    // - The current transformed view (markdown or pretty) renders the whole
    //   document (with a source-line map), then slices a visible window of
    //   *rendered rows* using view_scroll.
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
        *view_map = Some(rendered.map.clone());
        let rows: Vec<&str> = rendered.text.split('\n').collect();
        let total = rows.len().max(1);
        let max_scroll = total.saturating_sub(body_rows);
        if *view_scroll > max_scroll {
            *view_scroll = max_scroll;
        }
        let end = (*view_scroll + body_rows).min(total);
        let visible = &rows[*view_scroll..end];
        let body = visible.join("\n");
        let src_line = crate::markdown::source_line_for_rendered(&rendered.map, *view_scroll);
        let label = format!(
            "rendered {}/{} ↔ src {}",
            *view_scroll + 1,
            total,
            src_line
        );
        (body.into_bytes(), label)
    } else if pretty_view {
        let line_no_width = total_lines.to_string().len().max(4);
        let mut highlighter = Highlighter::new(syntax, theme, syntax_set);
        let rendered = crate::json::render_with_gutter(
            contents,
            line_no_width,
            gutter_visible, // numbers track the n-key toggle
            false,          // no grid in interactive mode
            true,           // interactive is always color
            &mut highlighter,
        )?;
        *view_map = Some(rendered.map.clone());
        let rows: Vec<&str> = rendered.text.split('\n').collect();
        let total = rows.len().max(1);
        let max_scroll = total.saturating_sub(body_rows);
        if *view_scroll > max_scroll {
            *view_scroll = max_scroll;
        }
        let end = (*view_scroll + body_rows).min(total);
        let visible = &rows[*view_scroll..end];
        let body = visible.join("\n");
        let src_line = crate::markdown::source_line_for_rendered(&rendered.map, *view_scroll);
        let label = format!("pretty {}/{} ↔ src {}", *view_scroll + 1, total, src_line);
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
        // When wrapping, viewport_bot (a source-line count) is too small —
        // each line spans several rows. Extend the range until we have at
        // least body_rows visual rows (or hit EOF); the buffer is clipped to
        // exactly body_rows rows after rendering.
        let line_no_width = total_lines.to_string().len().max(4);
        let gutter_w = if gutter_visible { line_no_width + 1 + 2 } else { 0 };
        let body_width = term_w.saturating_sub(gutter_w);
        let render_end = if wrap_on {
            let mut acc = 0usize;
            let mut last = viewport_top;
            let mut l = viewport_top;
            while l <= total_lines {
                acc += crate::printer::visual_row_count(
                    contents.lines().nth(l - 1).unwrap_or(""),
                    body_width,
                    tabs,
                    show_all,
                    wrap_mode,
                );
                last = l;
                if acc >= body_rows {
                    break;
                }
                l += 1;
            }
            last
        } else {
            viewport_bot
        };
        let cfg = PrinterConfig {
            style,
            line_range: Some(LineRange {
                start: viewport_top,
                end: render_end,
            }),
            highlight_lines,
            tabs,
            wrap: if wrap_on { wrap_mode } else { crate::cli::WrapMode::Never },
            show_all,
            use_color: true,
            width: term_w,
            language_name: &syntax.name,
            // Cursor glyph (▶) lives in the gutter — hide it alongside line
            // numbers when the gutter is toggled off.
            cursor: if gutter_visible { Some(cursor) } else { None },
            line_numbers,
            markdown: false,
            pretty: false,
        };
        let mut buf: Vec<u8> = Vec::with_capacity(term_w * term_h);
        let stub_input = crate::input::InputKind::Stdin;
        print(&mut buf, &stub_input, contents, &mut highlighter, &cfg)?;
        (buf, format!("line {}/{}", cursor, total_lines))
    };

    // Status bar — shows position, mode tag, current top-pad (when nonzero),
    // and key hints.
    let mode_tag = if markdown_view {
        "  [md]"
    } else if pretty_view {
        "  [json]"
    } else {
        ""
    };
    let gutter_tag = if !gutter_visible { "  no-gutter" } else { "" };
    let wrap_tag = if wrap_on { "  wrap" } else { "" };
    let pad_tag = if top_pad > 0 {
        format!("  pad={}", top_pad)
    } else {
        String::new()
    };
    let live_tag = if live_mode {
        let recently_reloaded = reload_flash
            .map(|t| t.elapsed() < std::time::Duration::from_millis(1500))
            .unwrap_or(false);
        if recently_reloaded {
            "  [live · reloaded]"
        } else {
            "  [live]"
        }
    } else {
        ""
    };
    let status_label = format!(
        "  {}  {}  ({}){}{}{}{}{}  j/k g/G ^d/^u m n p w +/- q",
        file_label,
        position_label,
        match line_numbers {
            LineNumberStyle::Absolute => "abs",
            LineNumberStyle::Relative => "rel",
        },
        mode_tag,
        gutter_tag,
        wrap_tag,
        pad_tag,
        live_tag,
    );
    let status_truncated: String = status_label.chars().take(term_w).collect();
    let pad = term_w.saturating_sub(status_truncated.chars().count());

    // Keep the body within body_rows visual rows AND within term_w columns so
    // it never overflows past the status bar / scrolls the alt-screen.
    let body_bytes: Vec<u8> = if wrap_on && !markdown_view && !pretty_view {
        // Wrap-on: clip to body_rows visual rows. Each row is normally already
        // <= body_width <= term_w, but when body_width == 0 (gutter wider than
        // the terminal) the printer can't wrap, so also truncate each row to
        // term_w (ANSI-aware) — otherwise the terminal would soft-wrap it and
        // scroll the top off, the bug we're preventing. A trailing RESET stops
        // a row cut mid-color from bleeding onto the status bar.
        let body_text = String::from_utf8_lossy(&body_bytes);
        let mut s = String::with_capacity(body_text.len());
        for (i, line) in body_text.split('\n').take(body_rows).enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str(&crate::printer::truncate_to_visible_width(line, term_w));
        }
        s.push_str("\x1b[0m");
        s.into_bytes()
    } else {
        // Wrap-off (and markdown rows): wrap=Never emits full lines, so the
        // *terminal* would wrap any line wider than term_w onto extra rows —
        // overflowing past the status bar. Truncate each visual line to term_w
        // (ANSI-aware) to keep one source line on one row.
        let body_text = String::from_utf8_lossy(&body_bytes);
        let mut clipped = String::with_capacity(body_text.len());
        for (i, line) in body_text.split('\n').enumerate() {
            if i > 0 {
                clipped.push('\n');
            }
            clipped.push_str(&crate::printer::truncate_to_visible_width(line, term_w));
        }
        clipped.into_bytes()
    };

    // Build the whole frame in one buffer and flush once. Using execute! per
    // step (which flushes between calls) makes the terminal briefly show the
    // post-Clear blank state before the body arrives — that's visible flicker.
    // queue! defers I/O; the single flush at the end delivers clear + body +
    // status bar as one update.
    //
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
    queue!(out, Clear(ClearType::All), MoveTo(0, top_pad))?;
    out.write_all(&crlf_buf)?;
    queue!(
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

/// Live-mode file watcher state. Holds enough of the last-seen file to
/// (a) cheaply gate on mtime+len and (b) confirm content actually changed
/// via byte comparison when the gate trips.
struct WatchState {
    mtime: Option<SystemTime>,
    len: u64,
    bytes: Vec<u8>,
}

enum WatchTick {
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
    fn seed(path: &Path) -> anyhow::Result<Self> {
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
    fn poll(&mut self, path: &Path, encoding: Encoding) -> WatchTick {
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

    #[test]
    fn wrapped_scroll_no_change_when_cursor_fits() {
        // single-row lines, cursor 5, top 1, body 10 → stays 1.
        assert_eq!(scroll_viewport_wrapped(5, 1, 10, |_| 1), 1);
    }

    #[test]
    fn wrapped_scroll_pulls_top_up_to_cursor() {
        // cursor above the window → top snaps to cursor.
        assert_eq!(scroll_viewport_wrapped(3, 10, 10, |_| 1), 3);
    }

    #[test]
    fn wrapped_scroll_advances_top_for_tall_lines() {
        // every line is 4 rows; body 10 fits 2 full lines + part of a 3rd.
        // cursor 5, top 1 → must advance top so rows(top..=5) <= 10.
        // rows(3..=5)=12 >10, rows(4..=5)=8 <=10 → top 4.
        assert_eq!(scroll_viewport_wrapped(5, 1, 10, |_| 4), 4);
    }

    #[test]
    fn wrapped_scroll_clamps_when_single_line_taller_than_body() {
        // one line is 30 rows, body 10. cursor 7 → top can only reach cursor.
        assert_eq!(scroll_viewport_wrapped(7, 1, 10, |_| 30), 7);
    }

    #[test]
    fn step_down_one_screen_single_row_lines() {
        assert_eq!(step_by_rows(1, 10, 100, true, |_| 1), 10);
    }

    #[test]
    fn step_down_stops_early_on_tall_lines() {
        // each line 5 rows, budget 10 → 2 lines fill it; start 1 → 2.
        assert_eq!(step_by_rows(1, 10, 100, true, |_| 5), 2);
    }

    #[test]
    fn step_up_is_symmetric() {
        assert_eq!(step_by_rows(50, 10, 100, false, |_| 1), 41);
    }

    #[test]
    fn step_clamps_at_file_bounds() {
        assert_eq!(step_by_rows(98, 10, 100, true, |_| 1), 100);
        assert_eq!(step_by_rows(3, 10, 100, false, |_| 1), 1);
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
