use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum InputKind {
    File(PathBuf),
    Stdin,
}

impl InputKind {
    pub fn from_path(path: &Path) -> Self {
        if path.as_os_str() == "-" {
            InputKind::Stdin
        } else {
            InputKind::File(path.to_path_buf())
        }
    }

    pub fn read(&self) -> Result<String> {
        match self {
            InputKind::File(p) => fs::read_to_string(p)
                .with_context(|| format!("failed to read {}", p.display())),
            InputKind::Stdin => {
                let mut s = String::new();
                io::stdin().read_to_string(&mut s).context("failed to read stdin")?;
                Ok(s)
            }
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            InputKind::File(p) => p.display().to_string(),
            InputKind::Stdin => "STDIN".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LineRange {
    pub start: usize, // 1-indexed inclusive
    pub end: usize,   // 1-indexed inclusive
}

impl LineRange {
    pub fn parse(s: &str) -> Result<Self> {
        let (start, end) = match s.split_once(':') {
            None => {
                let n: usize = s.parse().context("invalid line range")?;
                (n, n)
            }
            Some((a, b)) => {
                let start = if a.is_empty() { 1 } else { a.parse().context("invalid start in line range")? };
                let end = if b.is_empty() { usize::MAX } else { b.parse().context("invalid end in line range")? };
                (start, end)
            }
        };
        if start == 0 || end == 0 {
            anyhow::bail!("line numbers are 1-indexed; got {}", s);
        }
        if start > end {
            anyhow::bail!("invalid line range: start {} > end {}", start, end);
        }
        Ok(LineRange { start, end })
    }

    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && line <= self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single() {
        let r = LineRange::parse("42").unwrap();
        assert_eq!((r.start, r.end), (42, 42));
    }

    #[test]
    fn parse_range() {
        let r = LineRange::parse("10:20").unwrap();
        assert_eq!((r.start, r.end), (10, 20));
    }

    #[test]
    fn parse_open_start() {
        let r = LineRange::parse(":15").unwrap();
        assert_eq!((r.start, r.end), (1, 15));
    }

    #[test]
    fn parse_open_end() {
        let r = LineRange::parse("30:").unwrap();
        assert_eq!((r.start, r.end), (30, usize::MAX));
    }

    #[test]
    fn contains_works() {
        let r = LineRange::parse("10:20").unwrap();
        assert!(r.contains(10));
        assert!(r.contains(15));
        assert!(r.contains(20));
        assert!(!r.contains(9));
        assert!(!r.contains(21));
    }

    #[test]
    fn rejects_zero() {
        assert!(LineRange::parse("0").is_err());
        assert!(LineRange::parse("0:5").is_err());
        assert!(LineRange::parse("5:0").is_err());
    }

    #[test]
    fn rejects_inverted_range() {
        assert!(LineRange::parse("20:10").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(LineRange::parse("abc").is_err());
        assert!(LineRange::parse("").is_err());
        assert!(LineRange::parse("10:abc").is_err());
    }

    #[test]
    fn open_open_matches_all() {
        let r = LineRange::parse(":").unwrap();
        assert_eq!((r.start, r.end), (1, usize::MAX));
    }
}
