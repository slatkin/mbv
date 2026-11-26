//! The Library Hero's artwork arm (gui-hero-design-polish design D2, the
//! rework of `LibraryHeroArt`): every art paints inside an inset frame — the
//! artboard's dark bezel with its rounded corners — and a landscape box takes
//! the larger share of the row beside the text column, dropping above the
//! text when the Hero is too narrow (the detail block's wrap rule, kept).
//! Square and portrait keep their 300 px box.
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
/// a 300 px side. A landscape arm ignores it, painting its flex share
/// instead.
const ART_HEIGHT: [f32; 3] = [
    169.0, // landscape
    300.0, // square
    450.0, // portrait
];

/// The tallest a landscape still paints, in CSS pixels: its box hugs the
/// 16:9 aspect up to this height, so a wide window never grows the still
/// without bound.
const BAND_MAX_HEIGHT: f32 = 360.0;

/// The landscape box's flex basis, in CSS pixels: the larger share of the row
/// beside the text column's 320 px basis (the artboard's proportion). The
/// grow of `1.6` over the text column's `1` keeps the share once both fit.
const BAND_FLEX_BASIS_PX: f32 = 380.0;

/// The inset frame's look (design D2): the artboard's dark bezel around the
/// art, with the art's own corners rounded inside it.
const FRAME_STYLE: &str = "padding: 2px; background: var(--canvas-art-frame); border-radius: 10px; box-sizing: border-box; position: relative;";

/// The box's text while no art is carried.
const DEFAULT_PLACEHOLDER: &str = "[No art · the window's placeholder]";

/// The props of the art box (the `LibraryHeroArt` props, in order).
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

/// The shape's wire name, as the former union spells it.
fn shape_wire(shape: GuiArtShapeFrame) -> &'static str {
    match shape {
        GuiArtShapeFrame::Landscape => "landscape",
        GuiArtShapeFrame::Square => "square",
        GuiArtShapeFrame::Portrait => "portrait",
    }
}

/// The art reference's kind, as the former client `art?.kind` reads it.
fn art_kind(art: Option<&GuiArtRef>) -> &'static str {
    match art {
        Some(GuiArtRef::Image { .. }) => "image",
        Some(GuiArtRef::Placeholder { .. }) => "placeholder",
        Some(GuiArtRef::Loading) => "loading",
        None => "none",
    }
}

/// Whether the arm is the landscape box beside the text column.
fn band(shape: GuiArtShapeFrame) -> bool {
    shape == GuiArtShapeFrame::Landscape
}

/// Whether a loaded image sizes its own box: a portrait or square box hugs
/// the image (never above its natural size) instead of holding the fixed art
/// box around a smaller, centered picture. The landscape box always fills its
/// flex share (design D2).
fn hugs(art: Option<&GuiArtRef>, shape: GuiArtShapeFrame) -> bool {
    !band(shape) && art.is_some_and(|art| *art != GuiArtRef::Loading)
}

/// The arm's index into [`ART_HEIGHT`]: the aspect the shape reserves.
fn height_index(shape: GuiArtShapeFrame) -> usize {
    match shape {
        GuiArtShapeFrame::Landscape => 0,
        GuiArtShapeFrame::Square => 1,
        GuiArtShapeFrame::Portrait => 2,
    }
}

/// The art box's own hooks: the art kind paints
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

/// The Library Hero's artwork box: the inset frame's sizing per arm and the art over the placeholder fill.
#[component]
pub fn LibraryHeroArt(props: LibraryHeroArtProps) -> Element {
    let band = band(props.shape);
    let hugs = hugs(props.art.as_ref(), props.shape);
    let art_height = ART_HEIGHT[height_index(props.shape)];

    // The root's placement in the detail block's wrapped row (design D2): the
    // landscape box takes the larger share beside the text column; the square
    // and portrait boxes keep their own width.
    let root_style = if band {
        format!("position: relative; flex: 1.6 1 {BAND_FLEX_BASIS_PX}px; min-width: 0;")
    } else {
        "position: relative; flex: none;".to_owned()
    };

    // The inset frame's width (design D2): the landscape frame fills its
    // flex share; the square and portrait frames wrap their box.
    let mut frame = FRAME_STYLE.to_owned();
    if band {
        let _ = write!(frame, " width: 100%;");
    } else if hugs {
        let _ = write!(frame, " width: fit-content; max-width: 100%;");
    }

    // The art area's own sizing inside the frame: the landscape area holds
    // the 16:9 aspect up to the band's height cap; a square or portrait area
    // keeps its fixed box, or hugs the image.
    let mut area = "overflow: hidden; border-radius: 8px;".to_owned();
    if band {
        let _ = write!(
            area,
            " width: 100%; aspect-ratio: 16 / 9; max-height: {BAND_MAX_HEIGHT}px; box-sizing: border-box;"
        );
    } else if hugs {
        let _ = write!(area, " width: fit-content; max-width: 100%;");
    } else {
        let _ = write!(area, " width: {ART_WIDTH}px; height: {art_height}px;");
    }
    let _ = write!(
        area,
        " background: {}; display: flex; align-items: center; justify-content: center;",
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
            style: "{root_style}",
            ..attrs,
            div {
                style: "{frame}",
                div {
                    class: "flex items-center justify-center",
                    style: "{area}",
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
    }
}

#[cfg(test)]
mod tests;
