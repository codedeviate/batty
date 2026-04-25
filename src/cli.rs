use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "batty", version, about = "A cat clone with syntax highlighting and Rhai support")]
pub struct Cli {
    /// Files to display (use - for stdin)
    pub files: Vec<PathBuf>,

    /// Set the language for syntax highlighting
    #[arg(short = 'l', long)]
    pub language: Option<String>,

    /// Set the color theme
    #[arg(long)]
    pub theme: Option<String>,

    /// When to use colors
    #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
    pub color: ColorWhen,

    /// When to use the pager
    #[arg(long, value_enum, default_value_t = PagingWhen::Auto)]
    pub paging: PagingWhen,

    /// Plain output (no decorations, equivalent to --style=plain)
    #[arg(short = 'p', long, overrides_with = "plain")]
    pub plain: bool,

    /// Show non-printable characters
    #[arg(short = 'A', long, overrides_with = "show_all")]
    pub show_all: bool,

    /// Show line numbers (equivalent to --style=numbers)
    #[arg(short = 'n', long, overrides_with = "number")]
    pub number: bool,

    /// Show git diff markers
    #[arg(short = 'd', long, overrides_with = "diff")]
    pub diff: bool,

    /// Lines of context for diff
    #[arg(long, default_value_t = 2)]
    pub diff_context: usize,

    /// Style components, comma-separated: full, plain, numbers, grid, header, rule, changes, snip
    #[arg(long, default_value = "full")]
    pub style: String,

    /// Display only specified line range, e.g. 10:20, :15, 30:
    #[arg(long)]
    pub line_range: Option<String>,

    /// Highlight specific line(s)
    #[arg(short = 'H', long)]
    pub highlight_line: Vec<usize>,

    /// Tab width
    #[arg(long, default_value_t = 4)]
    pub tabs: usize,

    /// Wrap mode
    #[arg(long, value_enum, default_value_t = WrapMode::Auto)]
    pub wrap: WrapMode,

    /// List supported languages and exit
    #[arg(short = 'L', long, overrides_with = "list_languages")]
    pub list_languages: bool,

    /// List supported themes and exit
    #[arg(long, overrides_with = "list_themes")]
    pub list_themes: bool,

    /// Decoration mode override
    #[arg(long, value_enum, default_value_t = DecorationsWhen::Auto)]
    pub decorations: DecorationsWhen,

    /// Line numbering style
    #[arg(long, value_enum, default_value_t = LineNumberStyle::Absolute)]
    pub line_numbers: LineNumberStyle,

    /// Enter interactive TUI mode (vim-style navigation: j/k, g/G, Ctrl-d/u, q to quit)
    #[arg(short = 'i', long, overrides_with = "no_interactive")]
    pub interactive: bool,

    /// Disable interactive mode. Overrides `interactive = true` in the config file.
    #[arg(long, overrides_with = "interactive")]
    pub no_interactive: bool,

    /// Reserve N rows at the top of the screen in interactive mode. Useful for
    /// terminals like Warp that overlay UI on the alternate screen's top rows.
    #[arg(long, default_value_t = 0)]
    pub top_pad: u16,
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

impl Cli {
    /// Parse from a custom args list (used for config-file merging)
    pub fn parse_from_args<I, T>(args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Cli::parse_from(args)
    }
}
