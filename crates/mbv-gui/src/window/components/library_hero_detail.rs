//! The Library Hero's detail block: the one box that holds a Hero's artwork
//! and, beside it, its text column. Every destination's Hero paints its art
//! through this block — the Music, Shows, Emby, Audiobooks, Podcasts, and Feeds
//! Heroes through `LibraryHeroFrame`, and the Home Hero through
//! `LibraryHeroSummary` — so the art's container, and with it the art's size
//! and placement, cannot differ between destinations.
//!
//! The block wraps: the text sits beside the art when 320 px or more is free
//! and drops below it when the Hero is narrower, for every art shape.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::GuiArtShapeFrame;
use crate::window::components::library_hero_art::LibraryHeroArt;

/// The props of the detail block.
#[derive(Props, Clone, PartialEq)]
pub struct LibraryHeroDetailProps {
    /// The destination's attribute prefix: empty for Music, as in `shows` or
    /// `home` for the rest.
    pub attr: &'static str,
    /// The block's own attribute name, the hook the destination's tests take.
    pub block_attr: &'static str,
    /// The block's accessible label: the item's title.
    pub label: String,
    /// The artwork box's shape and its fetched art.
    pub art_shape: GuiArtShapeFrame,
    pub art: Option<GuiArtRef>,
    /// The alternative text of the artwork.
    pub art_title: String,
    /// The artwork box's placeholder while no art is carried.
    #[props(default)]
    pub art_placeholder: Option<String>,
    /// The text column's lines.
    pub children: Element,
}

impl std::fmt::Debug for LibraryHeroDetailProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryHeroDetailProps")
            .field("attr", &self.attr)
            .field("block_attr", &self.block_attr)
            .field("label", &self.label)
            .field("art_shape", &self.art_shape)
            .field("art", &self.art)
            .field("art_title", &self.art_title)
            .field("art_placeholder", &self.art_placeholder)
            .finish_non_exhaustive()
    }
}

/// The detail block: the artwork box and the text column beside it.
#[component]
pub fn LibraryHeroDetail(props: LibraryHeroDetailProps) -> Element {
    let LibraryHeroDetailProps {
        attr,
        block_attr,
        label,
        art_shape,
        art,
        art_title,
        art_placeholder,
        children,
    } = props;
    let band = art_shape == GuiArtShapeFrame::Landscape;
    let block_attrs = vec![Attribute::new(block_attr, "", None, false)];

    rsx! {
        section {
            // Every art shape wraps: the text drops under the art when the
            // Hero is too narrow to hold both side by side (the text's
            // 320 px basis), not only for a landscape still.
            class: "flex flex-wrap items-start",
            style: "gap: 18px 32px;",
            "aria-label": "{label}",
            ..block_attrs,
            LibraryHeroArt {
                shape: art_shape,
                art,
                title: art_title,
                attr,
                placeholder: art_placeholder,
            }
            div {
                class: "flex min-w-0 flex-1 flex-col",
                style: if band {
                    "gap: 18px; flex-basis: 320px; padding-top: 0;"
                } else {
                    "gap: 18px; flex-basis: 320px; padding-top: 8px;"
                },
                { children }
            }
        }
    }
}
