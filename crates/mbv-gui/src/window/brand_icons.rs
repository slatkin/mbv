//! The brand mark beside a playback-target chooser row (the port of
//! `TargetBrandIcon`). Emby and Google Cast paint their brand hex from
//! `window/service_mark.rs`, the one brand-mark table in `window/` (issue
//! #1043), so the Library header's Emby mark and this row read one copy.
//! The mpv mark stands for a Player owner rather than a third-party brand,
//! so its path stays here and it takes the theme's remote-host role instead
//! of mpv's purple. The local output keeps its Lucide `Monitor` in the
//! chooser.
//!
//! The colour goes on the svg's computed `color` and the path fills with
//! `currentColor`, because usvg cannot resolve a `var()` in a `fill`
//! attribute (`window/icons.rs` holds the reason).

use dioxus_native::prelude::*;

use crate::bridge::frame::theme::GuiRole;
use crate::window::service_mark::{CAST_COLOR, CAST_PATH, EMBY_COLOR, EMBY_PATH};
use crate::window::theme::role_var;

/// The target kind a chooser row's mark names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBrand {
    /// A plain Emby Session.
    Emby,
    /// A Google Cast receiver.
    Cast,
    /// A peer device that owns a Player with a direct endpoint: an `mbvd`, or
    /// another mbv.
    DirectPeer,
}

impl TargetBrand {
    /// The `data-brand` value, as the former component wrote it.
    fn name(self) -> &'static str {
        match self {
            Self::Emby => "emby",
            Self::Cast => "cast",
            Self::DirectPeer => "mbvd",
        }
    }

    /// The colour the mark paints in.
    fn color(self) -> String {
        match self {
            Self::Emby => EMBY_COLOR.to_owned(),
            Self::Cast => CAST_COLOR.to_owned(),
            Self::DirectPeer => role_var(GuiRole::PlaybackHostRemoteFg),
        }
    }

    /// The simple-icons path, on a 24 × 24 view box.
    fn path(self) -> &'static str {
        match self {
            Self::Emby => EMBY_PATH,
            Self::Cast => CAST_PATH,
            Self::DirectPeer => {
                "M12 0C5.373 0 0 5.373 0 12s5.373 12 12 12 12-5.373 12-12S18.627 0 12 0zm.312 22.775c-6.153 0-11.142-4.988-11.142-11.142S6.16.491 12.312.491c6.154 0 11.142 4.989 11.142 11.142s-4.988 11.142-11.142 11.142zm.643-20.464a8.587 8.587 0 1 0 0 17.174 8.587 8.587 0 0 0 0-17.174zm-1.113 15.257a5.517 5.517 0 1 1 0-11.034 5.517 5.517 0 0 1 0 11.034zm-1.399-7.995L14.4 11.97l-3.957 2.518V9.573z"
            }
        }
    }
}

/// The brand mark of `brand`, `size` CSS pixels square.
#[component]
pub fn TargetBrandIcon(brand: TargetBrand, #[props(default = 14)] size: u16) -> Element {
    let color = brand.color();
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            width: "{size}",
            height: "{size}",
            fill: "currentColor",
            style: "color: {color};",
            "aria-hidden": "true",
            "data-brand": brand.name(),
            path { d: brand.path() }
        }
    }
}
