use crate::cli::Encoding;
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

    pub fn read(&self, encoding: Encoding) -> Result<String> {
        let bytes = match self {
            InputKind::File(p) => fs::read(p)
                .with_context(|| format!("failed to read {}", p.display()))?,
            InputKind::Stdin => {
                let mut buf = Vec::new();
                io::stdin().read_to_end(&mut buf).context("failed to read stdin")?;
                buf
            }
        };
        decode(&bytes, encoding).with_context(|| match self {
            InputKind::File(p) => format!("failed to decode {}", p.display()),
            InputKind::Stdin => "failed to decode stdin".to_string(),
        })
    }

    pub fn display_name(&self) -> String {
        match self {
            InputKind::File(p) => p.display().to_string(),
            InputKind::Stdin => "STDIN".to_string(),
        }
    }
}

/// Decode raw bytes per the chosen encoding.
///
/// `Auto` returns the UTF-8 string when valid, otherwise re-decodes as
/// ISO-8859-1 (which is infallible: every byte maps to U+0000..=U+00FF).
/// `Utf8` is strict — invalid sequences error. `Latin1` always succeeds.
pub fn decode(bytes: &[u8], encoding: Encoding) -> Result<String> {
    match encoding {
        Encoding::Utf8 => std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .context("invalid UTF-8 (try --encoding=auto or --encoding=iso-8859-1)"),
        Encoding::Latin1 => Ok(decode_latin1(bytes)),
        Encoding::Auto => match std::str::from_utf8(bytes) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => Ok(decode_latin1(bytes)),
        },
    }
}

fn decode_latin1(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        s.push(b as char);
    }
    s
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

    #[test]
    fn decode_utf8_strict_rejects_latin1() {
        // 0xE5 is `å` in Latin-1, but a continuation byte in UTF-8.
        let bytes = b"caf\xe9\n";
        assert!(decode(bytes, Encoding::Utf8).is_err());
    }

    #[test]
    fn decode_latin1_always_succeeds() {
        let bytes = b"caf\xe9 \xe5\xe4\xf6\n";
        let s = decode(bytes, Encoding::Latin1).unwrap();
        assert_eq!(s, "café åäö\n");
    }

    #[test]
    fn decode_auto_prefers_utf8_when_valid() {
        let bytes = "café".as_bytes();
        assert_eq!(decode(bytes, Encoding::Auto).unwrap(), "café");
    }

    #[test]
    fn decode_auto_falls_back_to_latin1() {
        let bytes = b"caf\xe9";
        assert_eq!(decode(bytes, Encoding::Auto).unwrap(), "café");
    }

    #[test]
    fn decode_latin1_covers_full_byte_range() {
        let bytes: Vec<u8> = (0u8..=255).collect();
        let s = decode(&bytes, Encoding::Latin1).unwrap();
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 256);
        for (i, &c) in chars.iter().enumerate() {
            assert_eq!(c as u32, i as u32);
        }
    }

    #[test]
    fn decode_empty_is_empty() {
        assert_eq!(decode(b"", Encoding::Auto).unwrap(), "");
        assert_eq!(decode(b"", Encoding::Utf8).unwrap(), "");
        assert_eq!(decode(b"", Encoding::Latin1).unwrap(), "");
    }
}
