//! The flat Library list's item row (`replace-tauri-with-blitz` task 7.2,
//! the port of `LibraryItemRow.svelte`; add-gui-library-panel design D15,
//! invariant 30): the title over its subtitle, the date or duration at the
//! right, the cursor and selection fills, and the resume run under the row.
//! Every flat destination list — Emby, Audiobooks, Podcasts, Feeds — paints
//! its rows here, so a fix to one row look lands in every list.
//!
//! A destination supplies its own hook prefix (`attr`), the row's text, and
//! the press, double-click, and context handlers; it never restates the
//! look. Every part paints `data-library-<attr>-<part>`.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::window::components::library_attr::library_attr;
use crate::window::input::f32_from_f64;

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

/// The props of the item row (the `LibraryItemRow.svelte` props, in order).
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
    /// The row's painted fill, when the cursor or the selection marks it.
    pub background: Option<String>,
    /// Whether the cursor rests on the row.
    pub cursor: bool,
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

/// The flat list's item row (the Svelte markup, character for character):
/// the destination's hooks paint from its prefix, so a destination's tests
/// keep their per-row parts beside the shared look.
#[component]
pub fn LibraryItemRow(props: LibraryItemRowProps) -> Element {
    // The row's own hooks (the Svelte `own` helper): the kind and the id
    // always paint, the cursor and selection marks only while they hold.
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
    if props.cursor {
        row_attrs.push(Attribute::new(
            library_attr(props.attr, "cursor"),
            "",
            None,
            false,
        ));
    }
    if props.selected {
        row_attrs.push(Attribute::new(
            library_attr(props.attr, "selected"),
            "",
            None,
            false,
        ));
    }
    let mut row_style = format!(
        "height: {}px; padding: 0 12px; gap: 10px; border: 0; border-radius: 4px; text-align: left; font-size: 14px; cursor: pointer;",
        props.height
    );
    if let Some(background) = props.background.as_ref() {
        let _ = write!(row_style, " background: {background};");
    }
    let title_weight = if props.selected { 600 } else { 400 };
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
                span {
                    class: "truncate",
                    style: "color: {props.title_color}; font-weight: {title_weight};",
                    ..title_attrs,
                    "{props.title}"
                }
                if let Some(subtitle) = props.subtitle.clone() {
                    span {
                        class: "truncate",
                        style: "font-size: 12px; color: var(--canvas-muted);",
                        ..subtitle_attrs,
                        "{subtitle}"
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
