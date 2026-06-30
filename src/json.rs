use std::path::Path;

use anyhow::Result;

use crate::highlight::Highlighter;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";

/// Pretty-print a single JSON value (one JSONL line) with 2-space indentation.
/// The `preserve_order` feature keeps object keys in their original order.
/// Returns `Err` if `line` is not a single valid JSON value.
pub fn prettify_line(line: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    Ok(serde_json::to_string_pretty(&value)?)
}

/// True when `path` looks like a JSON Lines file by extension.
pub fn is_jsonl_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jsonl" | "ndjson")
    )
}

/// A prettified JSONL render plus a map from rendered-row index back to source
/// line. `(rendered_row_start, source_line)` per source line, rows 0-based and
/// source lines 1-based — same shape as `markdown::RenderedMarkdown.map`.
pub struct RenderedJson {
    pub text: String,
    pub map: Vec<(usize, usize)>,
}

/// Prettify each source line and highlight the expanded text. Valid JSON lines
/// expand to multiple highlighted rows; malformed or blank lines pass through
/// verbatim, dimmed when `use_color`, so nothing is ever dropped.
pub fn render_with_map(
    source: &str,
    highlighter: &mut Highlighter,
    use_color: bool,
) -> Result<RenderedJson> {
    let mut rows: Vec<String> = Vec::new();
    let mut map: Vec<(usize, usize)> = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let source_line = idx + 1;
        let row_start = rows.len();
        match prettify_line(line) {
            Ok(pretty) => {
                for pline in pretty.lines() {
                    let row = if use_color {
                        let h = highlighter.highlight_line(&format!("{pline}\n"))?;
                        h.trim_end_matches('\n').to_string()
                    } else {
                        pline.to_string()
                    };
                    rows.push(row);
                }
            }
            Err(_) => {
                let row = if use_color {
                    format!("{DIM}{line}{RESET}")
                } else {
                    line.to_string()
                };
                rows.push(row);
            }
        }
        // Defensive: guarantee every source line owns at least one row so the
        // map stays in lock-step with later lines.
        if rows.len() == row_start {
            rows.push(String::new());
        }
        map.push((row_start, source_line));
    }

    Ok(RenderedJson {
        text: rows.join("\n"),
        map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::{resolve_theme, theme_set, Highlighter};

    fn json_highlighter<'a>(
        ss: &'a syntect::parsing::SyntaxSet,
        ts: &'a syntect::highlighting::ThemeSet,
    ) -> Highlighter<'a> {
        let theme = resolve_theme(ts, None);
        let syntax = ss.find_syntax_by_token("json").unwrap();
        Highlighter::new(syntax, theme, ss)
    }

    #[test]
    fn prettify_expands_and_preserves_key_order() {
        let out = prettify_line(r#"{"b":1,"a":2}"#).unwrap();
        assert!(out.contains('\n'), "expected multi-line output: {out:?}");
        let b = out.find("\"b\"").unwrap();
        let a = out.find("\"a\"").unwrap();
        assert!(b < a, "key order not preserved: {out:?}");
        assert!(out.contains("  \"b\": 1"), "expected 2-space indent: {out:?}");
    }

    #[test]
    fn prettify_rejects_non_json() {
        assert!(prettify_line("not json at all").is_err());
        assert!(prettify_line("").is_err());
    }

    #[test]
    fn render_with_map_expands_and_maps() {
        let ss = crate::syntax::build_syntax_set().unwrap();
        let ts = theme_set();
        let mut hl = json_highlighter(&ss, &ts);
        let r = render_with_map("{\"a\":1}\nnotjson\n", &mut hl, false).unwrap();
        // {"a":1} pretty-prints to 3 rows; "notjson" passes through as 1 row.
        assert_eq!(r.map, vec![(0, 1), (3, 2)], "map: {:?}", r.map);
        assert!(r.text.contains("  \"a\": 1"), "expanded object missing: {:?}", r.text);
        assert!(r.text.contains("notjson"), "passthrough missing: {:?}", r.text);
    }

    #[test]
    fn render_with_map_dims_malformed_lines() {
        let ss = crate::syntax::build_syntax_set().unwrap();
        let ts = theme_set();
        let mut hl = json_highlighter(&ss, &ts);
        let r = render_with_map("notjson\n", &mut hl, true).unwrap();
        assert!(r.text.contains("\x1b[2m"), "malformed line should be DIM: {:?}", r.text);
    }

    #[test]
    fn is_jsonl_path_matches() {
        use std::path::PathBuf;
        assert!(is_jsonl_path(&PathBuf::from("a.jsonl")));
        assert!(is_jsonl_path(&PathBuf::from("a.ndjson")));
        assert!(is_jsonl_path(&PathBuf::from("A.JSONL")));
        assert!(!is_jsonl_path(&PathBuf::from("a.json")));
        assert!(!is_jsonl_path(&PathBuf::from("a.txt")));
    }
}
