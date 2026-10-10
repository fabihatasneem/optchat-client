//! Small UI components on GPUI alone (text input, popup and context menus, buttons, switch), in place of
//! gpui-component, which does not track the GPUI revision with the iOS backend.
pub mod button;
pub mod input;
pub mod menu;

use gpui::{App, Global, Pixels, px};

/// the text size of inputs (follows the chat text zoom)
pub struct KitFont(pub Pixels);
impl Global for KitFont {}
pub fn set_font_size(cx: &mut App, size: Pixels) {
    cx.set_global(KitFont(size));
}
pub fn font_size(cx: &App) -> Pixels {
    cx.try_global::<KitFont>().map(|f| f.0).unwrap_or(px(13.5))
}

pub fn init(cx: &mut App) {
    input::bind(cx);
    menu::bind(cx);
}
