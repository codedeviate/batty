use std::fs;
use std::path::PathBuf;

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("batty").join("config"))
}

/// Load config args from a specific file path. Returns empty Vec if file is absent.
pub fn load_args_from(path: &std::path::Path) -> Vec<String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return vec![];
    };
    contents
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .flat_map(|l| l.split_whitespace().map(String::from).collect::<Vec<_>>())
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
    fn parses_one_arg_per_line() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "--theme Dracula").unwrap();
        writeln!(f, "# a comment").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "--tabs 2").unwrap();
        let args = load_args_from(f.path());
        assert_eq!(args, vec!["--theme", "Dracula", "--tabs", "2"]);
    }

    #[test]
    fn missing_file_returns_empty() {
        let args = load_args_from(std::path::Path::new("/nonexistent/path"));
        assert!(args.is_empty());
    }
}
