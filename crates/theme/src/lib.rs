//! OptChat theme data layer (GPUI-free).
//!
//! Loads Zed theme-family JSON files and maps them onto a small semantic
//! [`Palette`] that the app uses everywhere. Colors are `u32` in `0xRRGGBBAA`.
//!
//! Zed's style map has no key for some of OptChat's roles (accent2, text2,
//! rail, ...). Those are derived from neighbouring keys, unless the theme's
//! `style` carries an optional `"optchat"` extension object whose keys are
//! palette field names (`"accent2": "#6f86ffff"`, `"levels": [..]`, ...);
//! values found there override the derived ones.

mod color;
mod parse;

pub use color::{contrast, luminance, mix, parse_color, with_alpha};

use color::*;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// A few token colors for code blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Syntax {
    pub keyword: u32,
    pub string: u32,
    pub comment: u32,
    pub number: u32,
    pub function: u32,
    /// `type` token color (`type` is a Rust keyword).
    pub type_: u32,
    pub punctuation: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: u32,
    pub bg2: u32,
    pub rail: u32,
    pub panel: u32,
    pub panel2: u32,
    pub hover: u32,
    pub selected: u32,
    pub line: u32,
    pub line2: u32,
    pub text: u32,
    pub text2: u32,
    pub muted: u32,
    pub faint: u32,
    pub accent: u32,
    pub accent2: u32,
    pub accent_bg: u32,
    pub on_accent: u32,
    pub green: u32,
    pub amber: u32,
    pub red: u32,
    pub green_bg: u32,
    pub amber_bg: u32,
    pub red_bg: u32,
    pub violet: u32,
    pub cyan: u32,
    pub scrollbar: u32,
    pub selection: u32,
    pub shadow: u32,
    pub status_running: u32,
    pub status_paused: u32,
    pub levels: [u32; 8],
    pub code_bg: u32,
    pub syntax: Syntax,
}

#[derive(Clone, Debug)]
pub struct Theme {
    /// Slug, e.g. "solarized-dark".
    pub id: String,
    pub name: String,
    pub family: String,
    pub appearance: Appearance,
    pub palette: Palette,
    /// Where it came from, e.g. "zed:one.json".
    pub source: String,
}

/// Parses a Zed theme family. Lenient: `//` and `/* */` comments and trailing
/// commas are tolerated, unknown keys are ignored, bad colors are treated as
/// missing. Individual themes without a name are skipped.
pub fn load_family(json: &str, source: &str) -> anyhow::Result<Vec<Theme>> {
    let root = parse::lenient_json(json)?;
    let obj = root
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("{source}: theme family must be a JSON object"))?;
    let family = obj.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let list = obj
        .get("themes")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("{source}: missing \"themes\" array"))?;
    let empty = Map::new();
    let mut out = Vec::new();
    for t in list {
        let Some(t) = t.as_object() else { continue };
        let Some(name) = t.get("name").and_then(Value::as_str) else { continue };
        let appearance = match t.get("appearance").and_then(Value::as_str) {
            Some(a) if a.eq_ignore_ascii_case("light") => Appearance::Light,
            _ => Appearance::Dark,
        };
        let style = t.get("style").and_then(Value::as_object).unwrap_or(&empty);
        out.push(Theme {
            id: slug(name),
            name: name.to_string(),
            family: if family.is_empty() { name.to_string() } else { family.clone() },
            appearance,
            palette: build_palette(style, appearance),
            source: source.to_string(),
        });
    }
    if out.is_empty() {
        anyhow::bail!("{source}: no themes found");
    }
    Ok(out)
}

const BUILTIN_FILES: &[(&str, &str)] = &[
    ("optchat:optchat.json", include_str!("../themes/optchat.json")),
    ("zed:one.json", include_str!("../themes/one.json")),
    ("zed:ayu.json", include_str!("../themes/ayu.json")),
    ("zed:gruvbox.json", include_str!("../themes/gruvbox.json")),
    ("zed:solarized.json", include_str!("../themes/solarized.json")),
    ("optchat:tomorrow.json", include_str!("../themes/tomorrow.json")),
];

/// All bundled themes: OptChat Dark, OptChat Light, then the rest by name.
pub fn builtin() -> Vec<Theme> {
    let mut all: Vec<Theme> = BUILTIN_FILES
        .iter()
        .flat_map(|(src, json)| load_family(json, src).expect("bundled theme parses"))
        .collect();
    let rank = |t: &Theme| match t.id.as_str() {
        "optchat-dark" => 0,
        "optchat-light" => 1,
        _ => 2,
    };
    all.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.cmp(&b.name)));
    all
}

pub fn find<'a>(themes: &'a [Theme], id: &str) -> Option<&'a Theme> {
    themes.iter().find(|t| t.id == id)
}

pub fn default_for(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Dark => "optchat-dark",
        Appearance::Light => "optchat-light",
    }
}

fn slug(name: &str) -> String {
    let mut s = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch.to_ascii_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    s.trim_end_matches('-').to_string()
}

// ---------------------------------------------------------------- mapping

struct Style<'a> {
    s: &'a Map<String, Value>,
    ext: Option<&'a Map<String, Value>>,
}

impl Style<'_> {
    fn c(&self, key: &str) -> Option<u32> {
        self.s.get(key).and_then(Value::as_str).and_then(parse_color)
    }
    fn first(&self, keys: &[&str]) -> Option<u32> {
        keys.iter().find_map(|k| self.c(k))
    }
    fn ext(&self, key: &str) -> Option<u32> {
        self.ext?.get(key).and_then(Value::as_str).and_then(parse_color)
    }
    fn syntax(&self, keys: &[&str]) -> Option<u32> {
        let syn = self.s.get("syntax")?.as_object()?;
        keys.iter().find_map(|k| {
            let v = syn.get(*k)?;
            let c = match v {
                Value::String(s) => s.as_str(),
                Value::Object(o) => o.get("color")?.as_str()?,
                _ => return None,
            };
            parse_color(c)
        })
    }
    fn syntax_all(&self) -> Vec<u32> {
        let Some(syn) = self.s.get("syntax").and_then(Value::as_object) else { return vec![] };
        syn.values()
            .filter_map(|v| v.get("color").and_then(Value::as_str).and_then(parse_color))
            .collect()
    }
    fn players(&self) -> Vec<(Option<u32>, Option<u32>)> {
        let Some(ps) = self.s.get("players").and_then(Value::as_array) else { return vec![] };
        ps.iter()
            .map(|p| {
                let g = |k: &str| p.get(k).and_then(Value::as_str).and_then(parse_color);
                (g("cursor").or_else(|| g("background")), g("selection"))
            })
            .collect()
    }
}

fn build_palette(style: &Map<String, Value>, ap: Appearance) -> Palette {
    let dark = ap == Appearance::Dark;
    let st = Style { s: style, ext: style.get("optchat").and_then(Value::as_object) };
    let players = st.players();
    // `ext` wins, then the theme value, then a derivation.
    let pick = |name: &str, theme: Option<u32>| st.ext(name).or(theme);

    let base = if dark { 0x000000ff } else { 0xffffffff };
    let bg = solid(pick("bg", st.first(&["editor.background", "background"])).unwrap_or(if dark {
        0x1e1e1eff
    } else {
        0xffffffff
    }), base);
    let tint = |t: f32| mix(bg, if dark { 0xffffffff } else { 0x000000ff }, t);
    let over = |c: Option<u32>, under: u32| c.map(|c| solid(c, under));

    // Raw text first: many derivations below lean on it.
    let mut text = over(pick("text", st.first(&["text", "editor.foreground"])), bg)
        .unwrap_or(if dark { 0xe6e6e6ff } else { 0x1f1f1fff });
    text = ensure_contrast(text, bg, 4.5, if dark { 0xffffffff } else { 0x000000ff });

    let bg2 = over(pick("bg2", st.first(&["panel.background", "surface.background", "background"])), bg)
        .unwrap_or_else(|| tint(0.03));
    let rail = st.ext("rail").map(|c| solid(c, bg)).unwrap_or_else(|| {
        let tb = over(st.first(&["title_bar.background", "status_bar.background"]), bg).unwrap_or(bg2);
        mix(tb, 0x000000ff, if dark { 0.12 } else { 0.04 })
    });
    let panel = over(pick("panel", st.first(&["elevated_surface.background"])), bg).unwrap_or(bg2);
    let panel2 = over(pick("panel2", st.first(&["element.background"])), panel)
        .unwrap_or_else(|| mix(panel, text, 0.05));
    let hover = over(pick("hover", st.first(&["element.hover", "ghost_element.hover"])), bg2)
        .unwrap_or_else(|| mix(bg2, text, 0.07));
    let selected = over(pick("selected", st.first(&["element.selected", "ghost_element.selected", "element.active"])), bg2)
        .unwrap_or_else(|| mix(hover, text, 0.06));
    let line = over(pick("line", st.first(&["border.variant", "border"])), bg).unwrap_or_else(|| mix(bg, text, 0.1));
    let line2 = over(pick("line2", st.first(&["border", "border.variant"])), bg).unwrap_or_else(|| mix(bg, text, 0.18));

    let mut muted = over(pick("muted", st.first(&["text.muted", "editor.line_number"])), bg)
        .unwrap_or_else(|| mix(text, bg, 0.45));
    muted = ensure_contrast(muted, bg, 2.5, text);
    let text2 = over(st.ext("text2"), bg).unwrap_or_else(|| mix(text, muted, 0.35));
    let mut faint = over(pick("faint", st.first(&["text.placeholder", "text.disabled"])), bg)
        .unwrap_or_else(|| mix(muted, bg, 0.35));
    faint = ensure_contrast(faint, bg, 1.6, text);

    let accent = over(
        pick("accent", st.first(&["text.accent", "icon.accent"]).or(players.first().and_then(|p| p.0))),
        bg,
    )
    .unwrap_or(if dark { 0x8ea2ffff } else { 0x4a5bd4ff });
    let accent = ensure_contrast(accent, bg, 2.5, text);
    let accent2 = over(st.ext("accent2"), bg).unwrap_or_else(|| {
        let (h, s, l) = to_hsl(accent);
        if dark { from_hsl(h, (s + 0.05).min(1.0), (l - 0.06).max(0.2)) } else { from_hsl(h, s, (l - 0.05).max(0.1)) }
    });
    let on_accent = st.ext("on_accent").unwrap_or_else(|| {
        if contrast(0xffffffff, accent2) >= 3.0 { 0xffffffff } else { 0x000000ff }
    });
    let accent_bg = st.ext("accent_bg").unwrap_or_else(|| with_alpha(accent, 0.12));

    let status = |keys: &[&str], fb: u32| {
        let c = over(st.first(keys), bg).unwrap_or(fb);
        ensure_contrast(c, bg, 2.0, text)
    };
    let green = over(st.ext("green"), bg)
        .unwrap_or_else(|| status(&["created", "success", "version_control.added", "terminal.ansi.green"], 0x3fb950ff));
    let amber = over(st.ext("amber"), bg)
        .unwrap_or_else(|| status(&["modified", "warning", "version_control.modified", "terminal.ansi.yellow"], 0xd29922ff));
    let red = over(st.ext("red"), bg)
        .unwrap_or_else(|| status(&["deleted", "error", "version_control.deleted", "terminal.ansi.red"], 0xf85149ff));
    let tint12 = |name: &str, c: u32| st.ext(name).unwrap_or_else(|| with_alpha(c, 0.12));
    let (green_bg, amber_bg, red_bg) = (tint12("green_bg", green), tint12("amber_bg", amber), tint12("red_bg", red));

    // Hue-matched colour picker over everything colourful in the theme.
    let mut pool: Vec<u32> = Vec::new();
    for k in ["cyan", "blue", "magenta", "green", "yellow", "red", "bright_cyan", "bright_blue", "bright_magenta", "bright_green", "bright_yellow", "bright_red"] {
        pool.extend(st.c(&format!("terminal.ansi.{k}")));
    }
    pool.extend(players.iter().filter_map(|p| p.0));
    pool.extend(st.syntax_all());
    pool.extend([accent, green, amber, red]);
    let picker = HuePicker::new(pool, bg, dark);

    let violet = over(st.ext("violet"), bg).unwrap_or_else(|| picker.pick(275.0, &[]));
    let cyan = over(st.ext("cyan"), bg).unwrap_or_else(|| picker.pick(190.0, &[]));

    let levels = ext_levels(&st).unwrap_or_else(|| {
        // OptChat order: cyan, blue, violet, pink, amber, green, orange, lime.
        let mut used = vec![cyan, accent, violet, amber, green];
        let pink = picker.pick(320.0, &used);
        used.push(pink);
        let orange = picker.pick(18.0, &used);
        used.push(orange);
        let lime = picker.pick(88.0, &used);
        [cyan, accent, violet, pink, amber, green, orange, lime]
    });

    let scrollbar = pick("scrollbar", st.first(&["scrollbar.thumb.background"])).unwrap_or_else(|| with_alpha(text, 0.22));
    let selection = pick("selection", players.first().and_then(|p| p.1)).unwrap_or_else(|| with_alpha(accent, 0.3));
    let shadow = st.ext("shadow").unwrap_or(if dark { 0x00000080 } else { 0x0000002a });

    let status_bar = |c: u32| -> u32 {
        // Dark: the colour darkened until `text` reads on it. Light: a pale tint.
        let target = if dark { 0x000000ff } else { bg };
        let mut t = if dark { 0.35 } else { 0.78 };
        let mut out = mix(c, target, t);
        while contrast(text, out) < 4.5 && t < 1.0 {
            t = (t + 0.04).min(1.0);
            out = mix(c, target, t);
        }
        out
    };
    let status_running = over(st.ext("status_running"), bg).unwrap_or_else(|| status_bar(accent2));
    let status_paused = over(st.ext("status_paused"), bg).unwrap_or_else(|| status_bar(amber));

    let code_bg = over(st.ext("code_bg"), bg).unwrap_or_else(|| {
        if bg2 != bg { mix(bg, bg2, 0.5) } else { tint(0.03) }
    });

    let tok = |keys: &[&str], fb: u32| over(st.syntax(keys), code_bg).unwrap_or(fb);
    let syntax = Syntax {
        keyword: tok(&["keyword"], violet),
        string: tok(&["string"], green),
        comment: tok(&["comment", "comment.doc"], muted),
        number: tok(&["number", "constant", "boolean"], amber),
        function: tok(&["function", "function.method"], accent),
        type_: tok(&["type", "constructor", "type.builtin"], cyan),
        punctuation: tok(&["punctuation", "punctuation.delimiter", "punctuation.bracket", "operator"], muted),
    };

    Palette {
        bg, bg2, rail, panel, panel2, hover, selected, line, line2,
        text, text2, muted, faint,
        accent, accent2, accent_bg, on_accent,
        green, amber, red, green_bg, amber_bg, red_bg,
        violet, cyan, scrollbar, selection, shadow,
        status_running, status_paused, levels, code_bg, syntax,
    }
}

fn ext_levels(st: &Style) -> Option<[u32; 8]> {
    let arr = st.ext?.get("levels")?.as_array()?;
    let v: Vec<u32> = arr.iter().filter_map(|c| c.as_str().and_then(parse_color)).collect();
    v.try_into().ok()
}

/// Picks the theme colour closest in hue to a target; synthesises one in the
/// theme's typical saturation/lightness when nothing is close enough.
struct HuePicker {
    pool: Vec<(u32, f32)>,
    bg: u32,
    s: f32,
    l: f32,
}

impl HuePicker {
    fn new(raw: Vec<u32>, bg: u32, dark: bool) -> Self {
        let mut pool = Vec::new();
        let (mut ss, mut ls) = (0.0, 0.0);
        for c in raw {
            let c = solid(c, bg);
            let (h, s, l) = to_hsl(c);
            if s >= 0.25 && (0.15..=0.9).contains(&l) && contrast(c, bg) >= 2.0 && !pool.iter().any(|(p, _)| *p == c) {
                ss += s;
                ls += l;
                pool.push((c, h));
            }
        }
        let n = pool.len() as f32;
        let (s, l) = if pool.is_empty() {
            if dark { (0.75, 0.7) } else { (0.65, 0.42) }
        } else {
            (ss / n, ls / n)
        };
        HuePicker { pool, bg, s, l }
    }

    fn pick(&self, hue: f32, used: &[u32]) -> u32 {
        let best = self
            .pool
            .iter()
            .filter(|(c, _)| !used.contains(c))
            .map(|(c, h)| (*c, hue_dist(*h, hue)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((c, d)) if d <= 25.0 => c,
            _ => {
                let c = from_hsl(hue, self.s.clamp(0.45, 0.9), self.l);
                let toward = if luminance(self.bg) < 0.5 { 0xffffffff } else { 0x000000ff };
                ensure_contrast(c, self.bg, 2.5, toward)
            }
        }
    }
}

#[cfg(test)]
mod tests;
