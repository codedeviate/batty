use std::process::Command;

/// Build a `batty` Command with a hermetic environment: ignores the user's
/// `~/.config/batty/config.toml` by pointing `BATTY_CONFIG_PATH` at /dev/null.
fn batty() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_batty"));
    c.env("BATTY_CONFIG_PATH", "/dev/null");
    c
}

#[test]
fn list_languages_includes_rhai() {
    let out = batty().arg("--list-languages").output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.to_lowercase().contains("rhai"), "languages output: {}", s);
}

#[test]
fn list_themes_works() {
    let out = batty().arg("--list-themes").output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(!s.is_empty());
}

#[test]
fn plain_mode_produces_no_decorations() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "hello\nworld\n").unwrap();
    let out = batty()
        .arg("--plain")
        .arg("--color=never")
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert_eq!(s, "hello\nworld\n");
}

#[test]
fn line_range_filters_output() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "a\nb\nc\nd\ne\n").unwrap();
    let out = batty()
        .args(["--plain", "--color=never", "--line-range", "2:3"])
        .arg(&f).output().unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert_eq!(s, "b\nc\n");
}

#[test]
fn stdin_with_language_hint() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = batty()
        .args(["--plain", "--color=never", "--language", "rust"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"fn main() {}\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert_eq!(s, "fn main() {}\n");
}

#[test]
fn relative_line_numbers_with_highlight_line() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "a\nb\nc\nd\ne\n").unwrap();
    let out = batty()
        .args([
            "--style=numbers",
            "--color=never",
            "--line-numbers=relative",
            "--highlight-line=3",
        ])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // Cursor (line 3) shows absolute "3"; lines 1,2,4,5 show distances 2,1,1,2.
    // The cursor-indicator gutter ▶ marks line 3.
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 5, "expected 5 body lines, got: {:?}", lines);
    assert!(lines[0].trim_start().starts_with("2 "), "line 1 should label 2: {}", lines[0]);
    assert!(lines[1].trim_start().starts_with("1 "), "line 2 should label 1: {}", lines[1]);
    assert!(lines[2].trim_start().starts_with("3 "), "line 3 should label 3 (cursor abs): {}", lines[2]);
    assert!(lines[2].contains('▶'), "line 3 should have cursor glyph: {}", lines[2]);
    assert!(lines[3].trim_start().starts_with("1 "), "line 4 should label 1: {}", lines[3]);
    assert!(lines[4].trim_start().starts_with("2 "), "line 5 should label 2: {}", lines[4]);
}

#[test]
fn relative_line_numbers_without_cursor_falls_back_to_absolute() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "a\nb\nc\n").unwrap();
    let out = batty()
        .args([
            "--style=numbers",
            "--color=never",
            "--line-numbers=relative",
        ])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // No --highlight-line provided => no cursor => fall back to absolute numbering.
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].trim_start().starts_with("1 "), "got: {}", lines[0]);
    assert!(lines[1].trim_start().starts_with("2 "), "got: {}", lines[1]);
    assert!(lines[2].trim_start().starts_with("3 "), "got: {}", lines[2]);
    // No cursor glyph should appear when cursor is None.
    assert!(!s.contains('▶'), "no cursor glyph expected: {}", s);
}

#[test]
fn markdown_flag_renders_md() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("doc.md");
    std::fs::write(&f, "# Title\n\nSome **bold** text.\n").unwrap();
    let out = batty()
        .args(["--markdown", "--style=plain", "--color=always"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    // Rendered output should contain the heading text and the bold word,
    // plus ANSI escapes (since --color=always).
    assert!(s.contains("Title"), "output: {:?}", s);
    assert!(s.contains("bold"), "output: {:?}", s);
    assert!(s.contains("\x1b["), "expected ANSI escapes in: {:?}", s);
    // The raw markdown markers should NOT appear unaltered (rendered output
    // strips the literal `#` prefix, though `**` may be retained or stripped
    // depending on termimad — assert at least the leading `# ` is gone).
    assert!(!s.contains("# Title"), "raw heading should not appear: {:?}", s);
}

#[test]
fn no_markdown_overrides_config() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    std::fs::write(&cfg, "markdown = true\n").unwrap();
    let f = dir.path().join("doc.md");
    std::fs::write(&f, "# Title\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_batty"))
        .env("BATTY_CONFIG_PATH", &cfg)
        .args(["--no-markdown", "--plain", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // With --no-markdown overriding the config, output is the raw source.
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "# Title\n");
}

#[test]
fn rule_separates_multiple_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    std::fs::write(&a, "alpha\n").unwrap();
    std::fs::write(&b, "bravo\n").unwrap();
    let out = batty()
        .args(["--style=rule,header,numbers", "--color=never"])
        .arg(&a)
        .arg(&b)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // The rule glyph appears between the two files' renders.
    assert!(s.contains('─'), "expected rule glyph in: {}", s);
    // Both files' content is present.
    assert!(s.contains("alpha"));
    assert!(s.contains("bravo"));
}

#[test]
fn snip_marks_clipped_line_range() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    let body: String = (1..=20).map(|n| format!("line {}\n", n)).collect();
    std::fs::write(&f, body).unwrap();
    let out = batty()
        .args([
            "--style=snip,numbers",
            "--color=never",
            "--line-range",
            "5:10",
        ])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // Snip prefix: 4 lines skipped above
    assert!(s.contains("4 lines skipped"), "missing prefix snip: {}", s);
    // Snip suffix: 10 lines skipped below
    assert!(s.contains("10 lines skipped"), "missing suffix snip: {}", s);
    // Visible lines 5..=10 are present
    assert!(s.contains("line 5"));
    assert!(s.contains("line 10"));
}

#[test]
fn rhai_range_bitwise_and_nested_comments_render() {
    // Ensures the grammar still parses (and produces ANSI) when the source
    // exercises the v0.4.0 grammar additions: range/bitwise ops + nested
    // block comments. Failure mode would be syntect's runtime parser error.
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("t.rhai");
    std::fs::write(
        &f,
        "let r = 1..=10;\n\
         let m = a & b | c ^ d;\n\
         let s = x << 2 | y >> 1;\n\
         /* outer /* inner */ outer */\n\
         fn x() {}\n",
    )
    .unwrap();
    let out = batty()
        .args(["--plain", "--color=always"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    // Output contains ANSI escapes (grammar produced colored tokens).
    assert!(s.contains("\x1b["), "expected ANSI in: {:?}", s);
    // Strip ANSI to assert source content survived rendering. Tokens get
    // separated by escape sequences in colored output, so we compare
    // against the de-escaped text.
    let stripped: String = {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // skip until 'm' (end of CSI)
                for c2 in chars.by_ref() {
                    if c2 == 'm' { break; }
                }
            } else {
                out.push(c);
            }
        }
        out
    };
    assert!(stripped.contains("1..=10"), "stripped: {:?}", stripped);
    assert!(stripped.contains("inner"), "stripped: {:?}", stripped);
}

#[test]
fn paging_never_overrides_interactive_config() {
    // With `interactive = true` in config, `--paging=never` should be
    // treated as a 'flat output' signal that also disables interactive mode.
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    std::fs::write(&cfg, "interactive = true\n").unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "hello\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_batty"))
        .env("BATTY_CONFIG_PATH", &cfg)
        .args(["--paging=never", "--plain", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "hello\n");
}

#[test]
fn no_interactive_overrides_config() {
    // Simulate a user config that sets interactive = true; --no-interactive on
    // the CLI must override and let the static path render normally.
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    std::fs::write(&cfg, "interactive = true\n").unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "hello\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_batty"))
        .env("BATTY_CONFIG_PATH", &cfg)
        .args(["--no-interactive", "--plain", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "hello\n");
}

#[test]
fn duplicate_bool_flags_do_not_conflict() {
    // Config can emit a bool flag that's also passed on the CLI; clap must
    // treat the second occurrence as an override, not an error.
    let out = batty()
        .args(["--list-languages", "--list-languages"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "duplicate --list-languages should not error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.to_lowercase().contains("rhai"));
}

#[test]
fn interactive_rejects_multiple_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    std::fs::write(&a, "x").unwrap();
    std::fs::write(&b, "y").unwrap();
    let out = batty().arg("-i").arg(&a).arg(&b).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.contains("single file"),
        "expected single-file error, got: {}",
        err
    );
}

#[test]
fn interactive_rejects_stdin() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = batty()
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.as_mut().unwrap().write_all(b"hello\n");
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.contains("file path"),
        "expected file-path error, got: {}",
        err
    );
}

#[test]
fn diff_markers_appear_in_gutter() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let run_git = |args: &[&str]| {
        Command::new("git").args(args).current_dir(p).output().unwrap();
    };
    run_git(&["init", "-q"]);
    run_git(&["config", "user.email", "t@e.x"]);
    run_git(&["config", "user.name", "t"]);
    let f = p.join("a.txt");
    std::fs::write(&f, "alpha\nbeta\n").unwrap();
    run_git(&["add", "a.txt"]);
    run_git(&["commit", "-q", "-m", "init"]);
    // Modify line 1, add line 3
    std::fs::write(&f, "ALPHA\nbeta\ngamma\n").unwrap();

    let out = batty()
        .args(["--style=numbers,changes", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // Modified line 1 → "~", added line 3 → "+"
    assert!(s.contains('~'), "expected ~ marker for modified line; got: {}", s);
    assert!(s.contains('+'), "expected + marker for added line; got: {}", s);
}
