mod cli;
mod config;
mod git;
mod highlight;
mod input;
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

    pager::setup(args.paging);

    // Color decision
    let use_color = match args.color {
        ColorWhen::Always => true,
        ColorWhen::Never => false,
        ColorWhen::Auto => {
            use std::io::IsTerminal;
            std::io::stdout().is_terminal()
                && std::env::var_os("NO_COLOR").is_none()
        }
    };

    let style = StyleFlags::parse(&args.style, args.plain, args.number, args.diff);
    let theme = resolve_theme(&theme_set, args.theme.as_deref());
    let line_range = match &args.line_range {
        Some(s) => Some(LineRange::parse(s)?),
        None => None,
    };
    let highlight_lines: HashSet<usize> = args.highlight_line.iter().copied().collect();
    let width = term_width();

    // Default to stdin when no files given
    let inputs: Vec<InputKind> = if args.files.is_empty() {
        vec![InputKind::Stdin]
    } else {
        args.files.iter().map(|p: &PathBuf| InputKind::from_path(p)).collect()
    };

    let mut stdout = std::io::stdout().lock();
    for input in &inputs {
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
        };
        print(&mut stdout, input, &contents, &mut hl, &cfg)?;
        stdout.flush()?;
    }
    Ok(())
}

fn term_width() -> usize {
    use std::process::Command;
    if let Ok(out) = Command::new("tput").arg("cols").output() {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Ok(n) = s.trim().parse::<usize>() { return n; }
            }
        }
    }
    80
}
