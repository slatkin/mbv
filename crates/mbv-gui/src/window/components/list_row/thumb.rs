//! The row art thumb (`gui-unified-list-rows` design D1, D2; gui-delineated-rows
//! design D1, D2): the 72 × 40 box on the recessed fill, the image or the media
//! kind's icon inside it, and the row's resume run along the box's bottom edge.
//! Both row parts paint one thumb, so a change to the box lands in every list.
//!
//! A destination passes the thumb's art state and media kind; it never restates
//! the box. The box carries `data-library-<attr>-thumb` with its art state, and
//! its resume run carries `data-library-<attr>-thumb-progress`.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::{GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_attr::library_attr;
use crate::window::components::list_row::{
    THUMB_HEIGHT_PX, THUMB_ICON_PX, THUMB_PROGRESS_PX, THUMB_RADIUS_PX, THUMB_SQUARE_PX,
    THUMB_WIDTH_PX,
};
use crate::window::icons::{EighthNotes, MonitorPlay};
use crate::window::theme::canvas_var;

/// The art one row's thumb paints (gui-delineated-rows design D1): the frame's
/// art state and the media kind its icon names while it holds no image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRowThumb {
    /// The thumb's art state.
    pub art: GuiRowArtFrame,
    /// The media kind the icon shows in the missing and no-source states.
    pub media: GuiRowMediaKindFrame,
}

/// One row's art thumb (gui-delineated-rows design D1, D2): the 72 × 40 box on
/// the recessed fill, the image or the media kind's icon inside it, and the
/// row's resume run along the box's bottom edge.
pub fn paint_thumb(attr: &'static str, thumb: &LibraryRowThumb, progress: Option<f32>) -> Element {
    // The box: one fixed landscape shape every row shares. A whole (square or
    // portrait) image sits on no fill: the box only centres it.
    let whole = matches!(thumb.art, GuiRowArtFrame::Image { square: true, .. });
    let box_fill = if whole {
        "transparent".to_owned()
    } else {
        canvas_var("well")
    };
    let box_style = format!(
        "width: {THUMB_WIDTH_PX}px; height: {THUMB_HEIGHT_PX}px; border-radius: {THUMB_RADIUS_PX}px; background: {box_fill};"
    );
    let box_attrs = vec![Attribute::new(
        library_attr(attr, "thumb"),
        thumb_state(&thumb.art),
        None,
        false,
    )];
    let cover_class = "h-full w-full object-cover";
    let square_style = format!(
        "height: {THUMB_SQUARE_PX}px; width: auto; max-width: {THUMB_WIDTH_PX}px; border-radius: {THUMB_RADIUS_PX}px;"
    );
    let body: Element = match &thumb.art {
        GuiRowArtFrame::Image { art, square: true } => {
            thumb_image(art.clone(), "object-contain", &square_style)
        }
        GuiRowArtFrame::Image { art, square: false } => thumb_image(art.clone(), cover_class, ""),
        // A fetch in flight paints the well alone, as the loading fill does.
        GuiRowArtFrame::Loading => rsx! {},
        GuiRowArtFrame::Missing | GuiRowArtFrame::NoSource => thumb_icon(thumb.media),
    };
    let progress_attrs = vec![Attribute::new(
        library_attr(attr, "thumb-progress"),
        "",
        None,
        false,
    )];
    rsx! {
        span {
            class: "relative flex flex-none items-center justify-center overflow-hidden",
            style: "{box_style}",
            ..box_attrs,
            {body}
            if let Some(progress) = progress {
                // The row's resume position: the track along the box's bottom
                // edge and the accent fill over it, at the frame's percent.
                span {
                    class: "absolute",
                    style: "left: 0; bottom: 0; width: 100%; height: {THUMB_PROGRESS_PX}px; background: var(--canvas-thumb-track);",
                }
                span {
                    class: "absolute",
                    style: "left: 0; bottom: 0; height: {THUMB_PROGRESS_PX}px; width: {progress}%; background: var(--canvas-partial);",
                    ..progress_attrs,
                }
            }
        }
    }
}

/// The art state word a row's thumb hook carries (gui-delineated-rows design
/// D1): the state the destination's `data-library-<attr>-thumb` attribute
/// paints.
fn thumb_state(art: &GuiRowArtFrame) -> &'static str {
    match art {
        GuiRowArtFrame::Image { .. } => "image",
        GuiRowArtFrame::Loading => "loading",
        GuiRowArtFrame::Missing => "missing",
        GuiRowArtFrame::NoSource => "none",
    }
}

/// The media kind icon a thumb with no image paints (gui-delineated-rows design
/// D2): the audio notes or the video screen, in the muted tone.
fn thumb_icon(media: GuiRowMediaKindFrame) -> Element {
    let color = canvas_var("muted");
    match media {
        GuiRowMediaKindFrame::Audio => rsx! {
            EighthNotes { size: THUMB_ICON_PX, color }
        },
        GuiRowMediaKindFrame::Video => rsx! {
            MonitorPlay { size: THUMB_ICON_PX, color }
        },
    }
}

/// The image an image-state thumb paints (gui-delineated-rows design D2): a
/// landscape image cover-fills the box, and a square or portrait image shows
/// whole at the box's height, centred, on no fill, with rounded corners.
fn thumb_image(art: GuiArtRef, class: &str, style: &str) -> Element {
    rsx! {
        ArtImage { art: Some(art), alt: "", class: class.to_owned(), style: style.to_owned() }
    }
}
