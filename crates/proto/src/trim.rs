//! Structure-preserving trim (lib/trim.ts, spec-tasks part C): shrink a JSON value to about `budget` serialized
//! characters while it stays valid JSON with every object key. Big strings are cut in the middle (head ~75%, tail
//! ~25%, joined by CUT); arrays lose whole trailing items (no marker: the array keeps its shape); small values are
//! never touched. Used for the websocket copy of log entries (hub slimEntry); the stored entry is never trimmed.
//!
//! Sizes are `JSON.stringify(v).length`: UTF-16 code units of the JS serialization (`js_stringify` below formats
//! numbers the way JS does, e.g. `1.0` -> `1`).
use serde_json::{Map, Value};

pub const CUT: &str = "…[cut]…";
const CUT_LEN: i64 = 7; // CUT.length in UTF-16 units
const MAX_DEPTH: usize = 8; // deeper subtrees are kept as they are
const MIN_SHARE: i64 = 40; // the least a tool input gets
const WHOLE: i64 = 200; // array items up to this size are kept whole or dropped, never cut
const INF: i64 = i64::MAX;

fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// A JS `Number#toString` of `x` (also what `JSON.stringify` prints for a finite number).
pub(crate) fn js_num(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let sign = if x < 0.0 { "-" } else { "" };
    let e = format!("{:e}", x.abs()); // shortest round-trip digits, e.g. "1.2345e2"
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let exp: i64 = exp.parse().unwrap_or(0);
    let k = digits.len() as i64;
    let n = exp + 1;
    let body = if k <= n && n <= 21 {
        format!("{}{}", digits, "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{}", "0".repeat((-n) as usize), digits)
    } else {
        let rest = if k > 1 { format!(".{}", &digits[1..]) } else { String::new() };
        let es = n - 1;
        format!("{}{}e{}{}", &digits[..1], rest, if es >= 0 { "+" } else { "-" }, es.abs())
    };
    format!("{sign}{body}")
}

/// `JSON.stringify(v)` as JS prints it (numbers formatted like JS; strings escaped like JS, which serde_json matches:
/// only `"`, `\` and control characters, lowercase `\u00xx`).
pub(crate) fn js_stringify(v: &Value) -> String {
    let mut out = String::new();
    write_js(v, &mut out);
    out
}

fn write_js(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            if f.is_finite() {
                out.push_str(&js_num(f))
            } else {
                out.push_str("null")
            }
        }
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_default()),
        Value::Array(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_js(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap_or_default());
                out.push(':');
                write_js(x, out);
            }
            out.push('}');
        }
    }
}

fn size(v: &Value) -> i64 {
    js_len(&js_stringify(v)) as i64
}

fn str_size(s: &str) -> i64 {
    js_len(&serde_json::to_string(s).unwrap_or_default()) as i64
}

// a string cut to fit `budget` serialized characters (quotes and escapes included), never splitting a surrogate pair
fn cut_string(s: &str, budget: i64) -> String {
    let cps: Vec<char> = s.chars().collect();
    let len = cps.len() as i64;
    let mut keep = 0.max(len.min(budget - 2 - CUT_LEN));
    loop {
        let head = (keep * 3 + 3) / 4; // ceil(keep * 0.75)
        let tail = keep - head;
        let mut out: String = cps[..head as usize].iter().collect();
        out.push_str(CUT);
        if tail > 0 {
            out.extend(cps[(len - tail) as usize..].iter());
        }
        let l = str_size(&out);
        if l <= budget || keep == 0 {
            return out;
        }
        // escapes or wide characters made it longer: scale down to what fits, by at least one
        let room = budget - 2 - CUT_LEN;
        let used = l - 2 - CUT_LEN;
        let scaled = ((keep as f64) * (room as f64) / (used as f64)).floor() as i64;
        keep = 0.max((keep - 1).min(scaled));
    }
}

// the cap c with Σ min(size_i, c) <= room, maximal (water-filling); INF when everything fits
fn waterline(sizes: &[i64], room: i64) -> i64 {
    if sizes.iter().sum::<i64>() <= room {
        return INF;
    }
    let mut lo = 0i64;
    let mut hi = sizes.iter().copied().max().unwrap_or(i64::MIN);
    while lo < hi {
        let mid = (lo + hi + 1).div_euclid(2); // ceil((lo + hi) / 2), lo and hi >= 0
        if sizes.iter().map(|s| (*s).min(mid)).sum::<i64>() <= room {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

fn shrinkable(v: &Value) -> bool {
    matches!(v, Value::String(_) | Value::Array(_) | Value::Object(_))
}

/// Trim `value` to about `budget` serialized UTF-16 characters (see the module doc). A value that fits is returned
/// unchanged.
pub fn trim(value: &Value, budget: usize) -> Value {
    trim_at(value, budget.min(i64::MAX as usize) as i64, 0)
}

fn trim_at(value: &Value, budget: i64, depth: usize) -> Value {
    if size(value) <= budget || depth >= MAX_DEPTH {
        return value.clone(); // fits, or too deep
    }
    match value {
        Value::String(s) => Value::String(cut_string(s, budget)),
        Value::Array(all) => {
            // structure: brackets and commas; fixed items (numbers…) count fully
            let fill = |xs: &[Value]| -> i64 {
                let overhead = 2 + 0.max(xs.len() as i64 - 1);
                let fixed: i64 = xs.iter().filter(|x| !shrinkable(x)).map(size).sum();
                let sizes: Vec<i64> = xs.iter().filter(|x| shrinkable(x)).map(size).collect();
                waterline(&sizes, budget - overhead - fixed)
            };
            // too many items for each to keep something useful: drop trailing ones first, until the small ones stay whole
            let want = WHOLE.min(all.iter().filter(|x| shrinkable(x)).map(size).max().unwrap_or(0).max(0));
            let mut n = all.len();
            while n > 1 && fill(&all[..n]) < want {
                n -= 1;
            }
            let items = &all[..n];
            let c = fill(items);
            let mut out: Vec<Value> = items
                .iter()
                .map(|x| if shrinkable(x) && size(x) > c { trim_at(x, c, depth + 1) } else { x.clone() })
                .collect();
            while out.len() > 1 && size(&Value::Array(out.clone())) > budget {
                out.pop(); // still over: whole trailing items go
            }
            Value::Array(out)
        }
        Value::Object(m) => {
            // every key stays, in order; values share what is left after the keys and punctuation
            let zeros: Map<String, Value> = m.keys().map(|k| (k.clone(), Value::from(0))).collect();
            let overhead = size(&Value::Object(zeros)) - m.len() as i64; // {"a":0,"b":0} minus the zeros
            let fixed: i64 = m.values().filter(|v| !shrinkable(v)).map(size).sum();
            let sizes: Vec<i64> = m.values().filter(|v| shrinkable(v)).map(size).collect();
            let c = waterline(&sizes, budget - overhead - fixed);
            let out: Map<String, Value> = m
                .iter()
                .map(|(k, v)| {
                    let v2 = if shrinkable(v) && size(v) > c { trim_at(v, c, depth + 1) } else { v.clone() };
                    (k.clone(), v2)
                })
                .collect();
            Value::Object(out)
        }
        _ => value.clone(), // a number, boolean or null can't shrink
    }
}

/// A tool entry's text is `name {json input}`: trim the input as JSON so the card can still read it.
/// Returns `(text, cut)`.
pub fn trim_tool_text(text: &str, budget: usize) -> (String, bool) {
    if js_len(text) <= budget {
        return (text.to_string(), false);
    }
    if let Some(sp) = text.find(' ') {
        if sp > 0 {
            if let Ok(input) = serde_json::from_str::<Value>(&text[sp + 1..]) {
                let name = &text[..sp];
                let b = MIN_SHARE.max(budget as i64 - js_len(name) as i64 - 1);
                let t = trim_at(&input, b, 0);
                return (format!("{} {}", name, js_stringify(&t)), true);
            }
        }
    }
    (head_tail(text, budget), true)
}

// byte offset of the UTF-16 unit index `u` (clamped); a boundary inside a surrogate pair moves forward (`up`) or back
fn utf16_byte(s: &str, u: usize, up: bool) -> usize {
    let mut units = 0usize;
    for (b, c) in s.char_indices() {
        if units >= u {
            return b;
        }
        let w = c.len_utf16();
        if units + w > u {
            // u falls inside this char's surrogate pair
            return if up { b + c.len_utf8() } else { b };
        }
        units += w;
    }
    s.len()
}

/// JS `s.slice(0, n)` on UTF-16 units, never splitting a surrogate pair (a split pair is kept whole).
pub(crate) fn js_head(s: &str, n: usize) -> &str {
    &s[..utf16_byte(s, n, true)]
}

/// JS `s.slice(-n)` on UTF-16 units (`n == 0` gives the whole string, as `slice(-0)` does), never splitting a
/// surrogate pair (a split pair is kept whole).
pub(crate) fn js_tail(s: &str, n: usize) -> &str {
    let len = js_len(s);
    let start = len.saturating_sub(n);
    if n == 0 {
        return s;
    }
    &s[utf16_byte(s, start, false)..]
}

/// Plain text (echoes, talk, non-JSON tool text): head + tail, as the websocket copy has always been cut.
pub fn head_tail(text: &str, budget: usize) -> String {
    if js_len(text) <= budget {
        return text.to_string();
    }
    let head = budget * 3 / 4; // floor(budget * 0.75)
    let tail = budget / 5; // floor(budget * 0.2)
    format!("{}\n…\n{}", js_head(text, head), js_tail(text, tail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sz(v: &Value) -> usize {
        js_len(&js_stringify(v))
    }
    fn split_cut(s: &str) -> (String, String) {
        let (h, t) = s.split_once(CUT).expect("has the marker");
        (h.to_string(), t.to_string())
    }

    #[test]
    fn js_formatting() {
        assert_eq!(js_num(1.0), "1");
        assert_eq!(js_num(-0.0), "0");
        assert_eq!(js_num(1.5), "1.5");
        assert_eq!(js_num(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(js_num(1e21), "1e+21");
        assert_eq!(js_num(1.5e-7), "1.5e-7");
        assert_eq!(js_num(0.000001), "0.000001");
        assert_eq!(js_num(123456789012345680000.0), "123456789012345680000");
        assert_eq!(js_num(-2.5e30), "-2.5e+30");
        assert_eq!(js_stringify(&json!({"a": 1.0, "b": [true, null, "x\n\u{1}é\u{2028}😀"]})),
            "{\"a\":1,\"b\":[true,null,\"x\\n\\u0001é\u{2028}😀\"]}");
    }

    // 1. under budget: unchanged
    #[test]
    fn t1_under_budget() {
        let v = json!({"a": "x", "b": [1, 2, 3]});
        assert_eq!(trim(&v, 1000), v);
    }

    // 2. every key kept, in order
    #[test]
    fn t2_keys_in_order() {
        let v = json!({"mode": "handoff", "title": "a title", "instruction": "y".repeat(5000), "settings": {"model": "m", "effort": "low"}, "budget": null});
        let t = trim(&v, 600);
        let k1: Vec<_> = v.as_object().unwrap().keys().collect();
        let k2: Vec<_> = t.as_object().unwrap().keys().collect();
        assert_eq!(k1, k2);
    }

    // 3. one big string with small siblings: the siblings whole, the big one cut with the marker
    #[test]
    fn t3_siblings_whole() {
        let v = json!({"a": "small", "b": "z".repeat(3000), "c": "tiny"});
        let t = trim(&v, 500);
        assert_eq!(t["a"], "small");
        assert_eq!(t["c"], "tiny");
        assert!(t["b"].as_str().unwrap().contains(CUT));
        assert!(sz(&t) <= 500, "{}", sz(&t));
    }

    // 4. nested: within budget + key/marker overhead
    #[test]
    fn t4_nested() {
        let v = json!({"a": {"b": {"c": "q".repeat(4000), "d": ["r".repeat(1500), "s".repeat(1500)]}, "e": "t".repeat(2000)}, "f": 1});
        let t = trim(&v, 1200);
        assert!(sz(&t) <= 1200 + 60, "{}", sz(&t));
    }

    // 5. head ~75%, tail ~25%
    #[test]
    fn t5_head_tail_ratio() {
        let s = "a".repeat(600) + &"b".repeat(600);
        let t = trim(&json!(s), 402);
        let (h, tl) = split_cut(t.as_str().unwrap());
        let r = h.len() as f64 / (h.len() + tl.len()) as f64;
        assert!((r - 0.75).abs() < 0.01 && h.starts_with("aaa") && tl.ends_with("bbb"), "{} / {}", h.len(), tl.len());
    }

    // 6. arrays: valid json, leading items kept in order, trailing ones dropped, no marker item
    #[test]
    fn t6_arrays() {
        let v: Vec<Value> = (0..100).map(|k| json!(format!("item {k} {}", "w".repeat(80)))).collect();
        let t = trim(&Value::Array(v.clone()), 1000);
        let ta = t.as_array().unwrap();
        assert!(ta.len() < 100 && !ta.is_empty());
        assert!(ta.iter().enumerate().all(|(k, x)| *x == v[k]));
        assert!(!ta.iter().any(|x| x.as_str().unwrap().contains(CUT)));
        assert!(sz(&t) <= 1000);
        let few = trim(&json!(["x".repeat(3000), "y".repeat(3000)]), 1000);
        let fa = few.as_array().unwrap();
        assert!(fa.len() == 2 && fa.iter().all(|x| x.as_str().unwrap().contains(CUT)) && sz(&few) <= 1000);
    }

    // 7. depth 8: frozen
    #[test]
    fn t7_depth_frozen() {
        let inner = json!({"s": "d".repeat(5000)});
        let mut deep = inner.clone();
        for _ in 0..9 {
            deep = json!({"n": deep});
        }
        let mut t = trim(&deep, 500);
        for _ in 0..8 {
            t = t["n"].clone();
        }
        assert!(t["n"] == inner || t == inner || js_stringify(&t).contains(&"d".repeat(5000)));
    }

    // 8. a surrogate pair at the cut is never split (Rust strings can't hold lone surrogates; check the size and
    // that whole emoji survive on both sides)
    #[test]
    fn t8_surrogates() {
        let s = "😀".repeat(500);
        let t = trim(&json!(s), 301);
        let ts = t.as_str().unwrap();
        assert!(sz(&t) <= 301, "{}", sz(&t));
        let (h, tl) = split_cut(ts);
        assert!(h.chars().all(|c| c == '😀') && tl.chars().all(|c| c == '😀') && !h.is_empty());
        // the output uses most of the budget
        assert!(sz(&t) > 280, "{}", sz(&t));
    }

    // 9. a 2000-char string at budget 2000: quotes and marker taken off, 75/25
    #[test]
    fn t9_exact_split() {
        let t = trim(&json!("x".repeat(2000)), 2000);
        let (h, tl) = split_cut(t.as_str().unwrap());
        assert_eq!(sz(&t), 2000);
        assert_eq!(h.len(), 1494);
        assert_eq!(tl.len(), 497);
    }

    // 10. text that isn't json falls back to head + tail, no panic
    #[test]
    fn t10_garbage_json() {
        let (text, cut) = trim_tool_text(&format!("bash {{not json {}", "g".repeat(5000)), 2000);
        assert!(cut && js_len(&text) <= 2010 && text.starts_with("bash {not json"), "{}", js_len(&text));
    }

    // escapes count: a string of newlines stays within budget once serialized
    #[test]
    fn escapes_counted() {
        let t = trim(&json!("line\n".repeat(2000)), 1000);
        assert!(sz(&t) <= 1000, "{}", sz(&t));
    }

    // 11. regression (synthetic stand-in for the live task_spawn entry, ~6752 chars, budget 2000)
    #[test]
    fn t11_task_spawn() {
        let input = json!({
            "mode": "handoff",
            "title": "roll out pending tasks + model switch specs",
            "instruction": "Read the spec and build part A.\n".repeat(205),
            "settings": {"model": "claude-opus", "effort": "high"},
        });
        let text = format!("task_spawn {}", js_stringify(&input));
        assert!(js_len(&text) > 6000, "{}", js_len(&text));
        let (slim, cut) = trim_tool_text(&text, 2000);
        assert!(cut);
        let sp = slim.find(' ').unwrap();
        let parsed: Value = serde_json::from_str(&slim[sp + 1..]).expect("still json");
        let k1: Vec<_> = input.as_object().unwrap().keys().collect();
        let k2: Vec<_> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(k1, k2);
        assert_eq!(parsed["mode"], input["mode"]);
        assert_eq!(parsed["title"], input["title"]);
        assert_eq!(parsed["settings"], input["settings"]);
        assert!(parsed["instruction"].as_str().unwrap().contains(CUT));
        assert!(js_len(&slim) <= 2100, "{}", js_len(&slim));
        let preview = format!("{}: {}", parsed["mode"].as_str().unwrap(), parsed["title"].as_str().unwrap());
        assert_eq!(preview, "handoff: roll out pending tasks + model switch specs");
        // shorter spawns are untouched
        let short = format!("task_spawn {}", js_stringify(&json!({"mode": "fork", "title": "t", "instruction": "do it"})));
        assert_eq!(trim_tool_text(&short, 2000), (short.clone(), false));
    }

    // echo-style plain head + tail (slimEntry's echo path)
    #[test]
    fn head_tail_plain() {
        let s = "o".repeat(5000);
        let h = head_tail(&s, 2000);
        assert!(js_len(&h) < 2100 && h.contains("\n…\n"));
        assert_eq!(h, format!("{}\n…\n{}", "o".repeat(1500), "o".repeat(400)));
        assert_eq!(head_tail("short", 2000), "short");
        // a boundary inside a surrogate pair moves outward: the pair is kept whole
        let e = "😀".repeat(10); // 20 units
        let h = head_tail(&e, 5); // head 3 units -> 2 emoji, tail 1 unit -> 1 emoji
        assert_eq!(h, format!("{}\n…\n{}", "😀".repeat(2), "😀"));
        // slice(-0) is the whole string
        assert_eq!(head_tail("abcdefgh", 4), "abc\n…\nabcdefgh");
    }

    #[test]
    fn numbers_and_tiny_budgets() {
        let v = json!({"a": 1.0, "b": "x".repeat(100)});
        let t = trim(&v, 50);
        assert!(sz(&t) <= 50, "{}", js_stringify(&t));
        assert_eq!(trim(&json!(12345), 1), json!(12345));
        assert_eq!(trim(&json!("abcdefghijkl"), 3), json!(CUT));
        assert_eq!(trim(&json!([]), 0), json!([]));
    }
}
