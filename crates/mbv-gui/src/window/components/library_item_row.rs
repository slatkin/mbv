//! The Library's top-level row part (`replace-tauri-with-blitz` task 7.2,
//! the port of `LibraryItemRow`; add-gui-library-panel design D15,
//! invariant 30; gui-delineated-rows design D1, D2): the art thumb, the title
//! over its subtitle, the date or duration at the right, and the selection
//! fill. Every destination paints its top-level rows here, so a fix to one row
//! look lands in every list (the gui-client spec's "Library lists and
//! Workspace tables paint through shared parts"). A row keeps no keyboard
//! cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2, D6).
//!
//! A destination supplies its own hook prefix (`attr`), the row's text, its
//! thumb, and the press, double-click, and context handlers; it never restates
//! the look. Every part paints `data-library-<attr>-<part>`, and the thumb adds
//! `data-library-<attr>-thumb` with its art state and
//! `data-library-<attr>-thumb-progress`.
//!
//! [`paint_item_row`] paints one row of a flat entry list — Podcasts and Feeds
//! — from its carried entry, so the two lists keep one row painter.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::{
    GuiEntryRowFrame, GuiHomeRowStateFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_attr::library_attr;
use crate::window::icons::{EighthNotes, MonitorPlay};
use crate::window::input::f32_from_f64;
use crate::window::library::shows_workspace::episode_progress;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The top-level row's height in logical pixels (gui-delineated-rows design
/// D2): every destination's row constant reads it, so a list's geometry and
/// its painted rows share one height.
pub const LIBRARY_ROW_HEIGHT: f32 = 56.0;

/// The top-level row's title weight (design D2): 400 at every selection state,
/// as the canvas paints it.
pub const FLAT_ROW_TITLE_WEIGHT: f32 = 400.0;

/// The thumb box's width in logical pixels (design D2).
pub const THUMB_WIDTH_PX: f32 = 64.0;

/// The thumb box's height in logical pixels (design D2).
pub const THUMB_HEIGHT_PX: f32 = 36.0;

/// The side of a square image inside the thumb box (design D2): the box's
/// height, centred, so the image shows whole.
pub const THUMB_SQUARE_PX: f32 = THUMB_HEIGHT_PX;

/// The thumb box's corner radius (design D2).
pub const THUMB_RADIUS_PX: f32 = 3.0;

/// The media kind icon's size inside a thumb with no image (design D2).
pub const THUMB_ICON_PX: u16 = 16;

/// The thumb's progress track and fill height, along the box's bottom edge
/// (design D2).
pub const THUMB_PROGRESS_PX: f32 = 3.0;

/// The art one top-level row's thumb paints (gui-delineated-rows design D1):
/// the frame's art state and the media kind its icon names while it holds no
/// image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRowThumb {
    /// The thumb's art state.
    pub art: GuiRowArtFrame,
    /// The media kind the icon shows in the missing and no-source states.
    pub media: GuiRowMediaKindFrame,
}

/// The trailing text's hook part: the date gutter or the duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTrailPart {
    /// The date gutter.
    Date,
    /// The runtime.
    Duration,
}

impl LibraryTrailPart {
    /// The part name the row's hook paints (`data-library-<attr>-<part>`).
    #[must_use]
    pub fn part(self) -> &'static str {
        match self {
            LibraryTrailPart::Date => "date",
            LibraryTrailPart::Duration => "duration",
        }
    }
}

/// The props of the item row (the `LibraryItemRow` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryItemRowProps {
    /// The destination's hook prefix, as in `podcasts`, `feeds`, `emby`,
    /// `abs`.
    pub attr: &'static str,
    /// The row's own kind value, as in `episode`, `entry`, `book`.
    pub kind: String,
    /// The row's stable id.
    pub id: String,
    /// The row's painted height, in pixels.
    pub height: f32,
    /// The row's primary label.
    pub title: String,
    /// The row's secondary label, `None` while the row paints one line.
    pub subtitle: Option<String>,
    /// The trailing text, `None` while the row paints no gutter value.
    pub trail: Option<String>,
    /// The trailing text's hook part.
    pub trail_part: LibraryTrailPart,
    /// The title's colour: the text tone, or the muted tone on a played row.
    pub title_color: String,
    /// The row's painted fill, when the selection marks it.
    pub background: Option<String>,
    /// Whether the selection marks the row.
    pub selected: bool,
    /// The resume percent of a partly played row, or `None`. It paints along
    /// the thumb's bottom edge (design D2); a row with no thumb paints none.
    pub progress: Option<f32>,
    /// The row's art thumb (design D1), `None` while the destination paints no
    /// thumb yet.
    #[props(default)]
    pub thumb: Option<LibraryRowThumb>,
    /// A press on the row.
    pub on_press: Callback<()>,
    /// A double-click on the row: the play path.
    pub on_play: Callback<()>,
    /// A right-press on the row, with the pointer's client position; `None`
    /// while the row names no item a menu could open.
    #[props(default)]
    pub on_context: Option<Callback<(f32, f32)>>,
}

/// The flat list's item row:
/// the destination's hooks paint from its prefix, so a destination's tests
/// keep their per-row parts beside the shared look.
#[component]
pub fn LibraryItemRow(props: LibraryItemRowProps) -> Element {
    // The row's own hooks: the kind and the id
    // always paint, the selection mark only while it holds.
    let mut row_attrs = vec![
        Attribute::new(
            library_attr(props.attr, "row"),
            props.kind.clone(),
            None,
            false,
        ),
        Attribute::new(
            library_attr(props.attr, "row-id"),
            props.id.clone(),
            None,
            false,
        ),
    ];
    if props.selected {
        row_attrs.push(Attribute::new(
            library_attr(props.attr, "selected"),
            "",
            None,
            false,
        ));
    }
    // The top-level row look (design D2): the fixed height, the tight padding,
    // the square corners, and the 1 px divider under the row. `border-box`
    // keeps the divider inside the row height the list's geometry holds.
    let mut row_style = format!(
        "height: {}px; box-sizing: border-box; padding: 0 8px; gap: 12px; border: 0; border-bottom: 1px solid {}; border-radius: 0; text-align: left; font-size: 14px; cursor: pointer;",
        props.height,
        canvas_var("border")
    );
    if let Some(background) = props.background.as_ref() {
        let _ = write!(row_style, " background: {background};");
    }
    let title_attrs = vec![Attribute::new(
        library_attr(props.attr, "title"),
        "",
        None,
        false,
    )];
    let subtitle_attrs = vec![Attribute::new(
        library_attr(props.attr, "subtitle"),
        "",
        None,
        false,
    )];
    let trail_attrs = props.trail.as_ref().map_or(Vec::new(), |trail| {
        vec![Attribute::new(
            library_attr(props.attr, props.trail_part.part()),
            trail.clone(),
            None,
            false,
        )]
    });
    let thumb = props
        .thumb
        .as_ref()
        .map(|thumb| paint_thumb(props.attr, thumb, props.progress));
    let on_press = props.on_press;
    let on_play = props.on_play;
    let on_context = props.on_context;

    rsx! {
        button {
            "type": "button",
            class: "relative flex w-full items-center overflow-hidden",
            style: "{row_style}",
            onclick: move |_| on_press.call(()),
            ondoubleclick: move |_| on_play.call(()),
            oncontextmenu: move |event: MouseEvent| {
                if let Some(on_context) = on_context {
                    event.prevent_default();
                    let coordinates = event.client_coordinates();
                    on_context
                        .call((f32_from_f64(coordinates.x), f32_from_f64(coordinates.y)));
                }
            },
            ..row_attrs,
            if let Some(thumb) = thumb {
                {thumb}
            }
            span {
                class: "flex min-w-0 flex-1 flex-col",
                style: "gap: 2px;",
                Truncate {
                    text: props.title.clone(),
                    css: format!(
                        "color: {}; font-weight: {FLAT_ROW_TITLE_WEIGHT};",
                        props.title_color
                    ),
                    style: TextStyle::sans(14.0, FLAT_ROW_TITLE_WEIGHT),
                    attrs: title_attrs,
                }
                if let Some(subtitle) = props.subtitle.clone() {
                    Truncate {
                        text: subtitle,
                        css: "font-size: 12px; color: var(--canvas-muted);".to_owned(),
                        style: TextStyle::sans(12.0, 400.0),
                        attrs: subtitle_attrs,
                    }
                }
            }
            if let Some(trail) = props.trail.clone().filter(|trail| !trail.is_empty()) {
                span {
                    class: "flex-none font-mono tabular-nums",
                    style: "font-size: 12px; color: var(--canvas-muted);",
                    ..trail_attrs,
                    "{trail}"
                }
            }
        }
    }
}

/// The art state word a row's thumb hook carries (design D1): the state the
/// destination's `data-library-<attr>-thumb` attribute paints.
fn thumb_state(art: &GuiRowArtFrame) -> &'static str {
    match art {
        GuiRowArtFrame::Image { .. } => "image",
        GuiRowArtFrame::Loading => "loading",
        GuiRowArtFrame::Missing => "missing",
        GuiRowArtFrame::NoSource => "none",
    }
}

/// The media kind icon a thumb with no image paints (design D2): the audio
/// notes or the video screen, in the muted tone.
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

/// The image an image-state thumb paints (design D2): a landscape or portrait
/// image cover-fills the box, and a square image shows whole at the box's
/// height, centred.
fn thumb_image(art: GuiArtRef, class: &str, style: &str) -> Element {
    rsx! {
        ArtImage { art: Some(art), alt: "", class: class.to_owned(), style: style.to_owned() }
    }
}

/// One row's art thumb (design D1, D2): the 64 × 36 box on the recessed fill,
/// the image or the media kind's icon inside it, and the row's resume run along
/// the box's bottom edge.
fn paint_thumb(attr: &'static str, thumb: &LibraryRowThumb, progress: Option<f32>) -> Element {
    // The box: one fixed landscape shape every top-level row shares.
    let box_style = format!(
        "width: {THUMB_WIDTH_PX}px; height: {THUMB_HEIGHT_PX}px; border-radius: {THUMB_RADIUS_PX}px; background: {};",
        canvas_var("well")
    );
    let box_attrs = vec![Attribute::new(
        library_attr(attr, "thumb"),
        thumb_state(&thumb.art),
        None,
        false,
    )];
    let cover_class = "h-full w-full object-cover";
    let square_style = format!("width: {THUMB_SQUARE_PX}px; height: {THUMB_SQUARE_PX}px;");
    let body: Element = match &thumb.art {
        GuiRowArtFrame::Image { art, square: true } => {
            thumb_image(art.clone(), "object-cover", &square_style)
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

/// One flat entry row's painted facts: the destination's hook prefix and kind,
/// the row's height, the carried entry, and whether the selection marks the
/// row.
#[derive(Debug, Clone, Copy)]
pub struct ItemRowPaint<'a> {
    /// The destination's hook prefix, as in `podcasts` or `feeds`.
    pub attr: &'static str,
    /// The row's own kind value, as in `episode` or `entry`.
    pub kind: &'static str,
    /// The row's painted height, in pixels.
    pub height: f32,
    /// The row's stable id.
    pub id: &'a str,
    /// The carried entry the row paints.
    pub entry: &'a GuiEntryRowFrame,
    /// Whether the selection marks the row.
    pub selected: bool,
}

/// Paint one flat entry row (Podcasts, Feeds): the entry's text at the row's
/// height with the selection fill, and the destination's press, play, and
/// context handlers. A destination that names no right-press menu passes no
/// context handler.
pub fn paint_item_row(
    paint: ItemRowPaint<'_>,
    on_press: Callback<()>,
    on_play: Callback<()>,
    on_context: Option<Callback<(f32, f32)>>,
) -> Element {
    let background = paint.selected.then(|| canvas_var("library-raise"));
    rsx! {
        LibraryItemRow {
            attr: paint.attr,
            kind: paint.kind.to_owned(),
            id: paint.id.to_owned(),
            height: paint.height,
            title: paint.entry.title.clone(),
            subtitle: paint.entry.subtitle.clone(),
            trail: paint.entry.date.clone(),
            trail_part: LibraryTrailPart::Date,
            title_color: name_color(&paint.entry.state),
            background,
            selected: paint.selected,
            progress: episode_progress(&paint.entry.state).map(f32::from),
            thumb: Some(LibraryRowThumb {
                art: paint.entry.art.clone(),
                media: paint.entry.media,
            }),
            on_press,
            on_play,
            on_context,
        }
    }
}

/// An item row's title colour: the canvas's text tone, dimmed on a played row
/// (design D12).
#[must_use]
pub fn name_color(state: &GuiHomeRowStateFrame) -> String {
    if *state == GuiHomeRowStateFrame::Played {
        canvas_var("muted")
    } else {
        canvas_var("text")
    }
}

/// The mark a row paints only while it holds the state (the former client
/// `undefined` values painted nothing).
#[must_use]
pub fn mark(name: &'static str, on: bool) -> Vec<Attribute> {
    on.then(|| Attribute::new(name, "", None, false))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests;
