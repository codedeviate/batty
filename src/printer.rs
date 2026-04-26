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
    /// Wrap mode for long lines. `Never` emits each source line in one
    /// shot; `Character` and `Auto` break at column boundaries with a
    /// continuation prefix that preserves the gutter.
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
    // Markdown directly. The per-row gutter (line numbers, optional grid bar)
    // shows the source-line of each block on its first row, so users can
    // cross-reference rendered prose to source positions.
    if cfg.markdown {
        let r = crate::markdown::render_with_gutter(
            contents,
            cfg.width,
            line_no_width,
            cfg.style.numbers,
            cfg.style.grid,
            cfg.use_color,
        );
        out.write_all(r.text.as_bytes())?;
        // Ensure the body ends with a newline before any trailing grid_bot.
        if !r.text.ends_with('\n') {
            writeln!(out)?;
        }
        if cfg.style.grid {
            write_grid_bot(out, cfg, line_no_width)?;
        }
        return Ok(());
    }

    // Snip prefix: when --style includes `snip` and --line-range hides the
    // start of the file, emit a single "N lines skipped" indicator so the
    // reader knows there's content above the visible window.
    if cfg.style.snip {
        if let Some(r) = cfg.line_range {
            if r.start > 1 {
                let n = r.start - 1;
                writeln!(
                    out,
                    "{}··· {} line{} skipped ···{}",
                    if cfg.use_color { DIM } else { "" },
                    n,
                    if n == 1 { "" } else { "s" },
                    if cfg.use_color { RESET } else { "" },
                )?;
            }
        }
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

        // Build first-row gutter and continuation prefix as strings.
        let (first_gutter, gutter_w) =
            build_first_gutter(cfg, line_no_width, lineno, &changes);
        let cont_prefix = build_continuation_prefix(cfg, line_no_width);
        let body_width = cfg.width.saturating_sub(gutter_w);

        let highlighted_active = cfg.highlight_lines.contains(&lineno) && cfg.use_color;
        // INVERT is a persistent SGR attribute that needs re-applying after
        // each wrap break. The wrap fn handles it; we also emit it before
        // the first row's body content so row 1 is inverted from the start.
        let persistent_sgr = if highlighted_active { INVERT } else { "" };

        let wrapped = wrap_with_continuation(
            &highlighted,
            body_width,
            &cont_prefix,
            cfg.wrap,
            persistent_sgr,
        );

        out.write_all(first_gutter.as_bytes())?;
        if highlighted_active {
            out.write_all(INVERT.as_bytes())?;
        }
        out.write_all(wrapped.as_bytes())?;
        if cfg.use_color {
            write!(out, "{}", RESET)?;
        }
    }

    // Snip suffix: when --style includes `snip` and --line-range hides the
    // tail of the file, emit a "N lines skipped" indicator at the end.
    if cfg.style.snip {
        if let Some(r) = cfg.line_range {
            if r.end < line_count {
                let n = line_count - r.end;
                writeln!(
                    out,
                    "{}··· {} line{} skipped ···{}",
                    if cfg.use_color { DIM } else { "" },
                    n,
                    if n == 1 { "" } else { "s" },
                    if cfg.use_color { RESET } else { "" },
                )?;
            }
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

/// Build the first-row gutter for a source line: line-number cell (DIM when
/// colored) + cursor glyph cell + change-marker cell + grid bar.
/// Returns the string and its visible-column width (used to compute
/// `body_width` for wrapping).
fn build_first_gutter(
    cfg: &PrinterConfig,
    line_no_width: usize,
    lineno: usize,
    changes: &std::collections::HashMap<usize, LineChange>,
) -> (String, usize) {
    let mut s = String::new();
    let mut w = 0;
    if cfg.style.numbers {
        let label = line_number_label(lineno, cfg.cursor, cfg.line_numbers);
        let n = format!("{:>width$}", label, width = line_no_width);
        if cfg.use_color {
            s.push_str(DIM);
            s.push_str(&n);
            s.push_str(RESET);
            s.push(' ');
        } else {
            s.push_str(&n);
            s.push(' ');
        }
        w += line_no_width + 1;
    }
    if cfg.cursor.is_some() {
        let glyph = if cfg.cursor == Some(lineno) { "▶" } else { " " };
        s.push_str(glyph);
        s.push(' ');
        w += 2;
    }
    if cfg.style.changes {
        let m = match changes.get(&lineno) {
            Some(LineChange::Added) => "+",
            Some(LineChange::Modified) => "~",
            Some(LineChange::RemovedAbove) => "-",
            None => " ",
        };
        s.push_str(m);
        s.push(' ');
        w += 2;
    }
    if cfg.style.grid {
        s.push_str("│ ");
        w += 2;
    }
    (s, w)
}

/// Build the continuation prefix for wrapped rows. Same visible width as
/// `build_first_gutter`'s output — but the line number, cursor glyph, and
/// change marker are replaced with whitespace. The grid bar still repeats.
fn build_continuation_prefix(cfg: &PrinterConfig, line_no_width: usize) -> String {
    let mut s = String::new();
    if cfg.style.numbers {
        for _ in 0..line_no_width { s.push(' '); }
        s.push(' ');
    }
    if cfg.cursor.is_some() {
        s.push_str("  ");
    }
    if cfg.style.changes {
        s.push_str("  ");
    }
    if cfg.style.grid {
        s.push_str("│ ");
    }
    s
}

/// Wrap an already-rendered (highlighted, ANSI-escaped) line at column
/// boundaries. ANSI escape sequences pass through without counting toward
/// the column total. At each wrap point, emit:
/// - `\x1b[0m` (so the continuation prefix isn't tinted by leftover SGR)
/// - newline
/// - the continuation prefix (plain text)
/// - `persistent_sgr` (e.g. INVERT for highlighted lines) so SGR attributes
///   that aren't captured in syntect's per-token escapes resume on the next row
/// - the most recently seen `\x1b[...m` so the syntect-emitted FG resumes
///
/// A trailing `\n` in `rendered` (the source-line terminator) is preserved
/// untouched; columns reset on it.
fn wrap_with_continuation(
    rendered: &str,
    body_width: usize,
    continuation_prefix: &str,
    mode: WrapMode,
    persistent_sgr: &str,
) -> String {
    if matches!(mode, WrapMode::Never) || body_width == 0 {
        return rendered.to_string();
    }
    use unicode_width::UnicodeWidthChar;
    let mut out = String::with_capacity(rendered.len() + 32);
    let mut col: usize = 0;
    let mut in_escape = false;
    let mut current_escape = String::new();
    let mut last_escape = String::new();

    for ch in rendered.chars() {
        if in_escape {
            current_escape.push(ch);
            out.push(ch);
            if ch == 'm' {
                in_escape = false;
                last_escape = current_escape.clone();
                current_escape.clear();
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            current_escape.clear();
            current_escape.push(ch);
            out.push(ch);
            continue;
        }
        if ch == '\n' {
            out.push(ch);
            col = 0;
            continue;
        }
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        // Wrap when adding this char would overflow, but only if we've
        // already placed something on this row (col > 0) — otherwise a
        // single 2-col char into a 1-col body_width loops forever.
        if col > 0 && col + w > body_width {
            let needs_reset = !last_escape.is_empty() || !persistent_sgr.is_empty();
            if needs_reset {
                out.push_str(RESET);
            }
            out.push('\n');
            out.push_str(continuation_prefix);
            // Re-apply the persistent SGR (e.g. INVERT) FIRST so it sits
            // alongside any syntect FG/BG escape that follows.
            if !persistent_sgr.is_empty() {
                out.push_str(persistent_sgr);
            }
            if !last_escape.is_empty() {
                out.push_str(&last_escape);
            }
            col = 0;
        }
        out.push(ch);
        col += w;
    }
    out
}

fn expand_tabs(s: &str, width: usize) -> String {
    if width == 0 { return s.to_string(); }
    use unicode_width::UnicodeWidthChar;
    let mut out = String::with_capacity(s.len());
    let mut col = 0usize;
    for ch in s.chars() {
        if ch == '\t' {
            let pad = width - (col % width);
            for _ in 0..pad { out.push(' '); col += 1; }
        } else {
            out.push(ch);
            col += UnicodeWidthChar::width(ch).unwrap_or(0);
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
    fn expand_tabs_handles_wide_chars() {
        // 中 is double-width: after it, col is 2; tab pads to col 4 → 2 spaces.
        assert_eq!(expand_tabs("中\tx", 4), "中  x");
        // Two wide chars consume 4 cols; tab pads to col 8 → 4 spaces.
        assert_eq!(expand_tabs("中文\tx", 4), "中文    x");
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

    #[test]
    fn wrap_passes_through_when_never() {
        let s = "this string is exactly twenty-eight chars long";
        let out = wrap_with_continuation(s, 10, "  ", WrapMode::Never, "");
        assert_eq!(out, s);
    }

    #[test]
    fn wrap_breaks_at_body_width() {
        // body_width=5, prefix=">>", no ANSI, no persistent SGR.
        let out = wrap_with_continuation("abcdefghij", 5, ">>", WrapMode::Character, "");
        assert_eq!(out, "abcde\n>>fghij");
    }

    #[test]
    fn wrap_preserves_ansi_escapes_across_breaks() {
        let red = "\x1b[38;2;255;0;0m";
        let input = format!("{}abcdefghij", red);
        let out = wrap_with_continuation(&input, 4, "..", WrapMode::Character, "");
        assert!(out.starts_with(&format!("{}abcd", red)));
        let post_break_idx = out.find("\x1b[0m\n..").unwrap();
        let after = &out[post_break_idx + "\x1b[0m\n..".len()..];
        assert!(after.starts_with(red), "expected red re-emitted, got: {:?}", after);
    }

    #[test]
    fn wrap_handles_wide_chars() {
        let out = wrap_with_continuation("中文中文", 5, "..", WrapMode::Character, "");
        assert_eq!(out, "中文\n..中文");
    }

    #[test]
    fn wrap_resets_column_on_newline() {
        let out = wrap_with_continuation("aaaa\nbb", 4, "..", WrapMode::Character, "");
        assert_eq!(out, "aaaa\nbb");
    }

    #[test]
    fn wrap_re_emits_persistent_sgr_after_break() {
        // INVERT as persistent SGR should be re-emitted on every continuation
        // (alongside any captured FG escape).
        let invert = "\x1b[7m";
        let red = "\x1b[38;2;255;0;0m";
        let input = format!("{}abcdefghij", red);
        let out = wrap_with_continuation(&input, 4, "..", WrapMode::Character, invert);
        // After break: RESET + newline + prefix + INVERT + last_escape (red)
        let break_marker = format!("\x1b[0m\n..{}{}", invert, red);
        assert!(
            out.contains(&break_marker),
            "expected persistent INVERT after break in: {:?}",
            out
        );
    }

    #[test]
    fn build_first_gutter_includes_all_components() {
        let mut hl = std::collections::HashSet::new();
        hl.insert(1);
        let cfg = PrinterConfig {
            style: StyleFlags { numbers: true, grid: true, changes: true, ..Default::default() },
            line_range: None,
            highlight_lines: hl,
            tabs: 4,
            wrap: WrapMode::Auto,
            show_all: false,
            use_color: false,
            width: 80,
            language_name: "Rust",
            cursor: Some(1),
            line_numbers: LineNumberStyle::Absolute,
            markdown: false,
        };
        let changes = std::collections::HashMap::new();
        let (s, w) = build_first_gutter(&cfg, 4, 1, &changes);
        // numbers (4 + 1) + cursor (2) + changes (2) + grid (2) = 11
        assert_eq!(w, 11);
        assert!(s.contains("1"));
        assert!(s.contains("▶"));
        assert!(s.contains("│"));
    }
}
