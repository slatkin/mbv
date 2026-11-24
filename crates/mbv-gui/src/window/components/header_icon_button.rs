//! The list headers' icon button (gui-list-header-buttons design D2): the one
//! 28 × 28 px square control the Library title line paints for its refresh and
//! its tree actions, and the Playlists screen paints for its refresh. The
//! chrome lives here and nowhere else, so every control on a title line has one
//! border, one radius, one rest fill, and one icon tone, and the line keeps the
//! height its sort control and dropdown already have.
//!
//! The button owns the chrome only: the caller passes the icon as `children`,
//! so the part carries no icon table and the three Lucide glyphs stay in
//! `window::icons`. `label` names the control twice — as its `aria-label` for
//! assistive technology and as its `title` tooltip — and `hook` is the
//! `data-*` attribute the caller's tests query.

use dioxus_native::prelude::*;

/// The header buttons' icon size in CSS pixels (design D2).
pub const HEADER_ICON_SIZE: u16 = 14;

/// The header buttons' icon stroke width (design D2).
pub const HEADER_ICON_STROKE_WIDTH: f32 = 1.6;

/// The props of the header icon button (the `HeaderIconButton` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct HeaderIconButtonProps {
    /// The control's accessible name, which is also its tooltip.
    pub label: &'static str,
    /// The `data-*` attribute that marks the control for its tests.
    pub hook: &'static str,
    /// A press on the control.
    pub onpress: Callback<()>,
    /// The control's icon, sized with [`HEADER_ICON_SIZE`] and
    /// [`HEADER_ICON_STROKE_WIDTH`].
    pub children: Element,
}

/// The one header icon button.
#[component]
pub fn HeaderIconButton(props: HeaderIconButtonProps) -> Element {
    let attrs = vec![Attribute::new(props.hook, "", None, false)];
    let onpress = props.onpress;
    rsx! {
        button {
            "type": "button",
            class: "flex flex-none items-center justify-center",
            style: "width: 28px; height: 28px; padding: 0; background: none; border: 1px solid var(--canvas-border); border-radius: 6px; color: var(--canvas-muted); cursor: pointer;",
            "aria-label": props.label,
            title: props.label,
            onclick: move |_| onpress.call(()),
            ..attrs,
            {props.children}
        }
    }
}
