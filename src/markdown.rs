use std::collections::HashSet;
use std::path::Path;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";

/// A markdown render plus a map from rendered-row index back to source-line.
pub struct RenderedMarkdown {
    /// ANSI-rendered output, with rows separated by `\n`.
    pub text: String,
    /// `(rendered_row_start, source_line_start)` for each top-level block,
    /// sorted by `rendered_row_start` ascending. Both indices are 0-based for
    /// rendered rows and 1-based for source lines.
    pub map: Vec<(usize, usize)>,
}

/// Render Markdown to ANSI per top-level block, returning both the rendered
/// text and a source-line ↔ rendered-row map.
///
/// Strategy: walk pulldown_cmark's offset iterator to identify the byte
/// ranges of top-level blocks (heading / paragraph / list / blockquote /
/// code-block / table / html / rule). For each block, render its source
/// slice via termimad and record the running rendered-row offset alongside
/// the block's source-line start.
///
/// The map's granularity is one entry per top-level block. Within a block
/// (a wrapped paragraph, a multi-row code block) all rendered rows resolve
/// to the block's source-line start — coarse but useful for scroll-preserving
/// view toggles.
pub fn render_with_map(source: &str, width: usize) -> RenderedMarkdown {
    use pulldown_cmark::{Event, Parser, Tag};

    let skin = termimad::MadSkin::default();
    let render_width = width.max(20);

    // Collect top-level block byte ranges by walking pulldown_cmark events.
    // We must increment depth on EVERY Start (block-level *and* inline) and
    // decrement on EVERY End — otherwise inline tags like Strong / Emph
    // inside a paragraph would unbalance the counter and prematurely close
    // the outer block. We only record `current_start` when the Start is a
    // top-level block tag at depth 0.
    fn is_top_level_block(tag: &Tag) -> bool {
        matches!(
            tag,
            Tag::Paragraph
                | Tag::Heading { .. }
                | Tag::BlockQuote(_)
                | Tag::CodeBlock(_)
                | Tag::List(_)
                | Tag::HtmlBlock
                | Tag::Table(_)
                | Tag::FootnoteDefinition(_)
                | Tag::DefinitionList
                | Tag::MetadataBlock(_)
        )
    }

    let mut blocks: Vec<std::ops::Range<usize>> = Vec::new();
    let mut depth: i32 = 0;
    let mut current_start: Option<usize> = None;

    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 && is_top_level_block(&tag) {
                    current_start = Some(range.start);
                }
                depth += 1;
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = current_start.take() {
                        blocks.push(start..range.end);
                    }
                }
            }
            Event::Rule if depth == 0 => {
                blocks.push(range);
            }
            _ => {}
        }
    }

    // Render each block separately and stitch together the output + map.
    let mut text = String::new();
    let mut map: Vec<(usize, usize)> = Vec::with_capacity(blocks.len());
    let mut rendered_row: usize = 0;

    for range in &blocks {
        let source_line = source[..range.start].matches('\n').count() + 1;
        let chunk_src = &source[range.clone()];
        let chunk_out = skin.text(chunk_src, Some(render_width)).to_string();

        map.push((rendered_row, source_line));
        // Row count: empty → 0; ends with \n → newline count is exactly the
        // row count (each \n terminates a row); else → newline count + 1
        // (a final partial row without terminator).
        let row_count = if chunk_out.is_empty() {
            0
        } else if chunk_out.ends_with('\n') {
            chunk_out.matches('\n').count()
        } else {
            chunk_out.matches('\n').count() + 1
        };
        text.push_str(&chunk_out);
        // Ensure block separator: insert a \n if the chunk didn't end in one.
        // termimad usually ends each block with a newline, so this rarely fires.
        if !chunk_out.is_empty() && !chunk_out.ends_with('\n') {
            text.push('\n');
        }
        rendered_row = rendered_row.saturating_add(row_count);
    }

    RenderedMarkdown { text, map }
}

/// Render a Markdown source string to ANSI-escaped output, sized for the
/// given terminal width. Backwards-compatible thin wrapper over
/// `render_with_map` that drops the source-line map.
pub fn render_to_string(source: &str, width: usize) -> String {
    render_with_map(source, width).text
}

/// A rendered markdown with a per-row gutter prefix (source-line numbers +
/// optional grid bar). The gutter columns are already baked into `text`.
pub struct RenderedMarkdownWithGutter {
    pub text: String,
    pub map: Vec<(usize, usize)>,
    /// Visible columns the gutter consumes per row. Matches the prefix
    /// width that `text` carries.
    pub gutter_width: usize,
}

/// Render markdown for a target terminal width with a per-row gutter showing
/// source-line numbers (on each block's first row only) and an optional grid
/// bar. The body is rendered at `term_w - gutter_width` so the total row fits.
///
/// `line_no_width` is typically `total_source_lines.to_string().len().max(4)`.
/// `show_numbers` and `show_grid` gate the corresponding gutter components.
/// `use_color` controls whether the line-number column is dimmed via ANSI.
pub fn render_with_gutter(
    source: &str,
    term_w: usize,
    line_no_width: usize,
    show_numbers: bool,
    show_grid: bool,
    use_color: bool,
) -> RenderedMarkdownWithGutter {
    let gutter_width = (if show_numbers { line_no_width + 1 } else { 0 })
        + (if show_grid { 2 } else { 0 });

    if gutter_width == 0 {
        // No gutter requested — render at full width, return text as-is.
        let r = render_with_map(source, term_w);
        return RenderedMarkdownWithGutter {
            text: r.text,
            map: r.map,
            gutter_width: 0,
        };
    }

    let body_width = term_w.saturating_sub(gutter_width).max(20);
    let rendered = render_with_map(source, body_width);

    // O(1) lookup: which rendered-row indices are block starts?
    let block_starts: HashSet<usize> = rendered.map.iter().map(|(r, _)| *r).collect();
    // Build a parallel slice for source-line lookups.
    let map = rendered.map.clone();

    // Walk rows; prefix each.
    let rows: Vec<&str> = rendered.text.split('\n').collect();
    let mut out = String::with_capacity(rendered.text.len() + rows.len() * gutter_width);
    let last = rows.len().saturating_sub(1);
    for (idx, row) in rows.iter().enumerate() {
        // Number cell.
        if show_numbers {
            if block_starts.contains(&idx) {
                let src_line = source_line_for_rendered(&map, idx);
                let label = format!("{:>width$}", src_line, width = line_no_width);
                if use_color {
                    out.push_str(DIM);
                    out.push_str(&label);
                    out.push_str(RESET);
                } else {
                    out.push_str(&label);
                }
                out.push(' ');
            } else {
                // Continuation row: blank line-number column.
                for _ in 0..(line_no_width + 1) {
                    out.push(' ');
                }
            }
        }
        // Grid cell.
        if show_grid {
            out.push_str("│ ");
        }
        // Body row.
        out.push_str(row);
        if idx < last {
            out.push('\n');
        }
    }

    RenderedMarkdownWithGutter {
        text: out,
        map,
        gutter_width,
    }
}

/// Given a 1-indexed source line, find the rendered-row offset to scroll to.
/// Returns the row of the block whose source-line start is the largest value
/// less-than-or-equal to `source_line`. Returns 0 for an empty map.
pub fn rendered_row_for_source(map: &[(usize, usize)], source_line: usize) -> usize {
    if map.is_empty() {
        return 0;
    }
    // Binary search by source_line (the second tuple element).
    let mut lo = 0usize;
    let mut hi = map.len();
    let mut best: usize = 0;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if map[mid].1 <= source_line {
            best = map[mid].0;
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    best
}

/// Given a 0-indexed rendered row, find the source-line of the block that
/// row belongs to.
pub fn source_line_for_rendered(map: &[(usize, usize)], rendered_row: usize) -> usize {
    if map.is_empty() {
        return 1;
    }
    let mut lo = 0usize;
    let mut hi = map.len();
    let mut best: usize = map[0].1;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if map[mid].0 <= rendered_row {
            best = map[mid].1;
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    best
}

/// True when `path` looks like a Markdown file by extension.
pub fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown" | "mdown" | "mkd")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn renders_basic_markdown() {
        let out = render_to_string("# Hello\n\nThis is **bold**.\n", 80);
        assert!(out.contains("\x1b["), "expected ANSI in: {:?}", out);
        assert!(out.contains("Hello"));
        assert!(out.contains("bold"));
    }

    #[test]
    fn render_handles_empty_input() {
        let out = render_to_string("", 80);
        let _ = out;
    }

    #[test]
    fn render_with_map_handles_inline_tags_in_blocks() {
        // Regression: a paragraph with **bold** / *emph* / `code` should be
        // ONE block. Earlier impl decremented depth on inline End events
        // without incrementing on inline Starts, prematurely closing the
        // outer block.
        let src = "First **bold** paragraph.\n\n\
                   Second *emph* paragraph with `code`.\n\n\
                   Third paragraph plain.\n";
        let r = render_with_map(src, 80);
        // Three paragraphs → exactly three blocks in the map.
        assert_eq!(
            r.map.len(),
            3,
            "expected 3 paragraph blocks, got map: {:?}",
            r.map
        );
        // All three paragraph texts must survive in the rendered output.
        assert!(r.text.contains("First"), "missing first: {:?}", r.text);
        assert!(r.text.contains("Second"), "missing second: {:?}", r.text);
        assert!(r.text.contains("Third"), "missing third: {:?}", r.text);
    }

    #[test]
    fn render_with_map_distinct_blocks() {
        let src = "# Title\n\nA paragraph.\n\n- list item\n- another\n";
        let r = render_with_map(src, 80);
        assert!(r.map.len() >= 3, "expected ≥3 blocks, got map: {:?}", r.map);
        // Rendered rows strictly non-decreasing.
        for w in r.map.windows(2) {
            assert!(w[0].0 <= w[1].0, "rendered rows not monotonic: {:?}", r.map);
        }
        // Source lines strictly non-decreasing.
        for w in r.map.windows(2) {
            assert!(w[0].1 <= w[1].1, "source lines not monotonic: {:?}", r.map);
        }
    }

    #[test]
    fn render_with_map_first_block_at_row_zero() {
        let src = "# Heading\n";
        let r = render_with_map(src, 80);
        assert!(!r.map.is_empty());
        assert_eq!(r.map[0].0, 0, "first block must start at rendered row 0");
        assert_eq!(r.map[0].1, 1, "first block must start at source line 1");
    }

    #[test]
    fn rendered_row_for_source_lookup() {
        let map = vec![(0, 1), (5, 10), (12, 20), (30, 50)];
        assert_eq!(rendered_row_for_source(&map, 1), 0);
        assert_eq!(rendered_row_for_source(&map, 9), 0);
        assert_eq!(rendered_row_for_source(&map, 10), 5);
        assert_eq!(rendered_row_for_source(&map, 15), 5);
        assert_eq!(rendered_row_for_source(&map, 20), 12);
        assert_eq!(rendered_row_for_source(&map, 1000), 30);
        // Below the first source line falls back to row 0 (best=0 default).
        assert_eq!(rendered_row_for_source(&map, 0), 0);
    }

    #[test]
    fn source_line_for_rendered_lookup() {
        let map = vec![(0, 1), (5, 10), (12, 20), (30, 50)];
        assert_eq!(source_line_for_rendered(&map, 0), 1);
        assert_eq!(source_line_for_rendered(&map, 4), 1);
        assert_eq!(source_line_for_rendered(&map, 5), 10);
        assert_eq!(source_line_for_rendered(&map, 12), 20);
        assert_eq!(source_line_for_rendered(&map, 100), 50);
    }

    #[test]
    fn lookup_helpers_handle_empty_map() {
        let map: Vec<(usize, usize)> = vec![];
        assert_eq!(rendered_row_for_source(&map, 42), 0);
        assert_eq!(source_line_for_rendered(&map, 42), 1);
    }

    #[test]
    fn detects_md_extension() {
        assert!(is_markdown_path(&PathBuf::from("README.md")));
        assert!(is_markdown_path(&PathBuf::from("notes.markdown")));
        assert!(is_markdown_path(&PathBuf::from("doc.MD")));
        assert!(is_markdown_path(&PathBuf::from("file.mkd")));
        assert!(is_markdown_path(&PathBuf::from("file.mdown")));
    }

    #[test]
    fn rejects_non_md_extensions() {
        assert!(!is_markdown_path(&PathBuf::from("main.rs")));
        assert!(!is_markdown_path(&PathBuf::from("README")));
        assert!(!is_markdown_path(&PathBuf::from("config.toml")));
    }

    #[test]
    fn render_with_gutter_off_when_neither_flag() {
        let src = "# Title\n\nA paragraph.\n";
        let plain = render_with_map(src, 80).text;
        let r = render_with_gutter(src, 80, 4, false, false, false);
        assert_eq!(r.gutter_width, 0);
        assert_eq!(r.text, plain);
    }

    #[test]
    fn render_with_gutter_width_accounting() {
        // numbers (line_no_width=4 + 1 sep) + grid (2) = 7
        let r = render_with_gutter("# Title\n", 80, 4, true, true, false);
        assert_eq!(r.gutter_width, 7);
        // numbers only: 4 + 1 = 5
        let r = render_with_gutter("# Title\n", 80, 4, true, false, false);
        assert_eq!(r.gutter_width, 5);
        // grid only: 2
        let r = render_with_gutter("# Title\n", 80, 4, false, true, false);
        assert_eq!(r.gutter_width, 2);
    }

    #[test]
    fn render_with_gutter_prefixes_block_starts() {
        // 3 blocks at source lines 1, 3, 5.
        let src = "# Title\n\nA paragraph.\n\n- Item one\n- Item two\n";
        let r = render_with_gutter(src, 80, 4, true, false, false);
        // Each row in the output starts with a 5-char gutter ('NNNN '). Block
        // starts have a number; continuations have spaces. Iterate rows and
        // check that exactly the rows in r.map have non-blank line numbers.
        let starts: std::collections::HashSet<usize> =
            r.map.iter().map(|(rr, _)| *rr).collect();
        for (idx, row) in r.text.split('\n').enumerate() {
            let prefix: String = row.chars().take(5).collect();
            if starts.contains(&idx) {
                // Should contain at least one digit in the line-number cell.
                assert!(
                    prefix.trim().chars().any(|c| c.is_ascii_digit()),
                    "block-start row {} missing line number; prefix={:?}",
                    idx, prefix
                );
            } else {
                assert!(
                    prefix.trim().is_empty(),
                    "continuation row {} has unexpected number; prefix={:?}",
                    idx, prefix
                );
            }
        }
    }

    #[test]
    fn render_with_gutter_grid_repeats_on_every_row() {
        let src = "# Title\n\nA paragraph.\n";
        let r = render_with_gutter(src, 80, 4, true, true, false);
        // Every row should contain '│' (grid bar).
        for (idx, row) in r.text.split('\n').enumerate() {
            assert!(
                row.contains('│'),
                "row {} missing grid bar: {:?}",
                idx, row
            );
        }
    }
}
