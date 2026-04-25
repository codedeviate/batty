mod cli;
mod config;
mod git;
mod highlight;
mod input;
mod interactive;
mod markdown;
mod pager;
mod printer;
mod syntax;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, ColorWhen};
use highlight::{Highlighter, resolve_theme, theme_set};
use input::{InputKind, LineRange};
use printer::{PrinterConfig, StyleFlags, print};
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Reset SIGPIPE to its default disposition (terminate). Rust changes it
    // to ignore in libstd, which means writing to a closed pipe (e.g., when
    // the user pipes batty into `head -3`) returns an EPIPE error that gets
    // surfaced as `batty: error: ... Broken pipe`. Restoring SIG_DFL makes
    // the kernel terminate us silently when the reader goes away — same
    // behavior as `cat`.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(e) = run() {
        eprintln!("batty: error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    // Compose args: program name + config file args + cli args.
    let mut all: Vec<std::ffi::OsString> = std::env::args_os().take(1).collect();
    all.extend(config::load_args().into_iter().map(Into::into));
    all.extend(std::env::args_os().skip(1));
    let args = Cli::parse_from(all);

    let syntax_set = syntax::build_syntax_set()?;
    let theme_set = theme_set();

    // List-* short-circuits run before interactive so the config's
    // `interactive = true` doesn't block listing flags on the CLI.
    if args.list_languages {
        let mut names: Vec<&str> = syntax_set.syntaxes().iter().map(|s| s.name.as_str()).collect();
        names.sort_unstable();
        for n in names { println!("{}", n); }
        return Ok(());
    }
    if args.list_themes {
        let mut names: Vec<&String> = theme_set.themes.keys().collect();
        names.sort();
        for n in names { println!("{}", n); }
        return Ok(());
    }

    // --paging=never is treated as a "give me flat output" signal that
    // also disables interactive mode. This lets users with `interactive =
    // true` in their config bypass the TUI for a single run by passing
    // `--paging=never` without having to also remember `--no-interactive`.
    let want_interactive = args.interactive && args.paging != cli::PagingWhen::Never;
    if want_interactive {
        return run_interactive(&args, &syntax_set, &theme_set);
    }

    // Capture stdout TTY-ness BEFORE pager setup, since pager replaces stdout
    // with a pipe.
    let stdout_was_tty = {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal()
    };

    pager::setup(args.paging);

    // Color decision (uses pre-pager TTY status)
    let use_color = match args.color {
        ColorWhen::Always => true,
        ColorWhen::Never => false,
        ColorWhen::Auto => stdout_was_tty && std::env::var_os("NO_COLOR").is_none(),
    };

    // --decorations=never collapses to plain output (overrides --style)
    let force_plain = args.plain
        || matches!(args.decorations, cli::DecorationsWhen::Never);
    let style = StyleFlags::parse(&args.style, force_plain, args.number, args.diff);
    let theme = resolve_theme(&theme_set, args.theme.as_deref());
    let line_range = match &args.line_range {
        Some(s) => Some(LineRange::parse(s)?),
        None => None,
    };
    let highlight_lines: HashSet<usize> = args.highlight_line.iter().copied().collect();
    // First --highlight-line acts as the cursor reference for relative numbering in static mode.
    let cursor: Option<usize> = args.highlight_line.first().copied();
    let width = term_width();

    // Default to stdin when no files given
    let inputs: Vec<InputKind> = if args.files.is_empty() {
        vec![InputKind::Stdin]
    } else {
        args.files.iter().map(|p: &PathBuf| InputKind::from_path(p)).collect()
    };

    let mut stdout = std::io::stdout().lock();
    for (idx, input) in inputs.iter().enumerate() {
        // Inter-file rule: drawn before every input after the first when
        // `--style` includes `rule`. Reuses the grid glyph for visual parity.
        if idx > 0 && style.rule {
            writeln!(stdout, "{}", "─".repeat(width.saturating_sub(1)))?;
        }
        let contents = input.read()?;
        let path = match input { InputKind::File(p) => Some(p.as_path()), InputKind::Stdin => None };
        let first_line = contents.lines().next();
        let syntax = syntax::detect_syntax(&syntax_set, path, args.language.as_deref(), first_line);
        let mut hl = Highlighter::new(syntax, theme, &syntax_set);

        let cfg = PrinterConfig {
            style,
            line_range,
            highlight_lines: highlight_lines.clone(),
            tabs: args.tabs,
            wrap: args.wrap,
            show_all: args.show_all,
            use_color,
            width,
            language_name: &syntax.name,
            cursor,
            line_numbers: args.line_numbers,
            markdown: args.markdown,
        };
        print(&mut stdout, input, &contents, &mut hl, &cfg)?;
        stdout.flush()?;
    }
    Ok(())
}

fn run_interactive(
    args: &Cli,
    syntax_set: &syntect::parsing::SyntaxSet,
    theme_set: &syntect::highlighting::ThemeSet,
) -> Result<()> {
    if args.files.len() > 1 {
        anyhow::bail!("interactive mode supports a single file");
    }
    let path = match args.files.first() {
        Some(p) if p.as_os_str() != "-" => p.clone(),
        _ => anyhow::bail!("interactive mode requires a file path"),
    };
    let input = InputKind::from_path(&path);
    let contents = input.read()?;
    let first_line = contents.lines().next();
    let syntax = syntax::detect_syntax(syntax_set, Some(&path), args.language.as_deref(), first_line);
    let theme = resolve_theme(theme_set, args.theme.as_deref());

    // The `m` toggle is enabled when the file looks like markdown by extension,
    // OR the user explicitly forced --markdown (so they can flip back to raw).
    let is_md = markdown::is_markdown_path(&path);
    let can_toggle = is_md || args.markdown;

    interactive::run(
        &input.display_name(),
        &contents,
        syntax,
        syntax_set,
        theme,
        args.line_numbers,
        args.tabs,
        args.show_all,
        args.top_pad,
        args.markdown,
        can_toggle,
    )
}

fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
}
