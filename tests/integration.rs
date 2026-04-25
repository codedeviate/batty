use std::process::Command;

fn batty() -> Command {
    Command::new(env!("CARGO_BIN_EXE_batty"))
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
