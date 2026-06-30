use anyhow::Result;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";

/// Pretty-print a single JSON value (one JSONL line) with 2-space indentation.
/// The `preserve_order` feature keeps object keys in their original order.
/// Returns `Err` if `line` is not a single valid JSON value.
pub fn prettify_line(line: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    Ok(serde_json::to_string_pretty(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prettify_expands_and_preserves_key_order() {
        let out = prettify_line(r#"{"b":1,"a":2}"#).unwrap();
        assert!(out.contains('\n'), "expected multi-line output: {out:?}");
        let b = out.find("\"b\"").unwrap();
        let a = out.find("\"a\"").unwrap();
        assert!(b < a, "key order not preserved: {out:?}");
        assert!(out.contains("  \"b\": 1"), "expected 2-space indent: {out:?}");
    }

    #[test]
    fn prettify_rejects_non_json() {
        assert!(prettify_line("not json at all").is_err());
        assert!(prettify_line("").is_err());
    }
}
