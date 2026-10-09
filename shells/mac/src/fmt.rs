//! Number/time formatting (app/ui/format.ts).
use optchat_engine::types::{iso_ms, parse_iso, Cost, Usage};

pub fn tokens(n: i64) -> String {
    let v = n;
    if v < 1000 {
        return v.to_string();
    }
    if v < 100_000 {
        let s = format!("{:.1}", v as f64 / 1000.0);
        return format!("{}k", s.trim_end_matches(".0"));
    }
    if v < 1_000_000 {
        return format!("{}k", (v as f64 / 1000.0).round() as i64);
    }
    format!("{:.2}M", v as f64 / 1e6)
}

pub fn cost(c: Option<&Cost>) -> String {
    let Some(c) = c else { return "—".into() };
    if !c.priced && c.total == 0.0 {
        return "unpriced".into();
    }
    let t = c.total;
    let s = if t == 0.0 { "$0".into() } else if t < 0.01 { format!("${:.4}", t) } else if t < 1.0 { format!("${:.3}", t) } else { format!("${:.2}", t) };
    if c.priced { s } else { format!("{s}*") }
}
pub fn usd(total: f64) -> String {
    cost(Some(&Cost { total, priced: true, ..Default::default() }))
}

pub fn bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MB", n as f64 / 1048576.0)
    }
}

pub fn dur(a: Option<&str>, b: Option<&str>) -> String {
    let (Some(a), Some(b)) = (a, b) else { return String::new() };
    let ms = iso_ms(b) - iso_ms(a);
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}m{}s", ms / 60_000, ((ms % 60_000) as f64 / 1000.0).round() as i64)
    }
}

pub fn time(iso: &str) -> String {
    parse_iso(iso).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default()
}
pub fn datetime(iso: &str) -> String {
    parse_iso(iso).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %Y, %H:%M:%S").to_string()).unwrap_or_default()
}
pub fn day(iso: &str) -> String {
    parse_iso(iso).map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %Y").to_string()).unwrap_or_default()
}

pub fn cache_pct(u: Option<&Usage>) -> i64 {
    match u {
        Some(u) if u.input > 0 => ((u.cache_read as f64 / u.input as f64) * 100.0).round() as i64,
        _ => 0,
    }
}

pub fn ms(v: i64) -> String {
    optchat_engine::metrics::fmt_ms(v as f64)
}

pub fn elapsed(since: &str) -> String {
    let s = ((optchat_engine::types::now_ms() - iso_ms(since)) / 1000).max(0);
    if s >= 60 { format!("{}m {:02}s", s / 60, s % 60) } else { format!("{s}s") }
}

pub fn price(n: Option<f64>) -> String {
    match n {
        None => "—".into(),
        Some(n) => {
            let s = if n < 1.0 { format!("{:.4}", n) } else { format!("{:.3}", n) };
            let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
            format!("${s}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn f() {
        assert_eq!(tokens(999), "999");
        assert_eq!(tokens(1500), "1.5k");
        assert_eq!(tokens(2000), "2k");
        assert_eq!(tokens(150_000), "150k");
        assert_eq!(tokens(5_610_000), "5.61M");
        assert_eq!(usd(3.6412), "$3.64");
        assert_eq!(usd(0.0013), "$0.0013");
        assert_eq!(price(Some(4.0)), "$4");
        assert_eq!(price(Some(0.1)), "$0.1");
    }
}
