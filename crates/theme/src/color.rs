//! Color math on `0xRRGGBBAA` values.

fn ch(c: u32, i: u32) -> f32 {
    ((c >> (24 - 8 * i)) & 0xff) as f32
}

fn pack(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u32;
    (q(r) << 24) | (q(g) << 16) | (q(b) << 8) | q(a)
}

/// Parses `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (the `#` is optional).
pub fn parse_color(s: &str) -> Option<u32> {
    let h = s.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let expand = |h: &str| h.chars().flat_map(|c| [c, c]).collect::<String>();
    let full = match h.len() {
        3 => expand(h) + "ff",
        4 => expand(h),
        6 => format!("{h}ff"),
        8 => h.to_string(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok()
}

/// Linear interpolation of all four channels: `t = 0` → `a`, `t = 1` → `b`.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let l = |i| ch(a, i) + (ch(b, i) - ch(a, i)) * t;
    pack(l(0), l(1), l(2), l(3))
}

/// Replaces the alpha channel (`a` in 0..=1).
pub fn with_alpha(c: u32, a: f32) -> u32 {
    (c & 0xffffff00) | (a.clamp(0.0, 1.0) * 255.0).round() as u32
}

/// WCAG relative luminance (alpha ignored), 0..=1.
pub fn luminance(c: u32) -> f32 {
    let lin = |v: f32| {
        let v = v / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(ch(c, 0)) + 0.7152 * lin(ch(c, 1)) + 0.0722 * lin(ch(c, 2))
}

/// WCAG contrast ratio, 1..=21.
pub fn contrast(a: u32, b: u32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Alpha-composites `fg` over `bg`, returning an opaque color.
pub(crate) fn solid(fg: u32, bg: u32) -> u32 {
    let a = ch(fg, 3) / 255.0;
    if a >= 1.0 {
        return fg | 0xff;
    }
    let bg = bg | 0xff;
    let l = |i| ch(bg, i) + (ch(fg, i) - ch(bg, i)) * a;
    pack(l(0), l(1), l(2), 255.0)
}

/// Moves `c` toward `toward` until it reaches `min` contrast against `bg`
/// (or gets there completely).
pub(crate) fn ensure_contrast(c: u32, bg: u32, min: f32, toward: u32) -> u32 {
    if contrast(c, bg) >= min {
        return c;
    }
    let mut t = 0.0;
    let mut out = c;
    while t < 1.0 && contrast(out, bg) < min {
        t += 0.05;
        out = mix(c, toward, t);
    }
    if contrast(out, bg) < min {
        // `toward` itself was not enough; fall back to black/white.
        let bw = if luminance(bg) < 0.5 { 0xffffffff } else { 0x000000ff };
        return ensure_contrast(out, bg, min, bw);
    }
    out
}

pub(crate) fn to_hsl(c: u32) -> (f32, f32, f32) {
    let (r, g, b) = (ch(c, 0) / 255.0, ch(c, 1) / 255.0, ch(c, 2) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

pub(crate) fn from_hsl(h: f32, s: f32, l: f32) -> u32 {
    let h = h.rem_euclid(360.0) / 360.0;
    if s == 0.0 {
        return pack(l * 255.0, l * 255.0, l * 255.0, 255.0);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f32| {
        if t < 0.0 { t += 1.0 }
        if t > 1.0 { t -= 1.0 }
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t }
        else if t < 0.5 { q }
        else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 }
        else { p }
    };
    pack(f(h + 1.0 / 3.0) * 255.0, f(h) * 255.0, f(h - 1.0 / 3.0) * 255.0, 255.0)
}

pub(crate) fn hue_dist(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}
