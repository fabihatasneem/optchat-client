//! Live theme switching: the active OptChat palette as a GPUI global, mapped onto gpui-component's theme colors too.
use gpui::{App, Global, Hsla, Rgba, Window, WindowAppearance};
use optchat_theme::{Appearance, Palette, Theme};

#[derive(Clone)]
pub struct C {
    pub bg: Hsla,
    pub bg2: Hsla,
    pub rail: Hsla,
    pub panel: Hsla,
    pub panel2: Hsla,
    pub hover: Hsla,
    pub selected: Hsla,
    pub line: Hsla,
    pub line2: Hsla,
    pub text: Hsla,
    pub text2: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub accent: Hsla,
    pub accent2: Hsla,
    pub accent_bg: Hsla,
    pub on_accent: Hsla,
    pub green: Hsla,
    pub amber: Hsla,
    pub red: Hsla,
    pub green_bg: Hsla,
    pub amber_bg: Hsla,
    pub red_bg: Hsla,
    pub violet: Hsla,
    pub cyan: Hsla,
    pub scrollbar: Hsla,
    pub selection: Hsla,
    pub shadow: Hsla,
    pub status_running: Hsla,
    pub status_paused: Hsla,
    pub levels: [Hsla; 8],
    pub code_bg: Hsla,
    pub kw: Hsla,
    pub string: Hsla,
    pub comment: Hsla,
    pub number: Hsla,
    pub function: Hsla,
    pub type_: Hsla,
    pub dark: bool,
}

pub fn hx(c: u32) -> Hsla {
    Rgba { r: ((c >> 24) & 0xff) as f32 / 255.0, g: ((c >> 16) & 0xff) as f32 / 255.0, b: ((c >> 8) & 0xff) as f32 / 255.0, a: (c & 0xff) as f32 / 255.0 }.into()
}

impl C {
    pub fn from(p: &Palette, dark: bool) -> C {
        C {
            bg: hx(p.bg),
            bg2: hx(p.bg2),
            rail: hx(p.rail),
            panel: hx(p.panel),
            panel2: hx(p.panel2),
            hover: hx(p.hover),
            selected: hx(p.selected),
            line: hx(p.line),
            line2: hx(p.line2),
            text: hx(p.text),
            text2: hx(p.text2),
            muted: hx(p.muted),
            faint: hx(p.faint),
            accent: hx(p.accent),
            accent2: hx(p.accent2),
            accent_bg: hx(p.accent_bg),
            on_accent: hx(p.on_accent),
            green: hx(p.green),
            amber: hx(p.amber),
            red: hx(p.red),
            green_bg: hx(p.green_bg),
            amber_bg: hx(p.amber_bg),
            red_bg: hx(p.red_bg),
            violet: hx(p.violet),
            cyan: hx(p.cyan),
            scrollbar: hx(p.scrollbar),
            selection: hx(p.selection),
            shadow: hx(p.shadow),
            status_running: hx(p.status_running),
            status_paused: hx(p.status_paused),
            levels: p.levels.map(hx),
            code_bg: hx(p.code_bg),
            kw: hx(p.syntax.keyword),
            string: hx(p.syntax.string),
            comment: hx(p.syntax.comment),
            number: hx(p.syntax.number),
            function: hx(p.syntax.function),
            type_: hx(p.syntax.type_),
            dark,
        }
    }
}

pub struct Themes {
    pub all: Vec<Theme>,
    pub active: String,
    pub follow_system: bool,
    pub c: C,
}
impl Global for Themes {}

pub fn c(cx: &App) -> &C {
    &cx.global::<Themes>().c
}

pub fn init(cx: &mut App, theme_id: Option<String>, follow_system: bool, appearance: WindowAppearance) {
    let all = optchat_theme::builtin();
    let dark = matches!(appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark);
    let id = if follow_system || theme_id.is_none() {
        optchat_theme::default_for(if dark { Appearance::Dark } else { Appearance::Light }).to_string()
    } else {
        theme_id.unwrap()
    };
    let t = optchat_theme::find(&all, &id).cloned().unwrap_or_else(|| all[0].clone());
    let c = C::from(&t.palette, t.appearance == Appearance::Dark);
    cx.set_global(Themes { all, active: t.id.clone(), follow_system, c });
    apply_component_theme(cx);
}

/// Switch to a theme (live). `follow` keeps following the system appearance.
pub fn set(cx: &mut App, id: &str, follow: bool) {
    let th = cx.global::<Themes>();
    let Some(t) = optchat_theme::find(&th.all, id).cloned() else { return };
    let g = cx.global_mut::<Themes>();
    g.active = t.id.clone();
    g.follow_system = follow;
    g.c = C::from(&t.palette, t.appearance == Appearance::Dark);
    apply_component_theme(cx);
    cx.refresh_windows();
}

pub fn on_appearance(cx: &mut App, appearance: WindowAppearance) {
    if !cx.global::<Themes>().follow_system {
        return;
    }
    let dark = matches!(appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark);
    let id = optchat_theme::default_for(if dark { Appearance::Dark } else { Appearance::Light });
    set(cx, id, true);
}

pub fn sync_window(window: &mut Window, cx: &mut App) {
    let a = window.appearance();
    on_appearance(cx, a);
}

fn apply_component_theme(_cx: &mut App) {}
