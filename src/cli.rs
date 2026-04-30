use clap::{Parser, ValueEnum};
use std::path::PathBuf;

// NOTE: Flag fields are kept in alphabetical order by their long-flag name
// (e.g. --color before --decorations before --diff). Short flags ride along
// wherever their long form lands. See CLAUDE.md "Conventions" for the rule.

#[derive(Parser, Debug, Clone)]
#[command(name = "batty", version, about = "A cat clone with syntax highlighting and Rhai support")]
pub struct Cli {
    /// Files to display (use - for stdin)
    pub files: Vec<PathBuf>,

    /// When to use colors
    #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
    pub color: ColorWhen,

    /// Decoration mode override
    #[arg(long, value_enum, default_value_t = DecorationsWhen::Auto)]
    pub decorations: DecorationsWhen,

    /// Show git diff markers
    #[arg(short = 'd', long, overrides_with = "diff")]
    pub diff: bool,

    /// Lines of context for diff
    #[arg(long, default_value_t = 2)]
    pub diff_context: usize,

    /// Input file encoding. `auto` tries UTF-8 and falls back to ISO-8859-1
    /// if UTF-8 decoding fails.
    #[arg(long, value_enum, default_value_t = Encoding::Auto)]
    pub encoding: Encoding,

    /// Tail mode: show the last lines of the file and keep watching for new
    /// content (like `tail -f`). Single file only; no stdin.
    #[arg(short = 'f', long, overrides_with = "no_follow")]
    pub follow: bool,

    /// Show the left-side gutter (line numbers, diff markers, grid bar).
    /// Cancels `--no-gutter` from config; otherwise inert (defers to --style).
    #[arg(long, overrides_with = "no_gutter")]
    pub gutter: bool,

    /// Highlight specific line(s)
    #[arg(short = 'H', long)]
    pub highlight_line: Vec<usize>,

    /// Enter interactive TUI mode (vim-style navigation: j/k, g/G, Ctrl-d/u, q to quit)
    #[arg(short = 'i', long, overrides_with = "no_interactive")]
    pub interactive: bool,

    /// Set the language for syntax highlighting
    #[arg(short = 'l', long)]
    pub language: Option<String>,

    /// Line numbering style
    #[arg(long, value_enum, default_value_t = LineNumberStyle::Absolute)]
    pub line_numbers: LineNumberStyle,

    /// Display only specified line range, e.g. 10:20, :15, 30:
    #[arg(long)]
    pub line_range: Option<String>,

    /// List supported languages and exit
    #[arg(short = 'L', long, overrides_with = "list_languages")]
    pub list_languages: bool,

    /// List supported themes and exit
    #[arg(long, overrides_with = "list_themes")]
    pub list_themes: bool,

    /// Render Markdown files instead of showing the raw source. Works for any
    /// file but most useful for .md / .markdown.
    #[arg(short = 'm', long, overrides_with = "no_markdown")]
    pub markdown: bool,

    /// Render markdown files (.md / .markdown / .mdown / .mkd) automatically,
    /// but leave other files as raw highlighted source. Lower priority than
    /// --markdown (force on for any file) and --no-markdown (force off).
    #[arg(long, overrides_with = "markdown_on_extension")]
    pub markdown_on_extension: bool,

    /// Disable follow mode. Overrides `follow = true` in the config.
    #[arg(long, overrides_with = "follow")]
    pub no_follow: bool,

    /// Hide the gutter (line numbers, diff markers, grid bar) regardless of
    /// --style. Header / rule / snip stay on. A "cleaner reading" preset
    /// that's less aggressive than --plain.
    #[arg(long, overrides_with = "gutter")]
    pub no_gutter: bool,

    /// Disable interactive mode. Overrides `interactive = true` in the config file.
    #[arg(long, overrides_with = "interactive")]
    pub no_interactive: bool,

    /// Disable Markdown rendering. Overrides `markdown = true` in the config.
    #[arg(long, overrides_with = "markdown")]
    pub no_markdown: bool,

    /// Show line numbers (equivalent to --style=numbers)
    #[arg(short = 'n', long, overrides_with = "number")]
    pub number: bool,

    /// When to use the pager. `never` also disables interactive mode (treats
    /// the flag as a global "flat output" override).
    #[arg(long, value_enum, default_value_t = PagingWhen::Auto)]
    pub paging: PagingWhen,

    /// Plain output (no decorations, equivalent to --style=plain)
    #[arg(short = 'p', long, overrides_with = "plain")]
    pub plain: bool,

    /// Show non-printable characters
    #[arg(short = 'A', long, overrides_with = "show_all")]
    pub show_all: bool,

    /// Style components, comma-separated: full, plain, numbers, grid, header, rule, changes, snip
    #[arg(long, default_value = "full")]
    pub style: String,

    /// Tab width
    #[arg(long, default_value_t = 4)]
    pub tabs: usize,

    /// Number of trailing lines to show in follow mode.
    #[arg(long, default_value_t = 10)]
    pub tail_lines: usize,

    /// Set the color theme
    #[arg(long)]
    pub theme: Option<String>,

    /// Reserve N rows at the top of the screen in interactive mode. Useful for
    /// terminals like Warp that overlay UI on the alternate screen's top rows.
    #[arg(long, default_value_t = 0)]
    pub top_pad: u16,

    /// Wrap mode
    #[arg(long, value_enum, default_value_t = WrapMode::Auto)]
    pub wrap: WrapMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorWhen { Always, Auto, Never }

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PagingWhen { Always, Auto, Never }

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WrapMode { Never, Character, Auto }

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DecorationsWhen { Always, Auto, Never }

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LineNumberStyle { Absolute, Relative }

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Encoding {
    /// Try UTF-8 first; fall back to ISO-8859-1 on decode failure.
    Auto,
    /// Strict UTF-8. Errors out on invalid byte sequences.
    #[value(name = "utf-8", alias = "utf8")]
    Utf8,
    /// ISO-8859-1 (Latin-1). Each byte maps to its matching Unicode code point.
    #[value(name = "iso-8859-1", alias = "latin1", alias = "latin-1")]
    Latin1,
}
