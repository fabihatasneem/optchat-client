//! Small helpers mirroring JS string semantics (UTF-16 lengths and slices) and hashing.
use sha2::{Digest, Sha256};

/// Split into chunks of at most `n` UTF-16 units (JS `text.slice(k*n, (k+1)*n)`), never splitting a surrogate pair.
/// An empty text is one empty chunk.
pub fn chunk_utf16(text: &str, n: usize) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut units = 0;
    for c in text.chars() {
        let u = c.len_utf16();
        if units + u > n && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            units = 0;
        }
        cur.push(c);
        units += u;
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

/// First 12 bytes of sha256, hex (the engine's `sha`).
pub fn sha12(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().take(12).map(|b| format!("{b:02x}")).collect()
}
/// First 8 bytes of sha256, hex (local.ts shortHash).
pub fn sha8(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Cut a string to at most `n` UTF-8 bytes on a char boundary (TS cutBytes).
pub fn cut_bytes(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut k = n;
    while !s.is_char_boundary(k) {
        k -= 1;
    }
    s[..k].to_string()
}

/// Newlines (with surrounding blanks) as single spaces (TS `flat`).
pub fn flat(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut pending_ws = String::new();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            pending_ws.push(c);
            continue;
        }
        if !pending_ws.is_empty() {
            if pending_ws.contains('\n') {
                out.push(' ');
            } else {
                out.push_str(&pending_ws);
            }
            pending_ws.clear();
        }
        out.push(c);
    }
    if !pending_ws.is_empty() {
        if pending_ws.contains('\n') {
            out.push(' ');
        } else {
            out.push_str(&pending_ws);
        }
    }
    out
}

/// JS `n.toFixed(d)`: the exact decimal value rounded half up at `d` digits (Rust's `{:.d}` rounds an exact tie to
/// even: 0.125 → "0.12" where JS gives "0.13").
pub fn to_fixed(n: f64, d: usize) -> String {
    if !n.is_finite() {
        return if n.is_nan() { "NaN".into() } else if n > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if n.abs() >= 1e21 {
        return format!("{n}");
    }
    // every f64 has an exact decimal expansion of at most 1074 fractional digits
    let exact = format!("{:.1100}", n.abs());
    let (int, frac) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().take(d)).map(|b| b - b'0').collect();
    let up = frac.as_bytes().get(d).map(|b| *b >= b'5').unwrap_or(false);
    if up {
        let mut k = digits.len();
        loop {
            if k == 0 {
                digits.insert(0, 1);
                break;
            }
            k -= 1;
            if digits[k] == 9 {
                digits[k] = 0;
            } else {
                digits[k] += 1;
                break;
            }
        }
    }
    let s: String = digits.iter().map(|x| (b'0' + x) as char).collect();
    let (i, f) = s.split_at(s.len() - d);
    let body = if d == 0 { i.to_string() } else { format!("{i}.{f}") };
    // JS keeps the sign of any x < 0 (-0.0001.toFixed(2) is "-0.00"; -0 is not < 0)
    if n < 0.0 { format!("-{body}") } else { body }
}

/// JS `usd` formatting of the engine: $n with 0/2/4/6 decimals by size.
pub fn usd(n: f64) -> String {
    let d = if n >= 10.0 {
        0
    } else if n >= 0.1 {
        2
    } else if n >= 0.001 {
        4
    } else {
        6
    };
    format!("${}", to_fixed(n, d))
}

/// task-tools.ts `money`: `$${n.toFixed(n < 1 ? 3 : 2)}`
pub fn money(n: f64) -> String {
    format!("${}", to_fixed(n, if n < 1.0 { 3 } else { 2 }))
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helpers() {
        assert_eq!(chunk_utf16("abcdef", 4), vec!["abcd", "ef"]);
        assert_eq!(chunk_utf16("", 4), vec![""]);
        assert_eq!(flat("a\n  b c\n"), "a b c ");
        assert_eq!(cut_bytes("héllo", 2), "h");
    }

    /// JS toFixed (outputs from `deno eval`): exact ties round up, inexact values by their exact decimal value
    #[test]
    fn to_fixed_matches_js() {
        for (n, d, want) in [
            (0.125, 2, "0.13"), (2.5, 0, "3"), (0.0625, 3, "0.063"), (0.25, 1, "0.3"), (1.005, 2, "1.00"), (0.0, 6, "0.000000"),
            (9.995, 2, "9.99"), (0.5, 0, "1"), (1.5, 0, "2"), (99.5, 0, "100"), (0.1234565, 6, "0.123456"), (123.456, 2, "123.46"),
            (-0.125, 2, "-0.13"), (-0.0001, 2, "-0.00"), (0.9995, 3, "1.000"), (0.0005, 3, "0.001"),
        ] {
            assert_eq!(to_fixed(n, d), want, "{n}.toFixed({d})");
        }
        assert_eq!(usd(0.125), "$0.13");
        assert_eq!(usd(0.0), "$0.000000");
        assert_eq!(usd(12.5), "$13");
        assert_eq!(money(0.0625), "$0.063");
        assert_eq!(money(2.125), "$2.13");
    }
}
