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
fn markdown_on_extension_renders_md_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("doc.md");
    std::fs::write(&f, "# Title\n\n**bold** text\n").unwrap();
    let out = batty()
        .args(["--markdown-on-extension", "--style=plain", "--color=always"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // Rendered markdown: heading text survives, ANSI styling present, raw `# `
    // prefix is gone.
    assert!(s.contains("Title"), "output: {:?}", s);
    assert!(s.contains("\x1b["), "expected ANSI escapes in: {:?}", s);
    assert!(!s.contains("# Title"), "raw heading should be gone: {:?}", s);
}

#[test]
fn markdown_on_extension_leaves_non_md_file_raw() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("foo.rs");
    std::fs::write(&f, "// # Heading-like comment\nfn main() {}\n").unwrap();
    let out = batty()
        .args(["--markdown-on-extension", "--plain", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // The literal source survives unchanged when extension doesn't match.
    assert_eq!(s, "// # Heading-like comment\nfn main() {}\n");
}

#[test]
fn markdown_force_overrides_markdown_on_extension() {
    // --markdown beats extension check: a .rs file still renders as markdown.
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("foo.rs");
    std::fs::write(&f, "# Heading\n\nfn main() {}\n").unwrap();
    let out = batty()
        .args([
            "--markdown",
            "--markdown-on-extension",
            "--style=plain",
            "--color=always",
        ])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // --markdown wins → file rendered as markdown despite .rs extension.
    assert!(s.contains("Heading"));
    assert!(s.contains("\x1b["));
    assert!(!s.contains("# Heading"));
}

#[test]
fn markdown_on_extension_via_config() {
    // Config-only path: markdown-on-extension = true in the TOML, .md file
    // on CLI → renders. Same config + .rs file → raw.
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    std::fs::write(&cfg, "markdown-on-extension = true\n").unwrap();

    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Title\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_batty"))
        .env("BATTY_CONFIG_PATH", &cfg)
        .args(["--style=plain", "--color=always"])
        .arg(&md)
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(!s.contains("# Title"), "md should render: {:?}", s);

    let rs = dir.path().join("foo.rs");
    std::fs::write(&rs, "fn main() {}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_batty"))
        .env("BATTY_CONFIG_PATH", &cfg)
        .args(["--plain", "--color=never"])
        .arg(&rs)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "fn main() {}\n");
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
fn follow_initial_render_shows_last_n_lines() {
    // Spawn batty with -f and --tail-lines=3 against a static file. Poll for
    // the initial render to land in the output file (don't rely on a fixed
    // sleep), then kill the child.
    use std::fs::File;
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("log.txt");
    let body: String = (1..=10).map(|n| format!("line {}\n", n)).collect();
    std::fs::write(&f, body).unwrap();
    let out_path = dir.path().join("stdout.txt");
    let err_path = dir.path().join("stderr.txt");
    let stdout_file = File::create(&out_path).unwrap();
    let stderr_file = File::create(&err_path).unwrap();

    let mut child = batty()
        .args(["-f", "--tail-lines=3", "--style=plain,numbers", "--color=never"])
        .arg(&f)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .spawn()
        .unwrap();

    // Poll the output file for up to 10 seconds, looking for the last line
    // we expect ("line 10"). Stops as soon as it appears.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut out = String::new();
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(s) = std::fs::read_to_string(&out_path) {
            if s.contains("line 10") {
                out = s;
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();

    if out.is_empty() {
        let err = std::fs::read_to_string(&err_path).unwrap_or_default();
        let final_out = std::fs::read_to_string(&out_path).unwrap_or_default();
        panic!("never saw 'line 10' within 10s. stderr: {:?}, stdout: {:?}", err, final_out);
    }
    // Last 3 lines (8, 9, 10) should appear; earlier lines should not.
    assert!(out.contains("line 8"), "expected line 8 in: {:?}", out);
    assert!(out.contains("line 9"), "expected line 9 in: {:?}", out);
    assert!(out.contains("line 10"), "expected line 10 in: {:?}", out);
    assert!(!out.contains("line 1\n"), "should not have line 1: {:?}", out);
    // Line numbers should be the actual file positions, not 1..3.
    assert!(out.contains("8 "));
    assert!(out.contains("10 "));
}

#[test]
fn follow_rejects_multiple_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    std::fs::write(&a, "x").unwrap();
    std::fs::write(&b, "y").unwrap();
    let out = batty().arg("-f").arg(&a).arg(&b).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("single file"), "got: {}", err);
}

#[test]
fn follow_rejects_stdin() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = batty()
        .arg("-f")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.as_mut().unwrap().write_all(b"x\n");
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("file path"), "got: {}", err);
}

#[test]
fn follow_and_interactive_are_mutually_exclusive() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "x").unwrap();
    let out = batty().args(["-i", "-f"]).arg(&f).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("mutually exclusive"), "got: {}", err);
}

#[test]
fn markdown_with_numbers_shows_source_line_in_gutter() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("doc.md");
    // Three blocks at known source lines: title (1), paragraph (3), list (5).
    std::fs::write(&f, "# Title\n\nA paragraph.\n\n- item one\n").unwrap();
    let out = batty()
        .args(["--markdown", "--style=numbers,grid", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // Block source lines (1, 3, 5) should appear before grid bars in the
    // gutter. Use a loose contains() since termimad varies whitespace.
    assert!(s.contains("│"), "expected grid bar: {}", s);
    assert!(s.contains("1 │") || s.contains("   1 │"), "missing source-line 1: {}", s);
    assert!(s.contains("3 │") || s.contains("   3 │"), "missing source-line 3: {}", s);
    assert!(s.contains("5 │") || s.contains("   5 │"), "missing source-line 5: {}", s);
}

#[test]
fn markdown_with_no_gutter_suppresses_gutter() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("doc.md");
    std::fs::write(&f, "# Title\n\nParagraph.\n").unwrap();
    let out = batty()
        .args(["--markdown", "--no-gutter", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // No grid bar with --no-gutter active.
    assert!(!s.contains("│"), "expected no grid bar: {}", s);
}

#[test]
fn no_gutter_strips_numbers_and_grid_keeps_header() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "alpha\nbeta\n").unwrap();
    let out = batty()
        .args(["--style=full", "--no-gutter", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // Header still present.
    assert!(s.contains("File:"), "expected File: header in: {:?}", s);
    // Body lines are bare — no `│` grid bar, no leading line-number column.
    assert!(s.contains("alpha\n"));
    assert!(s.contains("beta\n"));
    assert!(!s.contains("│"), "no grid bar expected: {}", s);
    // No leading line-number digits before "alpha" / "beta" (they appear
    // as the first non-decoration line content).
    let lines: Vec<&str> = s.lines().collect();
    assert!(
        lines.iter().any(|l| l.trim_start() == "alpha"),
        "expected bare 'alpha' line in: {:?}",
        lines
    );
}

#[test]
fn gutter_cancels_no_gutter_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    std::fs::write(&cfg, "no-gutter = true\n").unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "alpha\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_batty"))
        .env("BATTY_CONFIG_PATH", &cfg)
        .args(["--gutter", "--style=full", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // --gutter cancelled the config's no-gutter; the grid bar reappears.
    assert!(s.contains("│"), "expected grid bar back: {}", s);
}

#[test]
fn wrap_character_breaks_long_lines() {
    // batty doesn't get a real terminal width during tests; --color=never
    // and the default fallback width (80) make this deterministic. We
    // craft a 200-char line that should produce multiple visual rows.
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("long.txt");
    let line: String = "x".repeat(200);
    std::fs::write(&f, format!("{}\nshort\n", line)).unwrap();
    let out = batty()
        .args([
            "--wrap=character",
            "--style=numbers,grid",
            "--color=never",
        ])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // The 200-char line should be split into multiple body lines. With
    // --color=never the gutter is `   1 │ ` (line-no=4 + space + grid),
    // so body width ≈ 80 - 7 = 73 cols, giving us roughly ceil(200/73) ≈ 3
    // visual rows for source line 1.
    let xs_count = s.matches('x').count();
    assert_eq!(xs_count, 200, "all 200 'x' chars should survive: got {}", xs_count);
    // Continuation rows should have at least one `│` repeated past the first.
    let bar_count = s.matches('│').count();
    assert!(bar_count >= 3, "expected >=3 grid bars (line1 wraps + line2); got {}", bar_count);
    // The actual content "short" survived.
    assert!(s.contains("short"));
}

#[test]
fn wrap_never_emits_full_line() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("long.txt");
    let line: String = "y".repeat(200);
    std::fs::write(&f, format!("{}\n", line)).unwrap();
    let out = batty()
        .args([
            "--wrap=never",
            "--style=numbers",
            "--color=never",
        ])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    // With wrap=never, batty never inserts breaks within the source line.
    // The output has exactly one '\n' at the end of line 1 (and one for the
    // trailing newline of the file).
    let newlines = s.matches('\n').count();
    assert_eq!(newlines, 1, "wrap=never should emit one line; got {} newlines", newlines);
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
fn rhai_grammar_full_coverage() {
    // Exercises the full v0.4.x Rhai grammar: range/bitwise/nested comments
    // (0.4.0) plus template strings, map literals, ?? / ?., :: accessor,
    // leading-dot floats, and new builtins (0.4.1). Failure mode would be
    // syntect's runtime parser error or a content-survival assertion miss.
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("t.rhai");
    std::fs::write(
        &f,
        "let r = 1..=10;\n\
         let m = a & b | c ^ d;\n\
         let s = x << 2 | y >> 1;\n\
         /* outer /* inner */ outer */\n\
         let name = `Hello, ${user.name ?? \"world\"}!`;\n\
         let map = #{ key: 1, nested: #{ inner: 2 } };\n\
         let safe = obj?.field ?? default;\n\
         let pi = .14159;\n\
         let q = math::sqrt(2.0);\n\
         let f = Fn(\"name\");\n\
         let n = parse_int(\"42\");\n\
         let c = 'x';\n\
         fn double(x) { x * 2 }\n",
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
    // 0.4.1 additions:
    assert!(stripped.contains("Hello, ${"), "template string survived: {:?}", stripped);
    assert!(stripped.contains("#{ key: 1"), "map literal survived: {:?}", stripped);
    assert!(stripped.contains("obj?.field ?? default"), "?? / ?. ops: {:?}", stripped);
    assert!(stripped.contains("let c = 'x'"), "char literal survived: {:?}", stripped);
    assert!(stripped.contains(".14159"), "leading-dot float: {:?}", stripped);
    assert!(stripped.contains("math::sqrt"), ":: accessor: {:?}", stripped);
    assert!(stripped.contains("Fn("), "Fn builtin: {:?}", stripped);
    assert!(stripped.contains("parse_int"), "parse_int builtin: {:?}", stripped);
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

#[test]
fn auto_encoding_handles_iso_8859_1_file() {
    // "café åäö\n" encoded as ISO-8859-1 (not valid UTF-8).
    let bytes = b"caf\xe9 \xe5\xe4\xf6\n";
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("latin1.txt");
    std::fs::write(&f, bytes).unwrap();

    let out = batty()
        .args(["--plain", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    assert_eq!(s, "café åäö\n");
}

#[test]
fn explicit_iso_8859_1_encoding_decodes_latin1() {
    let bytes = b"Espa\xf1ol\n";
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("latin1.txt");
    std::fs::write(&f, bytes).unwrap();

    let out = batty()
        .args(["--plain", "--color=never", "--encoding=iso-8859-1"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    assert_eq!(s, "Español\n");
}

#[test]
fn strict_utf8_encoding_errors_on_latin1_input() {
    let bytes = b"caf\xe9\n";
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("latin1.txt");
    std::fs::write(&f, bytes).unwrap();

    let out = batty()
        .args(["--plain", "--color=never", "--encoding=utf-8"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(!out.status.success(), "expected failure; stdout: {}", String::from_utf8_lossy(&out.stdout));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("UTF-8") || stderr.contains("decode"),
        "expected decode error in stderr; got: {}", stderr);
}

#[test]
fn auto_encoding_passes_utf8_through_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("utf8.txt");
    std::fs::write(&f, "café 日本語\n").unwrap();

    let out = batty()
        .args(["--plain", "--color=never"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    assert_eq!(s, "café 日本語\n");
}

/// `--wrap=auto` should treat non-TTY stdout as `never` (matches bat).
/// Under captured stdout, `term_width()` falls back to 80, so a 200-char
/// line is longer than the printable width. With the resolution in place,
/// the long line emits unbroken — no continuation prefix, no extra `\n`.
#[test]
fn wrap_auto_is_never_when_stdout_not_tty() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("long.txt");
    // 200 'a's — comfortably > 80 cols (the non-TTY term_width fallback).
    let content = "a".repeat(200) + "\n";
    std::fs::write(&f, &content).unwrap();
    let out = batty()
        .args(["--plain", "--color=never", "--wrap=auto"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    // With wrap=auto resolved to never, the 200-char line stays on one
    // output line: exactly one '\n' (the source-line terminator).
    let newline_count = s.matches('\n').count();
    assert_eq!(
        newline_count, 1,
        "expected single newline (no wrap) under piped stdout, got {} in {:?}",
        newline_count, s
    );
}

/// `--wrap=character` must still wrap even when stdout is not a TTY.
/// Explicit user intent overrides the auto resolution.
#[test]
fn wrap_character_still_wraps_when_piped() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("long.txt");
    let content = "a".repeat(200) + "\n";
    std::fs::write(&f, &content).unwrap();
    let out = batty()
        .args(["--plain", "--color=never", "--wrap=character"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    let newline_count = s.matches('\n').count();
    assert!(
        newline_count > 1,
        "expected multiple newlines (wrap=character still wraps), got {} in {:?}",
        newline_count, s
    );
}
