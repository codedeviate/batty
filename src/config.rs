use std::fs;
use std::path::PathBuf;

/// Resolve the config file path. Always `~/.config/batty/config.toml`
/// (XDG-style) on every platform — including macOS, where
/// `dirs::config_dir()` would return `~/Library/Application Support`.
pub fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".config").join("batty").join("config.toml"))
}

/// Load config args from a specific TOML file. Returns empty Vec if absent.
/// Each top-level key is converted to a CLI argv token:
///
/// - `theme = "Dracula"`         → `--theme=Dracula`
/// - `top-pad = 2`               → `--top-pad=2`
/// - `interactive = true`        → `--interactive`
/// - `interactive = false`       → (omitted)
/// - `highlight-line = [10, 20]` → `--highlight-line=10 --highlight-line=20`
///
/// Nested tables and unsupported value types are ignored with a warning.
pub fn load_args_from(path: &std::path::Path) -> Vec<String> {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return vec![],
        Err(e) => {
            eprintln!("batty: warning: ignoring config {}: {}", path.display(), e);
            return vec![];
        }
    };
    let table: toml::Table = match contents.parse() {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "batty: warning: ignoring config {} (TOML parse error): {}",
                path.display(),
                e
            );
            return vec![];
        }
    };
    table_to_args(&table)
}

fn table_to_args(table: &toml::Table) -> Vec<String> {
    let mut out = Vec::with_capacity(table.len() * 2);
    for (key, value) in table {
        match value {
            toml::Value::Boolean(true) => out.push(format!("--{}", key)),
            toml::Value::Boolean(false) => {}
            toml::Value::Array(arr) => {
                for item in arr {
                    if let Some(s) = scalar_to_string(item) {
                        out.push(format!("--{}={}", key, s));
                    } else {
                        eprintln!(
                            "batty: warning: config key '{}' has unsupported array element; skipping",
                            key
                        );
                    }
                }
            }
            v => match scalar_to_string(v) {
                Some(s) => out.push(format!("--{}={}", key, s)),
                None => eprintln!(
                    "batty: warning: config key '{}' has unsupported value type; skipping",
                    key
                ),
            },
        }
    }
    out
}

fn scalar_to_string(v: &toml::Value) -> Option<String> {
    match v {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Float(f) => Some(f.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        toml::Value::Datetime(d) => Some(d.to_string()),
        _ => None,
    }
}

/// Load config args from the default location.
pub fn load_args() -> Vec<String> {
    config_path().map(|p| load_args_from(&p)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_toml(contents: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_string_and_int() {
        let f = write_toml(r#"
            theme = "Dracula"
            tabs = 2
        "#);
        let mut args = load_args_from(f.path());
        args.sort();
        assert_eq!(args, vec!["--tabs=2", "--theme=Dracula"]);
    }

    #[test]
    fn boolean_true_emits_flag_only() {
        let f = write_toml("interactive = true\n");
        let args = load_args_from(f.path());
        assert_eq!(args, vec!["--interactive"]);
    }

    #[test]
    fn boolean_false_is_omitted() {
        let f = write_toml("interactive = false\n");
        let args = load_args_from(f.path());
        assert!(args.is_empty());
    }

    #[test]
    fn array_expands_to_repeated_flags() {
        let f = write_toml("highlight-line = [10, 20, 30]\n");
        let args = load_args_from(f.path());
        assert_eq!(
            args,
            vec!["--highlight-line=10", "--highlight-line=20", "--highlight-line=30"]
        );
    }

    #[test]
    fn missing_file_returns_empty() {
        let args = load_args_from(std::path::Path::new("/nonexistent/path"));
        assert!(args.is_empty());
    }

    #[test]
    fn malformed_toml_returns_empty_with_warning() {
        let f = write_toml("not = valid = toml\n");
        let args = load_args_from(f.path());
        assert!(args.is_empty());
    }

    #[test]
    fn comments_and_whitespace_are_fine() {
        let f = write_toml(r#"
            # this is a comment
            theme = "Nord"   # inline comment
        "#);
        let args = load_args_from(f.path());
        assert_eq!(args, vec!["--theme=Nord"]);
    }

    #[test]
    fn preserves_value_with_spaces() {
        let f = write_toml(r#"theme = "Solarized Dark""#);
        let args = load_args_from(f.path());
        assert_eq!(args, vec!["--theme=Solarized Dark"]);
    }
}
