//! Popup menus on GPUI alone (replaces gpui-component's menu): `PopupMenu` (items with a check, labels, separators;
//! ↑ ↓ Enter Esc; dismissed on a click outside), context menus on any element with an id (`.context_menu(…)`), and
//! the menu layer the window's root draws (`layer`), which holds the one open menu.
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::rc::Rc;

actions!(popup_menu, [MenuUp, MenuDown, MenuConfirm, MenuCancel]);

pub fn bind(cx: &mut App) {
    let c = Some("PopupMenu");
    cx.bind_keys([
        KeyBinding::new("up", MenuUp, c),
        KeyBinding::new("down", MenuDown, c),
        KeyBinding::new("enter", MenuConfirm, c),
        KeyBinding::new("escape", MenuCancel, c),
    ]);
}

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub struct PopupMenuItem {
    label: SharedString,
    checked: bool,
    handler: Option<Handler>,
}
impl PopupMenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        PopupMenuItem { label: label.into(), checked: false, handler: None }
    }
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.handler = Some(Rc::new(f));
        self
    }
}

enum Row {
    Item(PopupMenuItem),
    Label(SharedString),
    Separator,
}

pub struct PopupMenu {
    rows: Vec<Row>,
    focus: FocusHandle,
    sel: Option<usize>,
    max_h: Option<Pixels>,
    scroll: ScrollHandle,
}
impl EventEmitter<DismissEvent> for PopupMenu {}
impl Focusable for PopupMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PopupMenu {
    pub fn build(window: &mut Window, cx: &mut App, f: impl FnOnce(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu) -> Entity<PopupMenu> {
        cx.new(|cx| {
            let m = PopupMenu { rows: vec![], focus: cx.focus_handle(), sel: None, max_h: None, scroll: ScrollHandle::new() };
            f(m, window, cx)
        })
    }
    pub fn item(mut self, i: PopupMenuItem) -> Self {
        self.rows.push(Row::Item(i));
        self
    }
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.rows.push(Row::Label(l.into()));
        self
    }
    pub fn separator(mut self) -> Self {
        if !self.rows.is_empty() {
            self.rows.push(Row::Separator);
        }
        self
    }
    pub fn max_h(mut self, h: Pixels) -> Self {
        self.max_h = Some(h);
        self
    }
    pub fn scrollable(self, _on: bool) -> Self {
        self
    }
    fn items(&self) -> Vec<usize> {
        self.rows.iter().enumerate().filter(|(_, r)| matches!(r, Row::Item(_))).map(|(i, _)| i).collect()
    }
    fn step(&mut self, d: isize, cx: &mut Context<Self>) {
        let items = self.items();
        if items.is_empty() {
            return;
        }
        let k = self.sel.and_then(|s| items.iter().position(|i| *i == s));
        let n = items.len() as isize;
        let next = match k {
            Some(k) => ((k as isize + d) % n + n) % n,
            None => if d > 0 { 0 } else { n - 1 },
        };
        self.sel = Some(items[next as usize]);
        self.scroll.scroll_to_item(next as usize);
        cx.notify();
    }
    fn run(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Row::Item(i)) = self.rows.get(ix) {
            let h = i.handler.clone();
            cx.emit(DismissEvent);
            if let Some(h) = h {
                let ev = ClickEvent::default();
                window.defer(cx, move |window, cx| h(&ev, window, cx));
            }
        }
    }
}

impl Render for PopupMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = crate::theme::c(cx).clone();
        let mut list = div().id("popup-rows").flex().flex_col().py(px(4.)).overflow_y_scroll().track_scroll(&self.scroll).when_some(self.max_h, |d, h| d.max_h(h));
        for (ix, r) in self.rows.iter().enumerate() {
            list = list.child(match r {
                Row::Separator => div().h(px(1.)).my(px(4.)).bg(k.line).into_any_element(),
                Row::Label(l) => div().px(px(10.)).pt(px(6.)).pb(px(2.)).text_size(px(11.)).text_color(k.muted).child(l.clone()).into_any_element(),
                Row::Item(i) => {
                    let on = self.sel == Some(ix);
                    let hover = k.hover;
                    div()
                        .id(("popup-item", ix))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .mx(px(4.))
                        .px(px(6.))
                        .h(px(26.))
                        .rounded(px(5.))
                        .text_size(px(13.))
                        .text_color(k.text)
                        .cursor_pointer()
                        .when(on, |d| d.bg(k.selected))
                        .hover(move |d| d.bg(hover))
                        .child(div().w(px(14.)).child(if i.checked { crate::ui::icon("check", 12., k.accent).into_any_element() } else { div().into_any_element() }))
                        .child(div().whitespace_nowrap().child(i.label.clone()))
                        .on_click(cx.listener(move |m, _, window, cx| m.run(ix, window, cx)))
                        .into_any_element()
                }
            });
        }
        div()
            .key_context("PopupMenu")
            .track_focus(&self.focus)
            .on_action(cx.listener(|m, _: &MenuUp, _, cx| m.step(-1, cx)))
            .on_action(cx.listener(|m, _: &MenuDown, _, cx| m.step(1, cx)))
            .on_action(cx.listener(|m, _: &MenuConfirm, window, cx| {
                if let Some(s) = m.sel {
                    m.run(s, window, cx)
                }
            }))
            .on_action(cx.listener(|_, _: &MenuCancel, _, cx| cx.emit(DismissEvent)))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .occlude()
            .min_w(px(180.))
            .bg(k.panel)
            .border_1()
            .border_color(k.line2)
            .rounded(px(8.))
            .shadow(vec![BoxShadow { color: k.shadow, offset: point(px(0.), px(8.)), blur_radius: px(24.), spread_radius: px(0.), inset: false }])
            .child(list)
    }
}

// ---- the menu layer: one open menu per window, drawn by the root on top of everything ---------------------------------
#[derive(Default)]
pub struct MenuLayer {
    open: Option<(Entity<PopupMenu>, Point<Pixels>, Subscription)>,
    /// focus to give back when it closes
    back: Option<FocusHandle>,
}
impl Global for MenuLayer {}

/// open `menu` at `pos` (window coordinates), focused; Esc, a pick or a click outside closes it
pub fn open_at(menu: Entity<PopupMenu>, pos: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let back = window.focused(cx);
    let sub = window.subscribe(&menu, cx, |_, _: &DismissEvent, window, cx| close(window, cx));
    let fh = menu.read(cx).focus.clone();
    fh.focus(window, cx);
    let l = cx.default_global::<MenuLayer>();
    l.open = Some((menu, pos, sub));
    l.back = back;
    window.refresh();
}
pub fn close(window: &mut Window, cx: &mut App) {
    let l = cx.default_global::<MenuLayer>();
    let was = l.open.take().is_some();
    let back = l.back.take();
    if was {
        if let Some(b) = back {
            b.focus(window, cx);
        }
        window.refresh();
    }
}
pub fn is_open(cx: &App) -> bool {
    cx.try_global::<MenuLayer>().is_some_and(|l| l.open.is_some())
}
/// the open menu, for the root to draw last
pub fn layer(cx: &App) -> Option<AnyElement> {
    let (m, pos, _) = cx.try_global::<MenuLayer>()?.open.as_ref()?;
    Some(deferred(anchored().position(*pos).snap_to_window_with_margin(px(8.)).child(m.clone())).with_priority(3).into_any_element())
}

/// `.context_menu(|menu, window, cx| menu.item(…))` on an element with an id: a right click opens it at the mouse
pub trait ContextMenuExt: InteractiveElement + Sized {
    fn context_menu(self, f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static) -> Self {
        let f = Rc::new(f);
        self.on_mouse_down(MouseButton::Right, move |e: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            let f = f.clone();
            let menu = PopupMenu::build(window, cx, move |m, w, c| f(m, w, c));
            open_at(menu, e.position, window, cx);
        })
    }
}
impl<E: InteractiveElement + Sized> ContextMenuExt for E {}
