use anyhow::{Context, Result};
use std::path::Path;
use syntect::parsing::{SyntaxReference, SyntaxSet, SyntaxSetBuilder};

/// Build a SyntaxSet that combines two-face's extended grammar pack with the bundled Rhai grammar.
pub fn build_syntax_set() -> Result<SyntaxSet> {
    let extra = two_face::syntax::extra_newlines();
    let mut builder: SyntaxSetBuilder = extra.into_builder();

    // Bundled Rhai grammar in .sublime-syntax (YAML) format.
    const RHAI_GRAMMAR: &str = include_str!("../assets/rhai.sublime-syntax");
    let def = syntect::parsing::SyntaxDefinition::load_from_str(
        RHAI_GRAMMAR,
        true,
        Some("Rhai"),
    )
    .context("failed to parse bundled Rhai grammar")?;
    builder.add(def);

    Ok(builder.build())
}

/// Detect a syntax for the given file path or language hint. Falls back to "Plain Text".
pub fn detect_syntax<'a>(
    set: &'a SyntaxSet,
    path: Option<&Path>,
    language_hint: Option<&str>,
    first_line: Option<&str>,
) -> &'a SyntaxReference {
    if let Some(lang) = language_hint {
        if let Some(s) = set.find_syntax_by_token(lang) {
            return s;
        }
    }
    if let Some(p) = path {
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if let Some(s) = set.find_syntax_by_extension(ext) {
                return s;
            }
            // JSONL / NDJSON have no dedicated grammar; every line is itself
            // valid JSON, so highlight them with the JSON grammar.
            if matches!(ext.to_ascii_lowercase().as_str(), "jsonl" | "ndjson") {
                if let Some(s) = set.find_syntax_by_token("json") {
                    return s;
                }
            }
        }
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if let Some(s) = set.find_syntax_by_token(name) {
                return s;
            }
        }
    }
    if let Some(line) = first_line {
        if let Some(s) = set.find_syntax_by_first_line(line) {
            return s;
        }
    }
    set.find_syntax_plain_text()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_by_extension() {
        let set = build_syntax_set().unwrap();
        let s = detect_syntax(&set, Some(Path::new("foo.rs")), None, None);
        assert_eq!(s.name, "Rust");
    }

    #[test]
    fn detects_rhai_by_extension() {
        let set = build_syntax_set().unwrap();
        let s = detect_syntax(&set, Some(Path::new("script.rhai")), None, None);
        assert!(s.name.to_lowercase().contains("rhai"), "got: {}", s.name);
    }

    #[test]
    fn falls_back_to_plain_text() {
        let set = build_syntax_set().unwrap();
        let s = detect_syntax(&set, Some(Path::new("noext")), None, None);
        assert_eq!(s.name, "Plain Text");
    }

    #[test]
    fn language_hint_overrides_extension() {
        let set = build_syntax_set().unwrap();
        let s = detect_syntax(&set, Some(Path::new("foo.rs")), Some("python"), None);
        assert_eq!(s.name, "Python");
    }

    #[test]
    fn detects_jsonl_as_json() {
        let set = build_syntax_set().unwrap();
        let json = set.find_syntax_by_token("json").unwrap();
        for name in ["data.jsonl", "logs.ndjson", "UPPER.JSONL"] {
            let s = detect_syntax(&set, Some(std::path::Path::new(name)), None, None);
            assert_eq!(s.name, json.name, "{name} should resolve to JSON, got {}", s.name);
        }
    }
}
