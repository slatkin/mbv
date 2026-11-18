//! The flat Library list's item row (`replace-tauri-with-blitz` task 7.2,
//! the port of `LibraryItemRow`; add-gui-library-panel design D15,
//! invariant 30): the title over its subtitle, the date or duration at the
//! right, the selection fill, and the resume run under the row. Every flat
//! destination list — Emby, Audiobooks, Podcasts, Feeds — paints its rows here,
//! so a fix to one row look lands in every list. A row keeps no keyboard
//! cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2, D6).
//!
//! A destination supplies its own hook prefix (`attr`), the row's text, and
//! the press, double-click, and context handlers; it never restates the
//! look. Every part paints `data-library-<attr>-<part>`.
//!
//! [`paint_item_row`] paints one row of a flat entry list — Podcasts and Feeds
//! — from its carried entry, so the two lists keep one row painter.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiEntryRowFrame, GuiHomeRowStateFrame};
use crate::window::components::library_attr::library_attr;
use crate::window::input::f32_from_f64;
use crate::window::library::shows_workspace::episode_progress;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

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
    /// The resume percent of a partly played row, or `None`.
    pub progress: Option<f32>,
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
    let mut row_style = format!(
        "height: {}px; padding: 0 12px; gap: 10px; border: 0; border-radius: 4px; text-align: left; font-size: 14.5px; cursor: pointer;",
        props.height
    );
    if let Some(background) = props.background.as_ref() {
        let _ = write!(row_style, " background: {background};");
    }
    // The Music tree's album row is the model: a fixed 14.5 px at weight 400, selected or not.
    let title_weight = 400.0;
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
    let progress_attrs = vec![Attribute::new(
        library_attr(props.attr, "progress"),
        "",
        None,
        false,
    )];
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
            span {
                class: "flex min-w-0 flex-1 flex-col",
                style: "gap: 2px;",
                Truncate {
                    text: props.title.clone(),
                    css: format!("color: {}; font-weight: {title_weight};", props.title_color),
                    style: TextStyle::sans(14.5, title_weight),
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
            if let Some(progress) = props.progress {
                // The resume position: a thin accent run under the row's
                // bottom edge, at the frame's bounded percent.
                span {
                    class: "absolute",
                    style: "left: 0; bottom: 0; height: 2px; width: {progress}%; background: var(--canvas-accent);",
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
