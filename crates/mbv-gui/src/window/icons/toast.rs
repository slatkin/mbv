//! The four toast-severity icons (design D7 of `gui-floating-toasts`), ported
//! from `@lucide/svelte` 1.70.0 the same way as the icons in the parent
//! module: the shared [`super::icon_shell`] writes Lucide's default svg
//! attributes, the class list `lucide lucide-<name>` (plus one
//! `lucide-<alias>` per alias), and the icon's child elements, character for
//! character. Lucide is ISC-licensed; the notice is
//! `assets/vendor/lucide-ISC.txt`.
//!
//! 1.70.0 names the warning triangle `triangle-exclamation-point`, and
//! `triangle-alert` is one of its aliases: the alias class carries the name
//! the design uses.

use dioxus_native::prelude::*;

use super::{IconProps, icon_shell};

/// The Lucide `info` icon.
#[component]
pub fn Info(props: IconProps) -> Element {
    icon_shell(
        "info",
        &[],
        &props,
        rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 16v-4" }
            path { d: "M12 8h.01" }
        },
    )
}

/// The Lucide `circle-check` icon (`lucide-check-circle-2` is its alias
/// class).
#[component]
pub fn CircleCheck(props: IconProps) -> Element {
    icon_shell(
        "circle-check",
        &["check-circle-2"],
        &props,
        rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "m16 9-5.5 5.5L8 12" }
        },
    )
}

/// The Lucide warning triangle: 1.70.0's `triangle-exclamation-point`
/// (`lucide-alert-triangle` and `lucide-triangle-alert` are its alias
/// classes).
#[component]
pub fn TriangleAlert(props: IconProps) -> Element {
    icon_shell(
        "triangle-exclamation-point",
        &["alert-triangle", "triangle-alert"],
        &props,
        rsx! {
            path { d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" }
            path { d: "M12 9v4" }
            path { d: "M12 17h.01" }
        },
    )
}

/// The Lucide `circle-x` icon (`lucide-x-circle` is its alias class).
#[component]
pub fn CircleX(props: IconProps) -> Element {
    icon_shell(
        "circle-x",
        &["x-circle"],
        &props,
        rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "m15 9-6 6" }
            path { d: "m9 9 6 6" }
        },
    )
}
