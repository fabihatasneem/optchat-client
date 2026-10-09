//! Lenient JSON: strict parse first, then retry with comments and trailing
//! commas stripped.

use serde_json::Value;

pub(crate) fn lenient_json(src: &str) -> anyhow::Result<Value> {
    let src = src.trim_start_matches('\u{feff}');
    match serde_json::from_str(src) {
        Ok(v) => Ok(v),
        Err(first) => serde_json::from_str(&clean(src))
            .map_err(|_| anyhow::anyhow!("invalid theme JSON: {first}")),
    }
}

/// Removes `//` / `/* */` comments and trailing commas outside strings.
fn clean(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => {
                in_str = true;
                out.push(c);
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i += 2;
            }
            b']' | b'}' => {
                // Drop a trailing comma (and the whitespace after it).
                let mut j = out.len();
                while j > 0 && out[j - 1].is_ascii_whitespace() {
                    j -= 1;
                }
                if j > 0 && out[j - 1] == b',' {
                    out.remove(j - 1);
                }
                out.push(c);
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_default()
}
