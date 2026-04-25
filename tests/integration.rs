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
