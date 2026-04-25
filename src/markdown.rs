use std::path::Path;

/// Render a Markdown source string to ANSI-escaped output, sized for the
/// given terminal width. Returns the rendered text ready to write to stdout.
///
/// Uses termimad's default skin. The skin is built once per call (cheap) so
/// the function is suitable for use in interactive mode where it may be
/// called every frame.
pub fn render_to_string(source: &str, width: usize) -> String {
    let skin = termimad::MadSkin::default();
    let w = width.max(20) as u16;
    skin.text(source, Some(w as usize)).to_string()
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
        // termimad emits ANSI escapes for headings + bold.
        assert!(out.contains("\x1b["), "expected ANSI in: {:?}", out);
        // The heading text and the body should both survive.
        assert!(out.contains("Hello"));
        assert!(out.contains("bold"));
    }

    #[test]
    fn render_handles_empty_input() {
        let out = render_to_string("", 80);
        // Empty input is fine; termimad produces a small amount of output (or none).
        let _ = out;
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
