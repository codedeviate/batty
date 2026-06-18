//! Colorized `--examples` output. Curated scenarios for batty's flags.
//!
//! Output is plain ANSI escapes — no extra dep. Honors `NO_COLOR` and
//! non-TTY stdout (pipes / redirects) by stripping styling.

use std::io::{self, IsTerminal, Write};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

pub fn print() {
    let use_color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let mut out = io::stdout().lock();
    let _ = render(&mut out, use_color);
}

fn render<W: Write>(w: &mut W, c: bool) -> io::Result<()> {
    writeln!(w)?;
    writeln!(w, "{}", paint(c, &[BOLD], "batty — usage examples"))?;
    writeln!(w)?;

    section(w, c, "BASIC USAGE")?;

    example(w, c, "Show a file with syntax highlighting", &[
        "batty src/main.rs",
        "batty README.md",
    ])?;
    example(w, c, "Show multiple files (inter-file rule with --style=rule)", &[
        "batty src/cli.rs src/main.rs",
        "batty --style=full,rule src/*.rs",
    ])?;
    example(w, c, "Read from stdin (pass `-` or pipe in)", &[
        "cat README.md | batty -l md",
        "echo 'fn main(){}' | batty -l rust",
    ])?;

    section(w, c, "STYLE / DECORATIONS")?;

    example(w, c, "Plain output (no decorations, like classic `cat`)", &[
        "batty -p src/main.rs",
        "batty --decorations=never src/main.rs",
    ])?;
    example(w, c, "Show line numbers (shorthand for --style=numbers)", &[
        "batty -n src/main.rs",
    ])?;
    example(w, c, "Pick exactly which style components to show", &[
        "batty --style=numbers,grid src/main.rs",
        "batty --style=header,changes src/main.rs",
        "batty --style=full,rule src/*.rs",
    ])?;
    note(w, c, "Components: full, plain, numbers, grid, header, rule, changes, snip.")?;
    example(w, c, "Hide the gutter without going fully plain", &[
        "batty --no-gutter README.md",
    ])?;
    example(w, c, "Relative line numbers (vim-style, centered on cursor)", &[
        "batty --line-numbers=relative -H 42 src/main.rs",
    ])?;
    example(w, c, "Highlight specific line(s)", &[
        "batty -H 10 src/main.rs",
        "batty -H 10 -H 20 -H 30 src/main.rs",
    ])?;
    example(w, c, "Render only a line range", &[
        "batty --line-range=10:20 src/main.rs",
        "batty --line-range=:50 src/main.rs   # head",
        "batty --line-range=100: src/main.rs  # tail-to-end",
    ])?;

    section(w, c, "SYNTAX / THEMES")?;

    example(w, c, "Force a specific language", &[
        "batty -l rust unknown.txt",
        "batty -l rhai script.txt",
        "batty -l md notes.txt",
    ])?;
    example(w, c, "Pick a color theme", &[
        "batty --theme=Dracula src/main.rs",
        "batty --theme='Solarized (dark)' src/main.rs",
    ])?;
    example(w, c, "List available languages / themes and exit", &[
        "batty -L",
        "batty --list-themes",
    ])?;

    section(w, c, "MARKDOWN")?;

    example(w, c, "Render Markdown to terminal escapes", &[
        "batty -m README.md",
    ])?;
    example(w, c, "Auto-render only files with markdown extensions", &[
        "batty --markdown-on-extension README.md src/main.rs",
    ])?;
    note(w, c, "Affects .md / .markdown / .mdown / .mkd; other files stay raw.")?;
    example(w, c, "Force raw view for a .md file (overrides config)", &[
        "batty --no-markdown README.md",
    ])?;

    section(w, c, "GIT DIFF")?;

    example(w, c, "Show diff markers vs HEAD in the gutter", &[
        "batty -d src/main.rs",
        "batty --style=full,changes src/main.rs",
    ])?;
    note(w, c, "Markers: ┃ added, ┃ modified (yellow), ┃ removed (red). HEAD is the comparison ref.")?;

    section(w, c, "TAIL / FOLLOW")?;

    example(w, c, "Follow a log file with syntax highlighting", &[
        "batty -f /var/log/syslog",
        "batty --follow --tail-lines=50 server.log",
    ])?;
    note(w, c, "Single file only. Disable with --no-follow to override `follow = true` in config.")?;

    section(w, c, "INTERACTIVE TUI")?;

    example(w, c, "Launch the interactive viewer", &[
        "batty -i src/main.rs",
    ])?;
    note(w, c, "Keys: j/k scroll, g/G top/bottom, Ctrl-d/u half-page, q to quit, m to toggle markdown, n to toggle gutter, +/- adjust top-pad.")?;
    example(w, c, "Reserve top rows (useful for Warp's overlay)", &[
        "batty -i --top-pad=2 src/main.rs",
    ])?;
    example(w, c, "Live mode: same TUI, autoreload on any file change", &[
        "batty --live src/lib.rs",
        "batty --live ~/.config/batty/config.toml",
    ])?;
    note(w, c, "Single file only. Status bar shows [live · reloaded] after each reload. Use --no-live to override `live = true` in config.")?;
    example(w, c, "Live-tail a log with soft-wrap on from the start", &[
        "batty --live --wrap=character app.log",
    ])?;
    note(w, c, "Press w to toggle wrapping on/off at any time. Status bar shows `wrap` while on.")?;

    section(w, c, "ENCODING / WRAPPING")?;

    example(w, c, "Override input encoding", &[
        "batty --encoding=utf-8 strict.txt",
        "batty --encoding=iso-8859-1 legacy.log",
    ])?;
    note(w, c, "Default `auto` tries UTF-8, falls back to ISO-8859-1 on decode failure.")?;
    example(w, c, "Force wrap mode", &[
        "batty --wrap=character long-lines.txt",
        "batty --wrap=word      long-paragraph.md",
        "batty --wrap=never     long-lines.txt",
    ])?;
    note(w, c, "`word` breaks at whitespace; words longer than the wrap width fall back to character-break.")?;

    section(w, c, "PAGER / OUTPUT")?;

    example(w, c, "Disable the pager (flat output, even on a TTY)", &[
        "batty --paging=never README.md",
    ])?;
    note(w, c, "Also disables interactive mode for that run; respects $PAGER and $LESS otherwise.")?;
    example(w, c, "Force color when piping to another command", &[
        "batty --color=always src/main.rs | less -R",
    ])?;
    example(w, c, "Show non-printable characters", &[
        "batty -A weird.bin",
    ])?;
    example(w, c, "Use a custom tab width", &[
        "batty --tabs=2 Makefile",
    ])?;

    section(w, c, "CONFIG / META")?;

    example(w, c, "Run without any config (hermetic)", &[
        "BATTY_CONFIG_PATH=/dev/null batty README.md",
    ])?;
    note(w, c, "Default config path is ~/.config/batty/config.toml. Set BATTY_CONFIG_PATH to a different file or /dev/null to opt out.")?;
    example(w, c, "Show curated usage examples (this screen)", &[
        "batty --examples",
    ])?;

    writeln!(w)?;
    Ok(())
}

fn section<W: Write>(w: &mut W, c: bool, title: &str) -> io::Result<()> {
    writeln!(w, "  {}", paint(c, &[BOLD, YELLOW], title))?;
    writeln!(w)
}

fn example<W: Write>(w: &mut W, c: bool, desc: &str, commands: &[&str]) -> io::Result<()> {
    writeln!(w, "    {}", paint(c, &[BOLD], desc))?;
    for cmd in commands {
        writeln!(w, "      {}", paint(c, &[CYAN], cmd))?;
    }
    writeln!(w)
}

fn note<W: Write>(w: &mut W, c: bool, text: &str) -> io::Result<()> {
    writeln!(w, "    {} {}", paint(c, &[DIM, BOLD], "note:"), paint(c, &[DIM], text))?;
    writeln!(w)
}

fn paint(use_color: bool, codes: &[&str], s: &str) -> String {
    if !use_color {
        return s.to_string();
    }
    let mut out = String::new();
    for code in codes {
        out.push_str(code);
    }
    out.push_str(s);
    out.push_str(RESET);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_without_panicking_uncolored() {
        let mut buf: Vec<u8> = Vec::new();
        render(&mut buf, false).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(s.contains("batty — usage examples"));
        assert!(s.contains("BASIC USAGE"));
        assert!(!s.contains("\x1b["), "uncolored output must not contain ANSI escapes");
    }

    #[test]
    fn renders_with_color_escapes() {
        let mut buf: Vec<u8> = Vec::new();
        render(&mut buf, true).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(s.contains("\x1b["), "colored output must include ANSI escapes");
        assert!(s.contains("\x1b[0m"), "must reset color");
    }
}
