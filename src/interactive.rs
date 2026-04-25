use crate::cli::LineNumberStyle;
use crate::highlight::Highlighter;
use crate::input::LineRange;
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

pub fn run<'a>(
    file_label: &str,
    contents: &str,
    syntax: &'a syntect::parsing::SyntaxReference,
    syntax_set: &'a syntect::parsing::SyntaxSet,
    theme: &'a syntect::highlighting::Theme,
    line_numbers: LineNumberStyle,
    tabs: usize,
    show_all: bool,
) -> Result<()> {
    let total_lines = contents.lines().count().max(1);
    let mut cursor: usize = 1;
    let mut viewport_top: usize = 1;

    let _guard = TerminalGuard::enter()?;

    loop {
        let (term_w, term_h) = size().unwrap_or((80, 24));
        let term_w = term_w as usize;
        let term_h = term_h as usize;
        // Reserve last row for the status bar.
        let body_rows = term_h.saturating_sub(1).max(1);
        viewport_top = scroll_viewport(cursor, viewport_top, body_rows, total_lines);
        let viewport_bot = (viewport_top + body_rows - 1).min(total_lines);

        render_frame(
            file_label,
            contents,
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
                        if cursor < total_lines {
                            cursor += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if cursor > 1 {
                            cursor -= 1;
                        }
                    }
                    KeyCode::Char('g') | KeyCode::Home => {
                        cursor = 1;
                    }
                    KeyCode::Char('G') | KeyCode::End => {
                        cursor = total_lines;
                    }
                    KeyCode::PageDown => {
                        cursor = (cursor + body_rows).min(total_lines);
                    }
                    KeyCode::PageUp => {
                        cursor = cursor.saturating_sub(body_rows).max(1);
                    }
                    KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                        cursor = (cursor + body_rows / 2).min(total_lines);
                    }
                    KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                        cursor = cursor.saturating_sub(body_rows / 2).max(1);
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
) -> Result<()> {
    let mut highlighter = Highlighter::new(syntax, theme, syntax_set);
    let mut highlight_lines = std::collections::HashSet::new();
    highlight_lines.insert(cursor);

    let style = StyleFlags {
        header: false,
        grid: false,
        numbers: true,
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
        wrap: crate::cli::WrapMode::Auto,
        show_all,
        use_color: true,
        width: term_w,
        language_name: &syntax.name,
        cursor: Some(cursor),
        line_numbers,
    };

    // Render body to a buffer.
    let mut buf: Vec<u8> = Vec::with_capacity(term_w * term_h);
    let stub_input = crate::input::InputKind::Stdin; // diff disabled via style.changes=false
    print(&mut buf, &stub_input, contents, &mut highlighter, &cfg)?;

    // Compose status bar.
    let status_label = format!(
        "  {}  line {}/{}  ({}, vim-keys: j/k g/G ^d/^u q quit)",
        file_label,
        cursor,
        total_lines,
        match line_numbers {
            LineNumberStyle::Absolute => "abs",
            LineNumberStyle::Relative => "rel",
        }
    );
    let status_truncated: String = status_label.chars().take(term_w).collect();
    let pad = term_w.saturating_sub(status_truncated.chars().count());

    // Atomic-ish write: clear screen, move to (0,0), write body, then status bar.
    // In raw mode, '\n' only moves the cursor down without returning to column 0,
    // which causes a staircase. Translate '\n' → '\r\n' so each line starts fresh.
    let mut crlf_buf: Vec<u8> = Vec::with_capacity(buf.len() + 64);
    for &b in &buf {
        if b == b'\n' {
            crlf_buf.push(b'\r');
        }
        crlf_buf.push(b);
    }
    let mut out = stdout().lock();
    execute!(out, Clear(ClearType::All), MoveTo(0, 0))?;
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
}
