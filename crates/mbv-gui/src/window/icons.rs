//! The Lucide icons (D13 of `replace-tauri-with-blitz`): the 36 icons the
//! former front end imported, as inline `svg` components over the child elements of
//! `@lucide/svelte` 1.70.0. Lucide is ISC-licensed; the notice is
//! `assets/vendor/lucide-ISC.txt`.
//!
//! the former client call sites render `<Icon size={..} class=".." />`, which
//! Lucide's `buildLucideIconNode` turns into an `svg` with the default
//! attributes (24 × 24 view box, `stroke="currentColor"`, `stroke-width="2"`,
//! round caps and joins, no fill), the class list `lucide lucide-<name>`
//! (plus one `lucide-<alias>` per alias), and the icon's child elements. The
//! ports keep those attributes and class lists character for character, so
//! the ported tests that query `svg.lucide-check` and friends keep working.
//! Call sites that pass more props (`color`, `fill`, `strokeWidth`) pass the
//! same values here; the shared shell names them in the attributes it
//! writes.
//!
//! The one attribute the ports cannot copy character for character is the
//! colour: the former call sites passed a `var(--mbv-…)` string into the
//! `stroke` and `fill` attributes, which a browser resolves and usvg cannot.
//! Blitz paints an inline `svg` by serialising the subtree and re-parsing it
//! with usvg, and usvg has no `var()` support: it drops the stroke and paints
//! the fill black. So the shell writes `stroke="currentColor"` and leaves the
//! role colour to the svg's computed `color`, which the serialiser resolves
//! into the attribute before usvg sees it — the same pattern the hand-written
//! svgs in this window use.

use dioxus_native::prelude::*;
use std::fmt::Write as _;

/// The props the former call sites pass: the icon's size in CSS pixels
/// (Lucide's default 24) and extra classes merged into the svg's `class`.
#[derive(Props, Clone, PartialEq)]
pub struct IconProps {
    /// Extra classes, as a the former client call site's `class="..."` value.
    #[props(default)]
    pub class: String,
    /// The svg's rendered width and height, in CSS pixels.
    #[props(default = 24)]
    pub size: u16,
    /// The colour the icon paints in, as a the former client call site's `color={...}`
    /// value: it lands on the svg's computed `color`, which the stroke and a
    /// filled body read as `currentColor`. Lucide's default is `currentColor`.
    #[props(default = "currentColor".to_owned())]
    pub color: String,
    /// The svg's fill, as a the former client call site's `fill={...}` value. Lucide's
    /// default is `none`; any other value paints the body in [`Self::color`].
    #[props(default = "none".to_owned())]
    pub fill: String,
    /// The svg's stroke width, as a the former client call site's `strokeWidth={...}`
    /// value. Lucide's default is 2.
    #[props(default = 2.0)]
    pub stroke_width: f32,
    /// The svg's `aria-hidden="true"`, as a the former client call site's
    /// `aria-hidden={...}` value. Lucide's default leaves it unset.
    #[props(default)]
    pub aria_hidden: bool,
}

impl std::fmt::Debug for IconProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IconProps")
            .field("class", &self.class)
            .field("size", &self.size)
            .field("color", &self.color)
            .field("fill", &self.fill)
            .field("stroke_width", &self.stroke_width)
            .field("aria_hidden", &self.aria_hidden)
            .finish()
    }
}

/// The `svg` every icon renders: Lucide's default attributes, the Lucide
/// class list, and the icon's own child elements.
fn icon_shell(
    name: &'static str,
    aliases: &[&'static str],
    props: &IconProps,
    children: Element,
) -> Element {
    let mut class = format!("lucide lucide-{name}");
    for alias in aliases {
        let _ = write!(class, " lucide-{alias}");
    }
    if !props.class.is_empty() {
        class.push(' ');
        class.push_str(&props.class);
    }
    let size = props.size;
    let color = &props.color;
    // Lucide's `fill` prop paints the glyph's body, so a call site that sets
    // it names the colour the stroke already has: both reach the paint as
    // `currentColor`, and the svg's computed `color` carries the role.
    let fill = if props.fill == "none" {
        "none"
    } else {
        "currentColor"
    };
    let stroke_width = props.stroke_width;
    let aria_hidden = props.aria_hidden;
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "{fill}",
            stroke: "currentColor",
            stroke_width: "{stroke_width}",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            style: "color: {color};",
            "aria-hidden": aria_hidden.then_some("true"),
            {children}
        }
    }
}

/// The Lucide `arrow-left` icon.
#[component]
pub fn ArrowLeft(props: IconProps) -> Element {
    icon_shell(
        "arrow-left",
        &[],
        &props,
        rsx! {
            path { d: "m12 19-7-7 7-7" }
            path { d: "M19 12H5" }
        },
    )
}

/// The Lucide `audio-lines` icon.
#[component]
pub fn AudioLines(props: IconProps) -> Element {
    icon_shell(
        "audio-lines",
        &[],
        &props,
        rsx! {
            path { d: "M2 10v3" }
            path { d: "M6 6v11" }
            path { d: "M10 3v18" }
            path { d: "M14 8v7" }
            path { d: "M18 5v13" }
            path { d: "M22 10v3" }
        },
    )
}

/// The Lucide `captions` icon (`lucide-subtitles` is its alias class).
#[component]
pub fn Captions(props: IconProps) -> Element {
    icon_shell(
        "captions",
        &["subtitles"],
        &props,
        rsx! {
            rect { width: "18", height: "14", x: "3", y: "5", rx: "2", ry: "2" }
            path { d: "M7 15h4M15 15h2M7 11h2M13 11h4" }
        },
    )
}

/// The Lucide `cast` icon.
#[component]
pub fn Cast(props: IconProps) -> Element {
    icon_shell(
        "cast",
        &[],
        &props,
        rsx! {
            path { d: "M2 8V6a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-6" }
            path { d: "M2 12a9 9 0 0 1 8 8" }
            path { d: "M2 16a5 5 0 0 1 4 4" }
            line { x1: "2", x2: "2.01", y1: "20", y2: "20" }
        },
    )
}

/// The Lucide `check` icon.
#[component]
pub fn Check(props: IconProps) -> Element {
    icon_shell(
        "check",
        &[],
        &props,
        rsx! {
            path { d: "M20 6 9 17l-5-5" }
        },
    )
}

/// The Lucide `chevron-down` icon.
#[component]
pub fn ChevronDown(props: IconProps) -> Element {
    icon_shell(
        "chevron-down",
        &[],
        &props,
        rsx! {
            path { d: "m6 9 6 6 6-6" }
        },
    )
}

/// The Lucide `chevron-up` icon.
#[component]
pub fn ChevronUp(props: IconProps) -> Element {
    icon_shell(
        "chevron-up",
        &[],
        &props,
        rsx! {
            path { d: "m18 15-6-6-6 6" }
        },
    )
}

/// The Lucide `chevron-left` icon.
#[component]
pub fn ChevronLeft(props: IconProps) -> Element {
    icon_shell(
        "chevron-left",
        &[],
        &props,
        rsx! {
            path { d: "m15 18-6-6 6-6" }
        },
    )
}

/// The Lucide `chevron-right` icon.
#[component]
pub fn ChevronRight(props: IconProps) -> Element {
    icon_shell(
        "chevron-right",
        &[],
        &props,
        rsx! {
            path { d: "m9 18 6-6-6-6" }
        },
    )
}

/// The Lucide `chevrons-left` icon.
#[component]
pub fn ChevronsLeft(props: IconProps) -> Element {
    icon_shell(
        "chevrons-left",
        &[],
        &props,
        rsx! {
            path { d: "m11 17-5-5 5-5" }
            path { d: "m18 17-5-5 5-5" }
        },
    )
}

/// The Lucide `clock` icon.
#[component]
pub fn Clock(props: IconProps) -> Element {
    icon_shell(
        "clock",
        &[],
        &props,
        rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 6v6l4 2" }
        },
    )
}

/// The Lucide `ellipsis` icon (`lucide-more-horizontal` is its alias class).
#[component]
pub fn Ellipsis(props: IconProps) -> Element {
    icon_shell(
        "ellipsis",
        &["more-horizontal"],
        &props,
        rsx! {
            circle { cx: "12", cy: "12", r: "1" }
            circle { cx: "19", cy: "12", r: "1" }
            circle { cx: "5", cy: "12", r: "1" }
        },
    )
}

/// The Lucide `grip-vertical` icon.
#[component]
pub fn GripVertical(props: IconProps) -> Element {
    icon_shell(
        "grip-vertical",
        &[],
        &props,
        rsx! {
            circle { cx: "9", cy: "12", r: "1" }
            circle { cx: "9", cy: "5", r: "1" }
            circle { cx: "9", cy: "19", r: "1" }
            circle { cx: "15", cy: "12", r: "1" }
            circle { cx: "15", cy: "5", r: "1" }
            circle { cx: "15", cy: "19", r: "1" }
        },
    )
}

/// The Lucide `keyboard` icon.
#[component]
pub fn Keyboard(props: IconProps) -> Element {
    icon_shell(
        "keyboard",
        &[],
        &props,
        rsx! {
            path { d: "M10 8h.01" }
            path { d: "M12 12h.01" }
            path { d: "M14 8h.01" }
            path { d: "M16 12h.01" }
            path { d: "M18 8h.01" }
            path { d: "M6 8h.01" }
            path { d: "M7 16h10" }
            path { d: "M8 12h.01" }
            rect { width: "20", height: "16", x: "2", y: "4", rx: "2" }
        },
    )
}

/// The Lucide `library` icon.
#[component]
pub fn Library(props: IconProps) -> Element {
    icon_shell(
        "library",
        &[],
        &props,
        rsx! {
            path { d: "m16 6 4 14" }
            path { d: "M12 6v14" }
            path { d: "M8 8v12" }
            path { d: "M4 4v16" }
        },
    )
}

/// The Lucide `list-end` icon.
#[component]
pub fn ListEnd(props: IconProps) -> Element {
    icon_shell(
        "list-end",
        &[],
        &props,
        rsx! {
            path { d: "M16 5H3" }
            path { d: "M16 12H3" }
            path { d: "M9 19H3" }
            path { d: "m16 16-3 3 3 3" }
            path { d: "M21 5v12a2 2 0 0 1-2 2h-6" }
        },
    )
}

/// The Lucide `list-ordered` icon.
#[component]
pub fn ListOrdered(props: IconProps) -> Element {
    icon_shell(
        "list-ordered",
        &[],
        &props,
        rsx! {
            path { d: "M11 5h10" }
            path { d: "M11 12h10" }
            path { d: "M11 19h10" }
            path { d: "M4 4h1v5" }
            path { d: "M4 9h2" }
            path { d: "M6.5 20H3.4c0-1 2.6-1.925 2.6-3.5a1.5 1.5 0 0 0-2.6-1.02" }
        },
    )
}

/// The Lucide `list-plus` icon.
#[component]
pub fn ListPlus(props: IconProps) -> Element {
    icon_shell(
        "list-plus",
        &[],
        &props,
        rsx! {
            path { d: "M16 5H3" }
            path { d: "M11 12H3" }
            path { d: "M16 19H3" }
            path { d: "M18 9v6" }
            path { d: "M21 12h-6" }
        },
    )
}

/// The Lucide `list-video` icon. A hand addition (user request 2026-11-19):
/// the former front end imported 36 icons and this is not one of them, so its
/// port keeps the same `@lucide/svelte` 1.70.0 child elements the rest port
/// from.
#[component]
pub fn ListVideo(props: IconProps) -> Element {
    icon_shell(
        "list-video",
        &[],
        &props,
        rsx! {
            path { d: "M21 5H3" }
            path { d: "M10 12H3" }
            path { d: "M10 19H3" }
            path { d: "M15 12.003a1 1 0 0 1 1.517-.859l4.997 2.997a1 1 0 0 1 0 1.718l-4.997 2.997a1 1 0 0 1-1.517-.86z" }
        },
    )
}

/// The Lucide `loader-circle` icon (`lucide-loader-2` is its alias class).
#[component]
pub fn LoaderCircle(props: IconProps) -> Element {
    icon_shell(
        "loader-circle",
        &["loader-2"],
        &props,
        rsx! {
            path { d: "M21 12a9 9 0 1 1-6.219-8.56" }
        },
    )
}

/// The Lucide `maximize-2` icon.
#[component]
pub fn Maximize2(props: IconProps) -> Element {
    icon_shell(
        "maximize-2",
        &[],
        &props,
        rsx! {
            path { d: "M15 3h6v6" }
            path { d: "m21 3-7 7" }
            path { d: "m3 21 7-7" }
            path { d: "M9 21H3v-6" }
        },
    )
}

/// The Lucide `monitor` icon.
#[component]
pub fn Monitor(props: IconProps) -> Element {
    icon_shell(
        "monitor",
        &[],
        &props,
        rsx! {
            rect { width: "20", height: "14", x: "2", y: "3", rx: "2" }
            line { x1: "8", x2: "16", y1: "21", y2: "21" }
            line { x1: "12", x2: "12", y1: "17", y2: "21" }
        },
    )
}

/// The Lucide `monitor-play` icon.
#[component]
pub fn MonitorPlay(props: IconProps) -> Element {
    icon_shell(
        "monitor-play",
        &[],
        &props,
        rsx! {
            path { d: "M15.033 9.44a.647.647 0 0 1 0 1.12l-4.065 2.352a.645.645 0 0 1-.968-.56V7.648a.645.645 0 0 1 .967-.56z" }
            path { d: "M12 17v4" }
            path { d: "M8 21h8" }
            rect { x: "2", y: "3", width: "20", height: "14", rx: "2" }
        },
    )
}

/// The Lucide `pause` icon.
#[component]
pub fn Pause(props: IconProps) -> Element {
    icon_shell(
        "pause",
        &[],
        &props,
        rsx! {
            rect { x: "14", y: "3", width: "5", height: "18", rx: "1" }
            rect { x: "5", y: "3", width: "5", height: "18", rx: "1" }
        },
    )
}

/// The Lucide `play` icon.
#[component]
pub fn Play(props: IconProps) -> Element {
    icon_shell(
        "play",
        &[],
        &props,
        rsx! {
            path { d: "M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z" }
        },
    )
}

/// The Lucide `plus` icon.
#[component]
pub fn Plus(props: IconProps) -> Element {
    icon_shell(
        "plus",
        &[],
        &props,
        rsx! {
            path { d: "M5 12h14" }
            path { d: "M12 5v14" }
        },
    )
}

/// The Lucide `rotate-ccw` icon.
#[component]
pub fn RotateCcw(props: IconProps) -> Element {
    icon_shell(
        "rotate-ccw",
        &[],
        &props,
        rsx! {
            path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
            path { d: "M3 3v5h5" }
        },
    )
}

/// The Lucide `rss` icon.
#[component]
pub fn Rss(props: IconProps) -> Element {
    icon_shell(
        "rss",
        &[],
        &props,
        rsx! {
            path { d: "M4 11a9 9 0 0 1 9 9" }
            path { d: "M4 4a16 16 0 0 1 16 16" }
            circle { cx: "5", cy: "19", r: "1" }
        },
    )
}

/// The Lucide `save-check` icon.
#[component]
pub fn SaveCheck(props: IconProps) -> Element {
    icon_shell(
        "save-check",
        &[],
        &props,
        rsx! {
            path { d: "M12.5 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h10.2a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4v4.35" }
            path { d: "m16 19 2 2 4-4" }
            path { d: "M17 15.13V14a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" }
            path { d: "M7 3v4a1 1 0 0 0 1 1h7" }
        },
    )
}

/// The Lucide `save-off` icon.
#[component]
pub fn SaveOff(props: IconProps) -> Element {
    icon_shell(
        "save-off",
        &[],
        &props,
        rsx! {
            path { d: "M13 13H8a1 1 0 0 0-1 1v7" }
            path { d: "M14 8h1" }
            path { d: "M17 21v-4" }
            path { d: "m2 2 20 20" }
            path { d: "M20.41 20.41A2 2 0 0 1 19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 .59-1.41" }
            path { d: "M9 3h6.2a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V15" }
        },
    )
}

/// The Lucide `search` icon.
#[component]
pub fn Search(props: IconProps) -> Element {
    icon_shell(
        "search",
        &[],
        &props,
        rsx! {
            path { d: "m21 21-4.34-4.34" }
            circle { cx: "11", cy: "11", r: "8" }
        },
    )
}

/// The Lucide `server` icon.
#[component]
pub fn Server(props: IconProps) -> Element {
    icon_shell(
        "server",
        &[],
        &props,
        rsx! {
            rect { width: "20", height: "8", x: "2", y: "2", rx: "2", ry: "2" }
            rect { width: "20", height: "8", x: "2", y: "14", rx: "2", ry: "2" }
            line { x1: "6", x2: "6.01", y1: "6", y2: "6" }
            line { x1: "6", x2: "6.01", y1: "18", y2: "18" }
        },
    )
}

/// The Lucide `settings` icon.
#[component]
pub fn Settings(props: IconProps) -> Element {
    icon_shell(
        "settings",
        &[],
        &props,
        rsx! {
            path { d: "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915" }
            circle { cx: "12", cy: "12", r: "3" }
        },
    )
}

/// The Lucide `shuffle` icon.
#[component]
pub fn Shuffle(props: IconProps) -> Element {
    icon_shell(
        "shuffle",
        &[],
        &props,
        rsx! {
            path { d: "m18 14 4 4-4 4" }
            path { d: "m18 2 4 4-4 4" }
            path { d: "M2 18h1.973a4 4 0 0 0 3.3-1.7l5.454-8.6a4 4 0 0 1 3.3-1.7H22" }
            path { d: "M2 6h1.972a4 4 0 0 1 3.6 2.2" }
            path { d: "M22 18h-6.041a4 4 0 0 1-3.3-1.8l-.359-.45" }
        },
    )
}

/// The Lucide `skip-back` icon.
#[component]
pub fn SkipBack(props: IconProps) -> Element {
    icon_shell(
        "skip-back",
        &[],
        &props,
        rsx! {
            path { d: "M17.971 4.285A2 2 0 0 1 21 6v12a2 2 0 0 1-3.029 1.715l-9.997-5.998a2 2 0 0 1-.003-3.432z" }
            path { d: "M3 20V4" }
        },
    )
}

/// The Lucide `skip-forward` icon.
#[component]
pub fn SkipForward(props: IconProps) -> Element {
    icon_shell(
        "skip-forward",
        &[],
        &props,
        rsx! {
            path { d: "M21 4v16" }
            path { d: "M6.029 4.285A2 2 0 0 0 3 6v12a2 2 0 0 0 3.029 1.715l9.997-5.998a2 2 0 0 0 .003-3.432z" }
        },
    )
}

/// The Lucide `square-play` icon (`lucide-play-square` is its alias class).
#[component]
pub fn SquarePlay(props: IconProps) -> Element {
    icon_shell(
        "square-play",
        &["play-square"],
        &props,
        rsx! {
            rect { x: "3", y: "3", width: "18", height: "18", rx: "2" }
            path { d: "M9 9.003a1 1 0 0 1 1.517-.859l4.997 2.997a1 1 0 0 1 0 1.718l-4.997 2.997A1 1 0 0 1 9 14.996z" }
        },
    )
}

/// The Lucide `x` icon.
#[component]
pub fn X(props: IconProps) -> Element {
    icon_shell(
        "x",
        &[],
        &props,
        rsx! {
            path { d: "M18 6 6 18" }
            path { d: "m6 6 12 12" }
        },
    )
}

/// The Lucide `zap` icon.
#[component]
pub fn Zap(props: IconProps) -> Element {
    icon_shell(
        "zap",
        &[],
        &props,
        rsx! {
            path { d: "M15.914 4a1.5 1.5 0 00-2.474-1.561l-9 9A1.5 1.5 0 005.5 14h4.002a.5.5 0 01.471.666L8.086 20a1.5 1.5 0 002.475 1.56l9-9A1.5 1.5 0 0018.5 10h-3.997a.5.5 0 01-.472-.667z" }
        },
    )
}

pub mod toast;

#[cfg(test)]
mod tests;
