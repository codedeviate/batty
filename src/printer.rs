use crate::cli::{LineNumberStyle, WrapMode};
use crate::git::{LineChange, diff_for_file};
use crate::highlight::Highlighter;
use crate::input::{InputKind, LineRange};
use anyhow::Result;
use std::collections::HashSet;
use std::io::Write;

#[derive(Debug, Clone, Copy, Default)]
pub struct StyleFlags {
    pub header: bool,
    pub grid: bool,
    pub numbers: bool,
    pub rule: bool,
    pub changes: bool,
    pub snip: bool,
}

impl StyleFlags {
    /// Parse a comma-separated style spec, e.g. "full" or "header,grid,numbers".
    pub fn parse(spec: &str, plain: bool, number_flag: bool, diff_flag: bool) -> Self {
        let mut s = StyleFlags::default();
        if plain {
            return s;
        }
        for token in spec.split(',') {
            match token.trim() {
                "full" => {
                    s = StyleFlags {
                        header: true, grid: true, numbers: true,
                        rule: true, changes: true, snip: true,
                    };
                }
                "plain" => s = StyleFlags::default(),
                "header" => s.header = true,
                "grid" => s.grid = true,
                "numbers" => s.numbers = true,
                "rule" => s.rule = true,
                "changes" => s.changes = true,
                "snip" => s.snip = true,
                _ => {}
            }
        }
        if number_flag { s.numbers = true; }
        if diff_flag { s.changes = true; }
        s
    }

    #[cfg(test)]
    pub fn any(&self) -> bool {
        self.header || self.grid || self.numbers || self.rule || self.changes || self.snip
    }
}

pub struct PrinterConfig<'a> {
    pub style: StyleFlags,
    pub line_range: Option<LineRange>,
    pub highlight_lines: HashSet<usize>,
    pub tabs: usize,
    /// Wrap-mode is parsed and threaded through but not yet honored.
    /// Tracked in OUT-OF-SCOPE.md; remove the allow when wrapping is implemented.
    #[allow(dead_code)]
    pub wrap: WrapMode,
    pub show_all: bool,
    pub use_color: bool,
    pub width: usize, // terminal width for grid drawing
    pub language_name: &'a str,
    /// 1-indexed cursor line. Drives the cursor-indicator gutter and
    /// is the reference point for relative line numbering.
    pub cursor: Option<usize>,
    pub line_numbers: LineNumberStyle,
    /// Render the input as Markdown instead of as syntax-highlighted source.
    /// When true, body decorations (numbers / changes / cursor / line range)
    /// are skipped — markdown rendering produces its own block structure.
    pub markdown: bool,
}

/// Compute the visible label for a line number given the configured style.
/// Pure function so it can be unit-tested without setting up a full render.
pub fn line_number_label(lineno: usize, cursor: Option<usize>, style: LineNumberStyle) -> usize {
    match (style, cursor) {
        (LineNumberStyle::Relative, Some(c)) if lineno != c => {
            if lineno > c { lineno - c } else { c - lineno }
        }
        _ => lineno,
    }
}

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const INVERT: &str = "\x1b[7m";

pub fn print<W: Write>(
    out: &mut W,
    input: &InputKind,
    contents: &str,
    highlighter: &mut Highlighter,
    cfg: &PrinterConfig,
) -> Result<()> {
    let changes = if cfg.style.changes {
        if let InputKind::File(p) = input {
            diff_for_file(p)
        } else {
            Default::default()
        }
    } else {
        Default::default()
    };

    let line_count = contents.lines().count();
    let line_no_width = line_count.max(1).to_string().len().max(4);

    // Header
    if cfg.style.header {
        if cfg.style.grid {
            write_grid_top(out, cfg)?;
        }
        let lang_label = if cfg.markdown {
            "Markdown (rendered)"
        } else {
            cfg.language_name
        };
        writeln!(
            out,
            "{}File:{} {}  {}{}{}",
            if cfg.use_color { BOLD } else { "" },
            if cfg.use_color { RESET } else { "" },
            input.display_name(),
            if cfg.use_color { DIM } else { "" },
            lang_label,
            if cfg.use_color { RESET } else { "" },
        )?;
        if cfg.style.grid {
            write_grid_mid(out, cfg, line_no_width)?;
        }
    } else if cfg.style.grid {
        write_grid_top(out, cfg)?;
    }

    // Markdown short-circuit: skip the per-line body and emit the rendered
    // Markdown directly. termimad respects the width we pass in.
    if cfg.markdown {
        let rendered = crate::markdown::render_to_string(contents, cfg.width);
        out.write_all(rendered.as_bytes())?;
        if cfg.style.grid {
            write_grid_bot(out, cfg, line_no_width)?;
        }
        return Ok(());
    }

    // Body
    for (idx, raw_line) in contents.lines().enumerate() {
        let lineno = idx + 1;
        if let Some(r) = cfg.line_range {
            if !r.contains(lineno) { continue; }
        }
        let displayed = expand_tabs(raw_line, cfg.tabs);
        let displayed = if cfg.show_all { show_all(&displayed) } else { displayed };
        let line_with_nl = format!("{}\n", displayed);
        let highlighted = if cfg.use_color {
            highlighter.highlight_line(&line_with_nl)?
        } else {
            line_with_nl
        };

        // gutter
        if cfg.style.numbers {
            let label = line_number_label(lineno, cfg.cursor, cfg.line_numbers);
            let n = format!("{:>width$}", label, width = line_no_width);
            if cfg.use_color {
                write!(out, "{}{}{} ", DIM, n, RESET)?;
            } else {
                write!(out, "{} ", n)?;
            }
        }
        // cursor indicator (only renders when cfg.cursor is set)
        if cfg.cursor.is_some() {
            let glyph = if cfg.cursor == Some(lineno) { "▶" } else { " " };
            write!(out, "{} ", glyph)?;
        }
        if cfg.style.changes {
            let m = match changes.get(&lineno) {
                Some(LineChange::Added) => "+",
                Some(LineChange::Modified) => "~",
                Some(LineChange::RemovedAbove) => "-",
                None => " ",
            };
            write!(out, "{} ", m)?;
        }
        if cfg.style.grid {
            write!(out, "│ ")?;
        }

        // highlight emphasis
        if cfg.highlight_lines.contains(&lineno) && cfg.use_color {
            write!(out, "{}", INVERT)?;
        }
        out.write_all(highlighted.as_bytes())?;
        if cfg.use_color {
            write!(out, "{}", RESET)?;
        }
    }

    if cfg.style.grid {
        write_grid_bot(out, cfg, line_no_width)?;
    }
    Ok(())
}

fn write_grid_top<W: Write>(out: &mut W, cfg: &PrinterConfig) -> Result<()> {
    let w = cfg.width.saturating_sub(1);
    writeln!(out, "{}", "─".repeat(w))?;
    Ok(())
}
fn write_grid_mid<W: Write>(out: &mut W, cfg: &PrinterConfig, ln_w: usize) -> Result<()> {
    let _ = ln_w;
    let w = cfg.width.saturating_sub(1);
    writeln!(out, "{}", "─".repeat(w))?;
    Ok(())
}
fn write_grid_bot<W: Write>(out: &mut W, cfg: &PrinterConfig, ln_w: usize) -> Result<()> {
    let _ = ln_w;
    let w = cfg.width.saturating_sub(1);
    writeln!(out, "{}", "─".repeat(w))?;
    Ok(())
}

fn expand_tabs(s: &str, width: usize) -> String {
    if width == 0 { return s.to_string(); }
    let mut out = String::with_capacity(s.len());
    let mut col = 0usize;
    for ch in s.chars() {
        if ch == '\t' {
            let pad = width - (col % width);
            for _ in 0..pad { out.push(' '); col += 1; }
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

fn show_all(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\t' => out.push('→'),
            ' ' => out.push('·'),
            c if c.is_control() => out.push('•'),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_style_full() {
        let s = StyleFlags::parse("full", false, false, false);
        assert!(s.header && s.grid && s.numbers && s.changes);
    }

    #[test]
    fn plain_overrides_style() {
        let s = StyleFlags::parse("full", true, false, false);
        assert!(!s.any());
    }

    #[test]
    fn number_flag_adds_numbers() {
        let s = StyleFlags::parse("plain", false, true, false);
        assert!(s.numbers && !s.header);
    }

    #[test]
    fn expand_tabs_to_4() {
        assert_eq!(expand_tabs("\tx", 4), "    x");
        assert_eq!(expand_tabs("a\tb", 4), "a   b");
    }

    #[test]
    fn show_all_marks_tabs() {
        let s = show_all("a\tb");
        assert!(s.contains('→'));
    }

    #[test]
    fn label_absolute_returns_lineno() {
        assert_eq!(line_number_label(7, Some(10), LineNumberStyle::Absolute), 7);
        assert_eq!(line_number_label(7, None, LineNumberStyle::Absolute), 7);
    }

    #[test]
    fn label_relative_without_cursor_falls_back_to_absolute() {
        assert_eq!(line_number_label(7, None, LineNumberStyle::Relative), 7);
    }

    #[test]
    fn label_relative_cursor_line_shows_absolute() {
        assert_eq!(line_number_label(10, Some(10), LineNumberStyle::Relative), 10);
    }

    #[test]
    fn label_relative_other_lines_show_distance() {
        assert_eq!(line_number_label(7, Some(10), LineNumberStyle::Relative), 3);
        assert_eq!(line_number_label(15, Some(10), LineNumberStyle::Relative), 5);
        assert_eq!(line_number_label(1, Some(10), LineNumberStyle::Relative), 9);
    }
}
