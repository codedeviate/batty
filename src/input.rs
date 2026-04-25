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
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        match parts.as_slice() {
            [a] => {
                let n: usize = a.parse().context("invalid line range")?;
                Ok(LineRange { start: n, end: n })
            }
            [a, b] => {
                let start = if a.is_empty() { 1 } else { a.parse().context("invalid start")? };
                let end = if b.is_empty() { usize::MAX } else { b.parse().context("invalid end")? };
                Ok(LineRange { start, end })
            }
            _ => unreachable!(),
        }
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
}
