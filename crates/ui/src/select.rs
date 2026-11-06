//! Selectable text. GPUI's `StyledText` can't be selected, so every block of rendered markdown (paragraphs, headings,
//! list items, table cells, code) is wrapped in a `Selectable`: drag to select (across blocks and messages), double-click
//! selects a word, triple-click the block, shift-click extends; Ctrl+C (⌘C on macOS) copies. A left click anywhere
//! else clears the selection, so the composer's own Ctrl+C keeps working.
//!
//! Blocks are ordered by their key (the element id the feed gives them, like `t42.3.l1.0`): the numbers in it, compared
//! in order, follow the document, so a selection survives its first block scrolling out of view.
use gpui::*;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

type Pos = (SharedString, usize);

#[derive(Default)]
struct Sel {
    anchor: Option<Pos>,
    focus: Option<Pos>,
    dragging: bool,
    texts: HashMap<SharedString, SharedString>,
    views: HashSet<EntityId>,
}

thread_local!(static SEL: RefCell<Sel> = RefCell::new(Sel::default()));

fn with<R>(f: impl FnOnce(&mut Sel) -> R) -> R {
    SEL.with(|s| f(&mut s.borrow_mut()))
}

/// the numbers in a key, in order: `t42.3.l1.0` -> [42, 3, 1, 0]
fn nums(k: &str) -> Vec<u64> {
    let mut out = vec![];
    let mut cur: Option<u64> = None;
    for ch in k.chars() {
        if let Some(d) = ch.to_digit(10) {
            cur = Some(cur.unwrap_or(0).saturating_mul(10).saturating_add(d as u64));
        } else if let Some(n) = cur.take() {
            out.push(n);
        }
    }
    out.extend(cur);
    out
}

fn key_cmp(a: &str, b: &str) -> Ordering {
    nums(a).cmp(&nums(b)).then_with(|| a.cmp(b))
}

fn pos_cmp(a: &Pos, b: &Pos) -> Ordering {
    key_cmp(&a.0, &b.0).then(a.1.cmp(&b.1))
}

/// the ordered (start, end) of a non-empty selection
fn ordered(s: &Sel) -> Option<(Pos, Pos)> {
    let (a, f) = (s.anchor.clone()?, s.focus.clone()?);
    match pos_cmp(&a, &f) {
        Ordering::Equal => None,
        Ordering::Less => Some((a, f)),
        Ordering::Greater => Some((f, a)),
    }
}

/// the selected byte range of the block `key` (of length `len`)
fn range_in(s: &Sel, key: &str, len: usize) -> Option<Range<usize>> {
    let (st, en) = ordered(s)?;
    if key_cmp(key, &st.0) == Ordering::Less || key_cmp(key, &en.0) == Ordering::Greater {
        return None;
    }
    let a = if *st.0 == *key { st.1.min(len) } else { 0 };
    let b = if *en.0 == *key { en.1.min(len) } else { len };
    (b > a).then_some(a..b)
}

/// the selected text, blocks joined by newlines
pub fn selected_text() -> Option<String> {
    with(|s| {
        let (st, en) = ordered(s)?;
        let mut keys: Vec<&SharedString> =
            s.texts.keys().filter(|k| key_cmp(k, &st.0) != Ordering::Less && key_cmp(k, &en.0) != Ordering::Greater).collect();
        keys.sort_by(|a, b| key_cmp(a, b));
        let parts: Vec<&str> = keys
            .into_iter()
            .filter_map(|k| {
                let t = &s.texts[k];
                range_in(s, k, t.len()).and_then(|r| t.get(r))
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join("\n"))
    })
}

pub fn clear(cx: &mut App) {
    let views = with(|s| {
        let had = ordered(s).is_some();
        s.anchor = None;
        s.focus = None;
        s.dragging = false;
        if had { s.views.iter().copied().collect() } else { vec![] }
    });
    if !views.is_empty() {
        crate::ui::set_state_extra("selection", serde_json::Value::Null);
    }
    for v in views {
        cx.notify(v);
    }
}

fn changed(cx: &mut App) {
    crate::ui::set_state_extra("selection", serde_json::json!(selected_text()));
    let views: Vec<EntityId> = with(|s| s.views.iter().copied().collect());
    for v in views {
        cx.notify(v);
    }
}

/// Ctrl+C / ⌘C copies the feed's selection (installed once, from kit::init)
pub fn bind(cx: &mut App) {
    cx.intercept_keystrokes(|e, _w, cx| {
        let k = &e.keystroke;
        let m = &k.modifiers;
        if k.key == "c" && m.secondary() && !m.shift && !m.alt {
            if let Some(t) = selected_text() {
                cx.write_to_clipboard(ClipboardItem::new_string(t));
                cx.stop_propagation();
            }
        }
    })
    .detach();
}

fn word_at(text: &str, ix: usize) -> Range<usize> {
    for (start, w) in text.split_word_bound_indices() {
        let end = start + w.len();
        if ix >= start && ix < end {
            return start..end;
        }
    }
    ix.min(text.len())..ix.min(text.len())
}

fn index_at(layout: &TextLayout, p: Point<Pixels>) -> usize {
    match layout.index_for_position(p) {
        Ok(i) | Err(i) => i,
    }
}

/// a text element (usually a `StyledText` or `InteractiveText`) whose `layout` lays out `text`, made selectable as `key`
pub struct Selectable {
    key: SharedString,
    text: SharedString,
    layout: TextLayout,
    child: AnyElement,
}

pub fn selectable(key: impl Into<SharedString>, text: impl Into<SharedString>, layout: TextLayout, child: impl IntoElement) -> Selectable {
    Selectable { key: key.into(), text: text.into(), layout, child: child.into_any_element() }
}

impl IntoElement for Selectable {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Selectable {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _id: Option<&GlobalElementId>, _i: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(&mut self, _id: Option<&GlobalElementId>, _i: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Hitbox {
        self.child.prepaint(window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(&mut self, _id: Option<&GlobalElementId>, _i: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), hitbox: &mut Hitbox, window: &mut Window, cx: &mut App) {
        let key = self.key.clone();
        let text = self.text.clone();
        let view = window.current_view();
        let sel = with(|s| {
            s.views.insert(view);
            if s.texts.len() > 20_000 {
                let keep: Vec<SharedString> = [&s.anchor, &s.focus].into_iter().flatten().map(|p| p.0.clone()).collect();
                s.texts.retain(|k, _| keep.contains(k));
            }
            s.texts.insert(key.clone(), text.clone());
            range_in(s, &key, text.len())
        });
        window.set_cursor_style(CursorStyle::IBeam, hitbox);
        crate::ui::set_probe(format!("sel.{key}"), bounds);

        // the selection, behind the text
        if let Some(r) = sel {
            let layout = &self.layout;
            let tb = layout.bounds();
            let lh = layout.line_height();
            let color = crate::theme::c(cx).selection;
            if let (Some(a), Some(b)) = (layout.position_for_index(r.start), layout.position_for_index(r.end)) {
                let mut rects = vec![];
                if a.y == b.y {
                    rects.push(Bounds::from_corners(a, point(b.x, a.y + lh)));
                } else {
                    rects.push(Bounds::from_corners(a, point(tb.right(), a.y + lh)));
                    if b.y > a.y + lh {
                        rects.push(Bounds::from_corners(point(tb.left(), a.y + lh), point(tb.right(), b.y)));
                    }
                    rects.push(Bounds::from_corners(point(tb.left(), b.y), point(b.x.max(tb.left() + px(4.)), b.y + lh)));
                }
                for q in rects {
                    window.paint_quad(fill(q, color));
                }
            }
        }

        self.child.paint(window, cx);

        // a left click anywhere clears the selection first (shift-click keeps it to extend), then the block clicked starts one
        window.on_mouse_event(|e: &MouseDownEvent, phase, _w, cx| {
            if phase == DispatchPhase::Capture && e.button == MouseButton::Left && !e.modifiers.shift {
                clear(cx);
            }
        });
        let hb = hitbox.clone();
        let (k2, t2, l2) = (key.clone(), text.clone(), self.layout.clone());
        window.on_mouse_event(move |e: &MouseDownEvent, phase, w, cx| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left || !hb.is_hovered(w) {
                return;
            }
            let ix = index_at(&l2, e.position);
            with(|s| {
                let (a, f) = match e.click_count {
                    1 if e.modifiers.shift && s.anchor.is_some() => (s.anchor.clone().unwrap(), (k2.clone(), ix)),
                    1 => ((k2.clone(), ix), (k2.clone(), ix)),
                    2 => {
                        let r = word_at(&t2, ix);
                        ((k2.clone(), r.start), (k2.clone(), r.end))
                    }
                    _ => ((k2.clone(), 0), (k2.clone(), t2.len())),
                };
                s.anchor = Some(a);
                s.focus = Some(f);
                s.dragging = e.click_count == 1;
            });
            changed(cx);
        });
        let (k3, l3) = (key, self.layout.clone());
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, _w, cx| {
            if phase != DispatchPhase::Bubble || !bounds.contains(&e.position) {
                return;
            }
            let ix = index_at(&l3, e.position);
            let moved = with(|s| {
                if !s.dragging || e.pressed_button != Some(MouseButton::Left) {
                    s.dragging = false;
                    return false;
                }
                let f = (k3.clone(), ix);
                let moved = s.focus.as_ref() != Some(&f);
                s.focus = Some(f);
                moved
            });
            if moved {
                changed(cx);
            }
        });
        window.on_mouse_event(|e: &MouseUpEvent, phase, _w, _cx| {
            if phase == DispatchPhase::Capture && e.button == MouseButton::Left {
                with(|s| s.dragging = false);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn orders_and_copies() {
        assert_eq!(key_cmp("t9.0", "t10.0"), Ordering::Less);
        assert_eq!(key_cmp("t10.0.l1.0", "t10.0.l0.3"), Ordering::Greater);
        assert_eq!(key_cmp("t10.2", "t10.0.l3.0"), Ordering::Greater);
        with(|s| {
            s.texts.insert("t1.0".into(), "hello world".into());
            s.texts.insert("t1.1".into(), "second para".into());
            s.texts.insert("t2.0".into(), "next message".into());
            s.anchor = Some(("t1.0".into(), 6));
            s.focus = Some(("t2.0".into(), 4));
        });
        assert_eq!(selected_text().as_deref(), Some("world\nsecond para\nnext"));
        with(|s| std::mem::swap(&mut s.anchor, &mut s.focus));
        assert_eq!(selected_text().as_deref(), Some("world\nsecond para\nnext"));
        assert_eq!(word_at("foo bar baz", 5), 4..7);
    }
}
