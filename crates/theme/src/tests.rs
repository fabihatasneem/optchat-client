use super::*;
use std::collections::HashSet;

#[test]
fn all_bundled_parse_and_order() {
    let all = builtin();
    assert!(all.len() >= 17, "got {}", all.len());
    assert_eq!(all[0].id, "optchat-dark");
    assert_eq!(all[1].id, "optchat-light");
    let rest: Vec<&str> = all[2..].iter().map(|t| t.name.as_str()).collect();
    let mut sorted = rest.clone();
    sorted.sort();
    assert_eq!(rest, sorted);
    for id in [
        "one-dark", "one-light", "ayu-dark", "ayu-light", "ayu-mirage", "gruvbox-dark",
        "gruvbox-dark-hard", "gruvbox-dark-soft", "gruvbox-light", "gruvbox-light-hard",
        "gruvbox-light-soft", "solarized-dark", "solarized-light", "tomorrow-night",
        "tomorrow-night-blue",
    ] {
        assert!(find(&all, id).is_some(), "missing {id}");
    }
    assert!(find(&all, default_for(Appearance::Dark)).is_some());
    assert_eq!(find(&all, default_for(Appearance::Light)).unwrap().appearance, Appearance::Light);
    assert_eq!(find(&all, "solarized-light").unwrap().appearance, Appearance::Light);
}

#[test]
fn ids_unique() {
    let all = builtin();
    let ids: HashSet<_> = all.iter().map(|t| t.id.clone()).collect();
    assert_eq!(ids.len(), all.len());
}

#[test]
fn optchat_dark_exact() {
    let all = builtin();
    let p = &find(&all, "optchat-dark").unwrap().palette;
    let want = [
        ("bg", p.bg, 0x0b0d10ff), ("bg2", p.bg2, 0x0f1215ff), ("panel", p.panel, 0x121519ff),
        ("panel2", p.panel2, 0x171b20ff), ("hover", p.hover, 0x1c2127ff), ("line", p.line, 0x22282fff),
        ("line2", p.line2, 0x2c333bff), ("text", p.text, 0xe7e9ecff), ("text2", p.text2, 0xb7bdc6ff),
        ("muted", p.muted, 0x7d8592ff), ("faint", p.faint, 0x565e69ff), ("accent", p.accent, 0x8ea2ffff),
        ("accent2", p.accent2, 0x6f86ffff), ("accent_bg", p.accent_bg, 0x8ea2ff1f),
        ("green", p.green, 0x5fd38dff), ("amber", p.amber, 0xf2b45aff), ("red", p.red, 0xff6b6bff),
        ("violet", p.violet, 0xc49bffff), ("cyan", p.cyan, 0x5cc8e0ff), ("rail", p.rail, 0x08090bff),
        ("code_bg", p.code_bg, 0x0d1014ff), ("shadow", p.shadow, 0x00000080),
    ];
    for (n, got, exp) in want {
        assert_eq!(got, exp, "{n}: got {got:08x} want {exp:08x}");
    }
    assert_eq!(
        p.levels,
        [0x5cc8e0ff, 0x8ea2ffff, 0xc49bffff, 0xf28bd8ff, 0xf2b45aff, 0x5fd38dff, 0xff8a65ff, 0x9ccc65ff]
    );
    assert!(contrast(p.text, p.status_running) >= 4.5);
}

#[test]
fn readable_text_everywhere() {
    for t in builtin() {
        let p = &t.palette;
        let (c1, c2) = (contrast(p.text, p.bg), contrast(p.muted, p.bg));
        assert!(c1 >= 4.5, "{}: text contrast {c1}", t.id);
        assert!(c2 >= 2.5, "{}: muted contrast {c2}", t.id);
        assert!(contrast(p.text, p.status_running) >= 4.5, "{}: status_running", t.id);
        assert!(contrast(p.text, p.status_paused) >= 4.5, "{}: status_paused", t.id);
        for c in [p.bg, p.bg2, p.rail, p.panel, p.panel2, p.hover, p.selected, p.text, p.muted, p.accent, p.accent2] {
            assert_eq!(c & 0xff, 0xff, "{}: {c:08x} not opaque", t.id);
        }
        let lv: HashSet<_> = p.levels.iter().collect();
        assert_eq!(lv.len(), 8, "{}: levels not distinct", t.id);
        let dark = t.appearance == Appearance::Dark;
        assert_eq!(luminance(p.bg) < 0.5, dark, "{}: appearance vs bg", t.id);
    }
}

#[test]
fn color_math() {
    assert_eq!(mix(0x000000ff, 0xffffffff, 0.0), 0x000000ff);
    assert_eq!(mix(0x000000ff, 0xffffffff, 1.0), 0xffffffff);
    assert_eq!(mix(0x000000ff, 0xffffffff, 0.5), 0x808080ff);
    assert_eq!(mix(0x10203040, 0x10203040, 0.3), 0x10203040);
    assert_eq!(with_alpha(0x8ea2ffff, 0.12), 0x8ea2ff1f);
    assert_eq!(with_alpha(0x112233ff, 0.0), 0x11223300);
    assert_eq!(with_alpha(0x11223300, 1.0), 0x112233ff);
    assert!((luminance(0xffffffff) - 1.0).abs() < 1e-4);
    assert!(luminance(0x000000ff).abs() < 1e-6);
    assert!((contrast(0x000000ff, 0xffffffff) - 21.0).abs() < 0.01);
    assert_eq!(parse_color("#abc"), Some(0xaabbccff));
    assert_eq!(parse_color("#abcd"), Some(0xaabbccdd));
    assert_eq!(parse_color("#a1b2c3"), Some(0xa1b2c3ff));
    assert_eq!(parse_color("#a1b2c3d4"), Some(0xa1b2c3d4));
    assert_eq!(parse_color("nope"), None);
    assert_eq!(parse_color("#12345"), None);
    assert_eq!(color::solid(0xffffff80, 0x000000ff), 0x808080ff);
}

#[test]
fn sparse_theme_full_palette() {
    let json = r##"{
      // a user theme with almost nothing in it
      "name": "Sparse",
      "themes": [
        { "name": "Sparse Dark", "appearance": "dark", "style": { "background": "#202020", "text": null, }, },
        { "name": "Sparse Light", "appearance": "light", "style": { "editor.background": "#fff8", "unknown.key": 42 } },
        { "name": "Bare", "appearance": "light" },
      ],
    }"##;
    let ts = load_family(json, "test").unwrap();
    assert_eq!(ts.len(), 3);
    assert_eq!(ts[0].id, "sparse-dark");
    assert_eq!(ts[0].family, "Sparse");
    assert_eq!(ts[0].palette.bg, 0x202020ff);
    // translucent bg composited over white for light themes
    assert_eq!(ts[1].palette.bg & 0xff, 0xff);
    for t in &ts {
        let p = &t.palette;
        assert!(contrast(p.text, p.bg) >= 4.5, "{}", t.id);
        assert!(contrast(p.muted, p.bg) >= 2.5, "{}", t.id);
        assert_ne!(p.accent, p.bg);
        assert_ne!(p.hover, p.bg2);
        let lv: HashSet<_> = p.levels.iter().collect();
        assert_eq!(lv.len(), 8, "{}: levels distinct", t.id);
    }
}

#[test]
fn rejects_garbage() {
    assert!(load_family("not json", "x").is_err());
    assert!(load_family("{\"name\":\"x\"}", "x").is_err());
    assert!(load_family("[]", "x").is_err());
}

#[test]
fn slugs() {
    assert_eq!(slug("OptChat Dark"), "optchat-dark");
    assert_eq!(slug("  Gruvbox Dark (Hard)! "), "gruvbox-dark-hard");
}

/// `cargo test -p optchat-theme dump -- --ignored --nocapture` prints every palette.
#[test]
#[ignore]
fn dump() {
    for t in builtin() {
        let p = &t.palette;
        println!(
            "{:22} bg {:08x} bg2 {:08x} rail {:08x} panel {:08x} hov {:08x} txt {:08x} t2 {:08x} mut {:08x} ({:.2}) acc {:08x} acc2 {:08x} on {:08x} vio {:08x} cy {:08x} run {:08x} pau {:08x} code {:08x}\n{:22} levels {:08x?}",
            t.id, p.bg, p.bg2, p.rail, p.panel, p.hover, p.text, p.text2, p.muted, contrast(p.muted, p.bg),
            p.accent, p.accent2, p.on_accent, p.violet, p.cyan, p.status_running, p.status_paused, p.code_bg, "", p.levels
        );
    }
}
