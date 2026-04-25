use crate::cli::Cli;
use crate::highlight::{Highlighter, resolve_theme};
use crate::input::{InputKind, LineRange};
use crate::printer::{PrinterConfig, StyleFlags, print};
use crate::syntax;
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Entry point for `--follow` / `-f`. Renders the last `tail_lines` lines of
/// the file, then polls for new content, rendering appended lines as they
/// arrive. Ctrl-C terminates the process via the default signal handler.
pub fn run(
    args: &Cli,
    syntax_set: &syntect::parsing::SyntaxSet,
    theme_set: &syntect::highlighting::ThemeSet,
) -> Result<()> {
    if args.files.len() > 1 {
        anyhow::bail!("follow mode supports a single file");
    }
    let path: PathBuf = match args.files.first() {
        Some(p) if p.as_os_str() != "-" => p.clone(),
        _ => anyhow::bail!("follow mode requires a file path"),
    };
    // (--interactive vs --follow exclusion is enforced in main::run.)

    let input = InputKind::File(path.clone());
    let theme = resolve_theme(theme_set, args.theme.as_deref());

    // Decide color once. Follow mode bypasses the pager so the original
    // stdout TTY-ness is the right signal.
    let use_color = match args.color {
        crate::cli::ColorWhen::Always => true,
        crate::cli::ColorWhen::Never => false,
        crate::cli::ColorWhen::Auto => {
            use std::io::IsTerminal;
            std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
        }
    };

    // Resolve --style and --decorations exactly like the static path does.
    let force_plain = args.plain
        || matches!(args.decorations, crate::cli::DecorationsWhen::Never);
    let style = StyleFlags::parse(&args.style, force_plain, args.number, args.diff);
    let highlight_lines: std::collections::HashSet<usize> = args.highlight_line.iter().copied().collect();
    let cursor: Option<usize> = args.highlight_line.first().copied();
    let width = crate::term_width();

    let mut stdout = std::io::stdout().lock();

    // First render: header + last tail_lines lines.
    let contents = fs::read_to_string(&path)?;
    let total = contents.lines().count();
    let first_line = contents.lines().next();
    let syntax = syntax::detect_syntax(syntax_set, Some(&path), args.language.as_deref(), first_line);

    let start = total.saturating_sub(args.tail_lines).saturating_add(1).max(1);
    let initial_range = LineRange { start, end: usize::MAX };

    {
        let mut hl = Highlighter::new(syntax, theme, syntax_set);
        let cfg = PrinterConfig {
            style,
            line_range: Some(initial_range),
            highlight_lines: highlight_lines.clone(),
            tabs: args.tabs,
            wrap: args.wrap,
            show_all: args.show_all,
            use_color,
            width,
            language_name: &syntax.name,
            cursor,
            line_numbers: args.line_numbers,
            markdown: false, // markdown rendering doesn't compose with streaming
        };
        print(&mut stdout, &input, &contents, &mut hl, &cfg)?;
        stdout.flush()?;
    }

    let mut last_lineno = total;
    let mut last_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    // Poll loop.
    loop {
        sleep(POLL_INTERVAL);
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue, // file vanished briefly (rotation in progress); keep waiting
        };
        let size = meta.len();
        if size == last_size {
            continue;
        }
        if size < last_size {
            // Truncation / rotation: reset and re-render the last tail_lines.
            writeln!(stdout, "--- file truncated, restarting ---")?;
            stdout.flush()?;
            let contents = fs::read_to_string(&path).unwrap_or_default();
            let total = contents.lines().count();
            let start = total.saturating_sub(args.tail_lines).saturating_add(1).max(1);
            let mut hl = Highlighter::new(syntax, theme, syntax_set);
            let cfg = PrinterConfig {
                style: StyleFlags { header: false, ..style },
                line_range: Some(LineRange { start, end: usize::MAX }),
                highlight_lines: highlight_lines.clone(),
                tabs: args.tabs,
                wrap: args.wrap,
                show_all: args.show_all,
                use_color,
                width,
                language_name: &syntax.name,
                cursor,
                line_numbers: args.line_numbers,
                markdown: false,
            };
            print(&mut stdout, &input, &contents, &mut hl, &cfg)?;
            stdout.flush()?;
            last_lineno = total;
            last_size = size;
            continue;
        }
        // size grew: render the new lines.
        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let total = contents.lines().count();
        if total > last_lineno {
            let mut hl = Highlighter::new(syntax, theme, syntax_set);
            let cfg = PrinterConfig {
                style: StyleFlags { header: false, ..style },
                line_range: Some(LineRange { start: last_lineno + 1, end: usize::MAX }),
                highlight_lines: highlight_lines.clone(),
                tabs: args.tabs,
                wrap: args.wrap,
                show_all: args.show_all,
                use_color,
                width,
                language_name: &syntax.name,
                cursor,
                line_numbers: args.line_numbers,
                markdown: false,
            };
            print(&mut stdout, &input, &contents, &mut hl, &cfg)?;
            stdout.flush()?;
            last_lineno = total;
        }
        last_size = size;
    }
}
