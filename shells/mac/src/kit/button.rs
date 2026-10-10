//! A small button with an optional dropdown menu (replaces gpui-component's Button + DropdownMenu), and a switch.
use super::menu::{open_at, PopupMenu};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::Cell;
use std::rc::Rc;

type MenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<SharedString>,
    ghost: bool,
    xsmall: bool,
    caret: bool,
    menu: Option<MenuBuilder>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Button { id: id.into(), label: None, icon: None, ghost: false, xsmall: false, caret: false, menu: None }
    }
    pub fn ghost(mut self) -> Self {
        self.ghost = true;
        self
    }
    pub fn outline(mut self) -> Self {
        self.ghost = false;
        self
    }
    pub fn xsmall(mut self) -> Self {
        self.xsmall = true;
        self
    }
    pub fn small(self) -> Self {
        self
    }
    /// an icon by asset name (e.g. "cpu")
    pub fn icon(mut self, name: impl Into<SharedString>) -> Self {
        self.icon = Some(name.into());
        self
    }
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn dropdown_caret(mut self, on: bool) -> Self {
        self.caret = on;
        self
    }
    pub fn dropdown_menu(mut self, f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static) -> Self {
        self.menu = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let k = crate::theme::c(cx).clone();
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let b2 = bounds.clone();
        let hover = k.hover;
        let menu = self.menu.clone();
        div()
            .id(self.id)
            .relative()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(if self.xsmall { 6. } else { 8. }))
            .h(px(if self.xsmall { 20. } else { 24. }))
            .rounded(px(5.))
            .text_size(px(if self.xsmall { 11.5 } else { 12.5 }))
            .cursor_pointer()
            .when(!self.ghost, |d| d.border_1().border_color(k.line2).bg(k.bg))
            .hover(move |d| d.bg(hover))
            .when_some(self.icon, |d, i| d.child(crate::ui::icon(&i, 12., k.muted)))
            .when_some(self.label, |d, l| d.child(div().whitespace_nowrap().child(l)))
            .when(self.caret, |d| d.child(crate::ui::icon("chevron-down", 10., k.muted)))
            .child(canvas(move |b, _, _| b2.set(b), |_, _, _, _| {}).absolute().size_full().top_0().left_0())
            .when_some(menu, |d, m| {
                d.on_click(move |_, window, cx| {
                    let m = m.clone();
                    let menu = PopupMenu::build(window, cx, move |p, w, c| m(p, w, c));
                    let b = bounds.get();
                    open_at(menu, point(b.left(), b.bottom() + px(2.)), window, cx);
                })
            })
    }
}

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    on: bool,
    handler: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
}
impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Switch { id: id.into(), on: false, handler: None }
    }
    pub fn checked(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.handler = Some(Rc::new(f));
        self
    }
}
impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let k = crate::theme::c(cx).clone();
        let on = self.on;
        let h = self.handler.clone();
        div()
            .id(self.id)
            .flex_none()
            .w(px(30.))
            .h(px(18.))
            .rounded_full()
            .p(px(2.))
            .cursor_pointer()
            .bg(if on { k.accent } else { k.line2 })
            .child(div().size(px(14.)).rounded_full().bg(white()).when(on, |d| d.ml(px(12.))))
            .when_some(h, |d, h| d.on_click(move |_, window, cx| h(&!on, window, cx)))
    }
}
