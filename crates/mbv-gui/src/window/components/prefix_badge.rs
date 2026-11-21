//! The armed prefix badge (design D15): the one `PREFIX` badge the window
//! paints while prefix mode stays armed, as the TUI's status row paints it.
//! Three surfaces carry it — the Queue footer's centre, the Library Status
//! bar's left edge, and the Settings status line's leading group — so the
//! paint lives here once and a surface cannot drift from the others.
//!
//! The badge is the same mono bold `PREFIX` in the accent role everywhere.
//! Each surface keeps what is its own: the class names its band's layout
//! needs, and its `data-*` hook, so each surface's tests keep their hooks.

use dioxus_native::prelude::*;

/// The props of the badge (the `PrefixBadge` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct PrefixBadgeProps {
    /// The badge `span`'s class names: the surface's own.
    pub class: &'static str,
    /// The surface's own `data-*` hook, or `None` for a surface that paints
    /// none.
    #[props(default)]
    pub hook: Option<&'static str>,
}

/// The armed prefix badge.
#[component]
pub fn PrefixBadge(props: PrefixBadgeProps) -> Element {
    let hook = props
        .hook
        .map(|name| vec![Attribute::new(name, "", None, false)])
        .unwrap_or_default();

    rsx! {
        span {
            class: "{props.class}",
            style: "color: var(--mbv-accent);",
            ..hook,
            "PREFIX"
        }
    }
}
