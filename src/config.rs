use std::fs;
use std::path::PathBuf;

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("batty").join("config"))
}

/// Load config args from a specific file path. Returns empty Vec if file is absent.
pub fn load_args_from(path: &std::path::Path) -> Vec<String> {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return vec![],
        Err(e) => {
            eprintln!("batty: warning: ignoring config {}: {}", path.display(), e);
            return vec![];
        }
    };
    contents
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// Load config args from the default location.
pub fn load_args() -> Vec<String> {
    config_path().map(|p| load_args_from(&p)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn one_token_per_line() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "--theme=Dracula").unwrap();
        writeln!(f, "# a comment").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "--tabs").unwrap();
        writeln!(f, "2").unwrap();
        let args = load_args_from(f.path());
        assert_eq!(args, vec!["--theme=Dracula", "--tabs", "2"]);
    }

    #[test]
    fn missing_file_returns_empty() {
        let args = load_args_from(std::path::Path::new("/nonexistent/path"));
        assert!(args.is_empty());
    }

    #[test]
    fn comment_only_file_returns_empty() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "# only comments").unwrap();
        writeln!(f, "# more comments").unwrap();
        let args = load_args_from(f.path());
        assert!(args.is_empty());
    }

    #[test]
    fn blank_file_returns_empty() {
        let f = tempfile::NamedTempFile::new().unwrap();
        let args = load_args_from(f.path());
        assert!(args.is_empty());
    }

    #[test]
    fn preserves_value_with_spaces() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "--theme=Solarized Dark").unwrap();
        let args = load_args_from(f.path());
        assert_eq!(args, vec!["--theme=Solarized Dark"]);
    }
}
