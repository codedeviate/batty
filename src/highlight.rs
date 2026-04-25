use anyhow::Result;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::as_24_bit_terminal_escaped;

pub struct Highlighter<'a> {
    inner: HighlightLines<'a>,
    set: &'a SyntaxSet,
    bg: bool,
}

impl<'a> Highlighter<'a> {
    pub fn new(syntax: &'a SyntaxReference, theme: &'a Theme, set: &'a SyntaxSet) -> Self {
        Self {
            inner: HighlightLines::new(syntax, theme),
            set,
            bg: false,
        }
    }

    /// Highlight one line and return ANSI-escaped string (no trailing newline added).
    pub fn highlight_line(&mut self, line: &str) -> Result<String> {
        let regions: Vec<(Style, &str)> = self.inner.highlight_line(line, self.set)?;
        Ok(as_24_bit_terminal_escaped(&regions[..], self.bg))
    }
}

/// Build the theme set bat ships, via two-face.
pub fn theme_set() -> ThemeSet {
    ThemeSet::from(two_face::theme::extra())
}

/// Default theme name (matches bat's default).
pub const DEFAULT_THEME: &str = "Monokai Extended";

/// Resolve the user's theme name, falling back to DEFAULT_THEME.
pub fn resolve_theme<'a>(set: &'a ThemeSet, name: Option<&str>) -> &'a Theme {
    let key = name.unwrap_or(DEFAULT_THEME);
    set.themes
        .get(key)
        .or_else(|| set.themes.get(DEFAULT_THEME))
        .or_else(|| set.themes.values().next())
        .expect("at least one theme is bundled")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::build_syntax_set;

    #[test]
    fn highlights_rust_line() {
        let set = build_syntax_set().unwrap();
        let themes = theme_set();
        let theme = resolve_theme(&themes, None);
        let syntax = set.find_syntax_by_extension("rs").unwrap();
        let mut h = Highlighter::new(syntax, theme, &set);
        let out = h.highlight_line("fn main() {}\n").unwrap();
        // Output contains ANSI escapes and the original text.
        assert!(out.contains("fn"));
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn default_theme_resolves() {
        let themes = theme_set();
        let _ = resolve_theme(&themes, None);
    }
}
