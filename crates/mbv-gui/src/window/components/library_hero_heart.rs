//! The Hero's heart control (emby-favourites design D10): the one toggle in
//! the Hero header band's upper right that shows the item's favourite state
//! and flips it through the same write the menu entry runs. Every non-music
//! Emby Hero paints it when its item supports favourites, so the control has
//! one look and one press path across the plain, Shows, Favourites, and Home
//! Heroes. The filled state is the rose `#e57e80` over the Lucide heart's own
//! fill, and only the heart glyph turns the accent green on hover; the empty state is the muted outline. A press sends the
//! `SetFavourite` command with the state the item does not yet have, so the
//! write, the menu entry, and every held copy flip together.

use dioxus_native::prelude::*;

use crate::bridge::library::GuiLibraryCommand;
use crate::gui_core::GuiHeroHeart;
use crate::window::icons::Heart;
use crate::window::library::state::LibraryState;

/// The props of the heart control (the `LibraryHeroHeart` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHeroHeartProps {
    /// The heart's facts: the item the press names and the state the control
    /// shows.
    pub heart: GuiHeroHeart,
    /// The `data-*` attribute that marks the control for its tests.
    pub hook: &'static str,
}

/// The heart's size in CSS pixels: the header band's control is a step over
/// the title line's own buttons, so it reads as the band's one action.
const HEART_ICON_SIZE: u16 = 20;

/// The Hero heart: the favourite toggle in the header band's upper right.
#[component]
pub fn LibraryHeroHeart(props: LibraryHeroHeartProps) -> Element {
    let library = consume_context::<LibraryState>();
    let attrs = vec![Attribute::new(props.hook, "", None, false)];
    let item_id = props.heart.item_id.clone();
    let favourite = props.heart.favourite;
    // The svg paints from its own `color` attribute, which a CSS `:hover` on
    // the button does not re-resolve, so the hover state is a signal that
    // names the glyph's colour: rose when favourited, muted when not, accent
    // green under the pointer.
    let mut hovered = use_signal(|| false);
    let (label, fill) = if favourite {
        ("Remove from Favourites", "currentColor".to_owned())
    } else {
        ("Add to Favourites", "none".to_owned())
    };
    let color = if hovered() {
        "#a7c080"
    } else if favourite {
        "#e57e80"
    } else {
        "var(--canvas-muted)"
    };
    rsx! {
        button {
            "type": "button",
            class: "flex flex-none items-center justify-center",
            style: "width: 32px; height: 32px; padding: 0; --hover-veil: transparent; background: none; border: none; color: {color}; cursor: pointer;",
            "aria-label": label,
            "aria-pressed": if favourite { "true" } else { "false" },
            title: label,
            onpointerenter: move |_| hovered.set(true),
            onpointerleave: move |_| hovered.set(false),
            onclick: move |_| {
                library.link.library(GuiLibraryCommand::SetFavourite {
                    item_id: item_id.clone(),
                    favourite: !favourite,
                });
            },
            ..attrs,
            Heart {
                size: HEART_ICON_SIZE,
                fill,
                color: color.to_owned(),
            }
        }
    }
}
