//! The Library Hero's artwork arm (`replace-tauri-with-blitz` task 7.5, the
//! port of `LibraryHeroArt.svelte`; add-gui-library-panel design D15, D14):
//! the canvas's 300 px box for a portrait or square cover, and the TUI's
//! layout for a landscape still — the artwork fills the Hero content width at
//! 16:9, because a 16:9 still shrunk into a square box is not the TUI's hero.
//! Every destination's Hero paints its art through this one component, so the
//! two arms cannot drift between tabs.
//!
//! `attr` names the destination's own attributes (`data-library-shows-hero-art`),
//! so a destination's tests keep their hook.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::GuiArtShapeFrame;
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_hero_frame::own;
use crate::window::natural_size::NaturalSizeBounds;
use crate::window::theme::canvas_var;

/// The canvas's box width, in CSS pixels: the Music Hero's 300 px side,
/// which the portrait and square arms keep.
const ART_WIDTH: f32 = 300.0;

/// The box height per arm, in CSS pixels: the aspect the artwork shape gives
/// a 300 px side. A landscape arm ignores it, painting the content width
/// instead.
const ART_HEIGHT: [f32; 3] = [
    169.0, // landscape
    300.0, // square
    450.0, // portrait
];

/// The tallest a landscape still paints, in CSS pixels: its box hugs the
/// image up to the content width, so a wide window never grows the still
/// without bound.
const BAND_MAX_HEIGHT: f32 = 360.0;

/// The box's text while no art is carried (the Svelte prop's default).
const DEFAULT_PLACEHOLDER: &str = "[No art · the window's placeholder]";

/// The props of the art box (the `LibraryHeroArt.svelte` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHeroArtProps {
    /// The artwork box the Hero reserves, from the artwork policy.
    pub shape: GuiArtShapeFrame,
    /// The fetched artwork, or `None` while the item declares none.
    pub art: Option<GuiArtRef>,
    /// The item's title: the image's alternative text.
    pub title: String,
    /// The destination's own attribute prefix, as in `shows` or `home`;
    /// empty for Music, whose attributes are `data-library-hero-*`.
    pub attr: &'static str,
    /// The box's text while no art is carried.
    #[props(default)]
    pub placeholder: Option<String>,
}

/// The shape's wire name, as the Svelte union spells it.
fn shape_wire(shape: GuiArtShapeFrame) -> &'static str {
    match shape {
        GuiArtShapeFrame::Landscape => "landscape",
        GuiArtShapeFrame::Square => "square",
        GuiArtShapeFrame::Portrait => "portrait",
    }
}

/// The art reference's kind, as the Svelte `art?.kind` reads it.
fn art_kind(art: Option<&GuiArtRef>) -> &'static str {
    match art {
        Some(GuiArtRef::Image { .. }) => "image",
        Some(GuiArtRef::Placeholder { .. }) => "placeholder",
        Some(GuiArtRef::Loading) => "loading",
        None => "none",
    }
}

/// Whether the arm paints the TUI's full-width still.
fn band(shape: GuiArtShapeFrame) -> bool {
    shape == GuiArtShapeFrame::Landscape
}

/// Whether a loaded image sizes its own box: a portrait or square box hugs
/// the image (never above its natural size) instead of holding the fixed art
/// box around a smaller, centered picture.
fn hugs(art: Option<&GuiArtRef>) -> bool {
    art.is_some_and(|art| *art != GuiArtRef::Loading)
}

/// The arm's index into [`ART_HEIGHT`]: the aspect the shape reserves.
fn height_index(shape: GuiArtShapeFrame) -> usize {
    match shape {
        GuiArtShapeFrame::Landscape => 0,
        GuiArtShapeFrame::Square => 1,
        GuiArtShapeFrame::Portrait => 2,
    }
}

/// The art box's own hooks (the Svelte `own` helper): the art kind paints
/// always, the shape with it.
fn box_attrs(
    attr: &'static str,
    art: Option<&GuiArtRef>,
    shape: GuiArtShapeFrame,
) -> Vec<Attribute> {
    let kind = art_kind(art);
    vec![
        Attribute::new(own(attr, "art"), kind, None, false),
        Attribute::new(own(attr, "art-shape"), shape_wire(shape), None, false),
    ]
}

/// The Library Hero's artwork box (the Svelte markup, character for
/// character): the box's sizing per arm, and the art over the placeholder
/// fill.
#[component]
pub fn LibraryHeroArt(props: LibraryHeroArtProps) -> Element {
    let band = band(props.shape);
    let hugs = hugs(props.art.as_ref());
    let art_height = ART_HEIGHT[height_index(props.shape)];

    let width = if hugs {
        "fit-content".to_owned()
    } else if band {
        "100%".to_owned()
    } else {
        format!("{ART_WIDTH}px")
    };
    let mut style = format!("width: {width};");
    if hugs {
        let max_width = if band { "100%" } else { width.as_str() };
        let _ = write!(style, " max-width: {max_width};");
    }
    if band {
        let _ = write!(style, " height: auto;");
    } else if !hugs {
        let _ = write!(style, " height: {art_height}px;");
    }
    if band && !hugs {
        let _ = write!(style, " aspect-ratio: 16 / 9;");
    }
    let _ = write!(
        style,
        " background: {}; border-radius: 6px; overflow: hidden;",
        canvas_var("library-raise")
    );

    let bounds = if band {
        NaturalSizeBounds {
            width: None,
            height: Some(BAND_MAX_HEIGHT),
        }
    } else {
        NaturalSizeBounds {
            width: Some(ART_WIDTH),
            height: Some(art_height),
        }
    };
    let placeholder = props
        .placeholder
        .unwrap_or_else(|| DEFAULT_PLACEHOLDER.to_owned());
    let muted = canvas_var("muted");
    let attrs = box_attrs(props.attr, props.art.as_ref(), props.shape);

    rsx! {
        div {
            class: "flex flex-none items-center justify-center",
            style: "{style}",
            ..attrs,
            ArtImage {
                art: props.art.clone(),
                alt: props.title.clone(),
                class: "block".to_owned(),
                bounds: Some(bounds),
                fallback: Some(rsx! {
                    span {
                        class: "font-mono",
                        style: "font-size: 13px; color: {muted};",
                        "{placeholder}"
                    }
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests;
