use std::path::Path;

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

    // Collect top-level block byte ranges. Track depth: Start increments,
    // End decrements. A "top-level block" is anything whose Start happens
    // at depth 0; it ends when we return to depth 0 via End.
    let mut blocks: Vec<std::ops::Range<usize>> = Vec::new();
    let mut depth: i32 = 0;
    let mut current_start: Option<usize> = None;

    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Start(Tag::Paragraph)
            | Event::Start(Tag::Heading { .. })
            | Event::Start(Tag::BlockQuote(_))
            | Event::Start(Tag::CodeBlock(_))
            | Event::Start(Tag::List(_))
            | Event::Start(Tag::Item)
            | Event::Start(Tag::HtmlBlock)
            | Event::Start(Tag::Table(_))
            | Event::Start(Tag::TableHead)
            | Event::Start(Tag::TableRow)
            | Event::Start(Tag::TableCell)
            | Event::Start(Tag::FootnoteDefinition(_))
            | Event::Start(Tag::DefinitionList)
            | Event::Start(Tag::DefinitionListTitle)
            | Event::Start(Tag::DefinitionListDefinition)
            | Event::Start(Tag::MetadataBlock(_)) => {
                if depth == 0 {
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
            // Standalone block-level events (no matching End): rules and
            // bare html sit at depth 0 by themselves.
            Event::Rule | Event::Html(_) if depth == 0 => {
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
        let row_count = chunk_out.matches('\n').count() + if chunk_out.is_empty() { 0 } else { 1 };
        text.push_str(&chunk_out);
        // Ensure block separator: a single newline between adjacent block
        // chunks if the chunk didn't end in one. termimad usually ends with
        // a newline, so this is mostly a safety net.
        if !chunk_out.ends_with('\n') && !chunk_out.is_empty() {
            text.push('\n');
        }
        rendered_row = rendered_row.saturating_add(row_count.max(1));
    }

    RenderedMarkdown { text, map }
}

/// Render a Markdown source string to ANSI-escaped output, sized for the
/// given terminal width. Backwards-compatible thin wrapper over
/// `render_with_map` that drops the source-line map.
pub fn render_to_string(source: &str, width: usize) -> String {
    render_with_map(source, width).text
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
}
