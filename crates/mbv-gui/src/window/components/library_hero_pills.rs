//! The Hero's fact pills (gui-hero-design-polish design D2): one pill per
//! `GuiHeroPillFrame`, in paint order. A pill paints its typed segments
//! through `TypedText`, so the value's text and type colour stay as today; a
//! link pill opens its URL through the core's open-url command, as the former
//! `IMDb` header action did — the core scheme-gates the url.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHeroPillFrame;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_hero_frame::own;
use crate::window::components::typed_text::TypedText;
use crate::window::library::state::LibraryState;

/// One pill's look (the artboard's rounded fact pill): the raised panel fill
/// over the pane, one control border, and the fully round end caps. The pill
/// stays compact (user request 2026-11-26): a small mono face and tight
/// padding, so the facts sit under the title as a fine row rather than as a
/// second heading.
const PILL_STYLE: &str = "padding: 2px 8px; background: var(--canvas-library-panel); border: 1px solid var(--canvas-library-control); border-radius: 999px; font-size: 11px; line-height: 1.35; letter-spacing: 0.02em; color: var(--canvas-heading);";

/// The look a link pill adds: the date hue of a link and the dotted underline
/// the former `IMDb` action painted.
const LINK_PILL_STYLE: &str =
    "cursor: pointer; color: var(--canvas-source); text-decoration: underline dotted;
     text-decoration-thickness: 1px; text-underline-offset: 3px;";

/// The props of the pills row (the `LibraryHeroPills` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHeroPillsProps {
    /// The pills, in paint order.
    pub pills: Vec<GuiHeroPillFrame>,
    /// The destination's attribute prefix: empty for Music, as in `shows` or
    /// `emby` for the rest.
    pub attr: &'static str,
    /// The row's own attribute part, as in `pills` or `block-pills`.
    pub hook: &'static str,
}

/// The Hero's fact pills row: one pill per fact, the link pills clickable.
#[component]
pub fn LibraryHeroPills(props: LibraryHeroPillsProps) -> Element {
    let library = consume_context::<LibraryState>();
    rsx! {
        div {
            class: "flex flex-wrap items-center",
            // The row's own left edge pulls back by the pill's side padding and
            // its border, so the first pill's text lines up with the title
            // above it (user request 2026-11-26).
            style: "gap: 6px; margin-left: -9px;",
            ..vec![Attribute::new(own(props.attr, props.hook), "", None, false)],
            for (index, pill) in props.pills.iter().enumerate() {
                {
                    let url = pill.link.clone();
                    let link_library = library.clone();
                    rsx! {
                        if let Some(url) = url {
                            button {
                                key: "{index}",
                                "type": "button",
                                class: "font-mono",
                                style: "{PILL_STYLE} {LINK_PILL_STYLE}",
                                onclick: move |_| {
                                    link_library.link.library(GuiLibraryCommand::OpenUrl {
                                        url: url.clone(),
                                    });
                                },
                                ..vec![Attribute::new(own(props.attr, "pill"), "link", None, false)],
                                TypedText { segments: pill.segments.clone() }
                            }
                        } else {
                            span {
                                key: "{index}",
                                class: "font-mono",
                                style: "{PILL_STYLE}",
                                ..vec![Attribute::new(own(props.attr, "pill"), "fact", None, false)],
                                TypedText { segments: pill.segments.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}
