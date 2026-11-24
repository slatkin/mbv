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
//! `data-library-<attr>-thumb-progress`. A top-level tree node passes
//! `tree_open` where it is a tree parent: the part then carries `tree-parent`,
//! its `aria-expanded`, the `tree-trailing` mark on its trailing text, and the
//! shared `TreeHint` at its trailing end, and its double-click toggles the node
//! through `on_toggle` instead of playing it (gui-tree-double-click design D4).
//! No row paints a caret: the open state shows as the children under the row,
//! and the hover hint names the gesture (design D2). A top-level tree parent
//! passes `tree_guide` while it shows child rows: the row then paints its own
//! guide's start through the shared [`TreeGuide`] part
//! (gui-tree-indent-guides design D3, D4).
//!
//! [`paint_item_row`] paints one row of a flat entry list — Podcasts and Feeds
//! — from its carried entry, so the two lists keep one row painter. The thumb
//! painter, the row-level rules, and the look constants live in the shared row
//! module (`list_row`, `gui-unified-list-rows` design D1).

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiEntryRowFrame, GuiHomeRowStateFrame};
use crate::window::components::library_attr::library_attr;
use crate::window::components::list_row::{RowLevel, context_press};
use crate::window::components::tree_guide::{TOP_GUIDE_X, parent_guide_start};
use crate::window::components::tree_hint::TreeHint;
use crate::window::components::tree_row::TREE_ROW_FONT_PX;
use crate::window::icons::Check;
use crate::window::library::LIBRARY_ROW_TITLE_WEIGHT;
use crate::window::library::episode::episode_progress;
use crate::window::theme::{EfToken, canvas_var, ef_var};
use crate::window::truncate::{TextStyle, Truncate};

// The row's look values now live in the shared row module (`list_row`,
// `gui-unified-list-rows` design D1); this part re-exports them while it owns
// the top-level row, so a destination keeps one import path until it paints
// `FlatRow`.
pub use crate::window::components::list_row::{
    LIBRARY_ROW_HEIGHT, LibraryRowThumb, LibraryTrailPart, ROW_DIVIDER_PX, ROW_PADDING_PX,
    THUMB_HEIGHT_PX, THUMB_ICON_PX, THUMB_PROGRESS_PX, THUMB_RADIUS_PX, THUMB_SQUARE_PX,
    THUMB_WIDTH_PX, paint_thumb,
};

/// The unplayed mark's filled circle, in logical pixels.
const PLAYED_DOT_PX: f32 = 8.0;

/// The played mark's check icon, in logical pixels.
const PLAYED_CHECK_PX: u16 = 12;

/// The mark slot's width and height: the larger of the two marks, so the date
/// stays at one place whatever the row's state.
const PLAYED_MARK_SLOT_PX: u16 = PLAYED_CHECK_PX;

/// The gap between the mark slot and the date.
const PLAYED_MARK_GAP_PX: f32 = 6.0;

/// The played treatment a row paints (the `Recently added` rows, user request
/// 2026-11-23): the title, subtitle, and date tones, and the mark before the
/// date. A partly played row paints [`Self::Unplayed`] and keeps its resume run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowPlayedMark {
    /// Not played, or partly played.
    Unplayed,
    /// Played.
    Played,
}

impl RowPlayedMark {
    /// The mark's accessible label and its hook value.
    fn label(self) -> &'static str {
        match self {
            RowPlayedMark::Unplayed => "Unplayed",
            RowPlayedMark::Played => "Played",
        }
    }

    /// The title's tone and weight.
    fn title(self) -> (&'static str, f32) {
        match self {
            RowPlayedMark::Unplayed => (ef_var(EfToken::Fg), 600.0),
            RowPlayedMark::Played => (ef_var(EfToken::Grey1), 400.0),
        }
    }

    /// The subtitle's tone.
    fn subtitle_color(self) -> &'static str {
        match self {
            RowPlayedMark::Unplayed => ef_var(EfToken::Grey2),
            RowPlayedMark::Played => ef_var(EfToken::Grey1),
        }
    }

    /// The date's tone.
    fn date_color(self) -> &'static str {
        match self {
            RowPlayedMark::Unplayed => ef_var(EfToken::Fg),
            RowPlayedMark::Played => ef_var(EfToken::Grey1),
        }
    }
}

/// The mark before the date: the green dot of an unplayed row or the grey
/// check of a played one, in one fixed slot so the dates stay aligned.
fn paint_played_mark(attr: &'static str, mark: RowPlayedMark) -> Element {
    let attrs = vec![Attribute::new(
        library_attr(attr, "played-mark"),
        mark.label(),
        None,
        false,
    )];
    let glyph = match mark {
        RowPlayedMark::Unplayed => rsx! {
            span {
                style: "width: {PLAYED_DOT_PX}px; height: {PLAYED_DOT_PX}px; border-radius: 9999px; background: {ef_var(EfToken::Green)};",
            }
        },
        RowPlayedMark::Played => rsx! {
            Check { size: PLAYED_CHECK_PX, color: ef_var(EfToken::Grey1).to_owned() }
        },
    };
    rsx! {
        span {
            class: "flex flex-none items-center justify-center",
            style: "width: {PLAYED_MARK_SLOT_PX}px; height: {PLAYED_MARK_SLOT_PX}px;",
            role: "img",
            "aria-label": mark.label(),
            ..attrs,
            {glyph}
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
    /// The row's open state as a tree parent (gui-tree-double-click design D4),
    /// `None` on a row that is no tree parent. `Some(open)` adds the
    /// `tree-parent` class and the row's `aria-expanded`, marks its trailing
    /// text `tree-trailing`, paints the shared [`TreeHint`], and turns the
    /// row's double-click into [`Self::on_toggle`].
    #[props(default)]
    pub tree_open: Option<bool>,
    /// Whether the open top-level parent shows child rows: the row then paints
    /// its guide's start under its thumb, down through its subtree
    /// (gui-tree-indent-guides design D3, D4). `false` on a row that is no
    /// top-level tree parent and on a parent that shows none of its children.
    #[props(default)]
    pub tree_guide: bool,
    /// The row's played treatment (the `Recently added` rows), `None` on every
    /// other row: it then overrides [`Self::title_color`] and the muted tones,
    /// dims a played thumb, and paints its mark before the date.
    #[props(default)]
    pub played_mark: Option<RowPlayedMark>,
    /// A press on the row: it picks the row's item.
    pub on_press: Callback<()>,
    /// A double-click on the row.
    pub on_play: Callback<()>,
    /// A double-click on a tree parent: it opens or closes the node, in place
    /// of [`Self::on_play`] (gui-tree-double-click design D4). `None` while the
    /// row is no tree parent.
    #[props(default)]
    pub on_toggle: Option<Callback<()>>,
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
    if let Some(open) = props.tree_open {
        row_attrs.push(Attribute::new(
            "aria-expanded",
            if open { "true" } else { "false" },
            None,
            false,
        ));
    }
    // The top-level row look (design D2): the fixed height, the tight padding,
    // the square corners, and the 1 px divider under the row. `border-box`
    // keeps the divider inside the row height the list's geometry holds. An
    // open parent that shows children paints none: its subtree's last row
    // paints it, so the guides never cross it.
    let divider_width = if props.tree_guide {
        0.0
    } else {
        ROW_DIVIDER_PX
    };
    let mut row_style = format!(
        "height: {}px; box-sizing: border-box; padding: 0 {ROW_PADDING_PX}px; gap: 12px; border: 0; border-bottom: {divider_width}px solid {}; border-radius: 0; text-align: left; font-size: {TREE_ROW_FONT_PX}px; cursor: pointer;",
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
    let muted = canvas_var("muted");
    let (title_color, title_weight): (&str, f32) = match props.played_mark {
        Some(mark) => mark.title(),
        None => (&props.title_color, LIBRARY_ROW_TITLE_WEIGHT),
    };
    let title_css = format!("color: {title_color}; font-weight: {title_weight};");
    let subtitle_color: &str = match props.played_mark {
        Some(mark) => mark.subtitle_color(),
        None => &muted,
    };
    let subtitle_css = format!("font-size: 12px; color: {subtitle_color};");
    let date_color: &str = match props.played_mark {
        Some(mark) => mark.date_color(),
        None => &muted,
    };
    let mut trail_style = format!("font-size: 12px; color: {date_color};");
    if props.played_mark.is_some() {
        let _ = write!(
            trail_style,
            " display: flex; align-items: center; gap: {PLAYED_MARK_GAP_PX}px;"
        );
    }
    let trail_text = props.trail.clone().filter(|trail| !trail.is_empty());
    let paints_trail = trail_text.is_some() || props.played_mark.is_some();
    let played_mark = props.played_mark;
    let attr = props.attr;
    // A tree parent's row look (gui-tree-double-click design D2, D4): the hover
    // hint and its trailing-text swap ride the `tree-parent` class, and the
    // double-click toggles the node in place of playing it.
    let (row_class, trail_class) = if props.tree_open.is_some() {
        (
            "tree-parent relative flex w-full items-center",
            "tree-trailing flex-none font-mono tabular-nums",
        )
    } else {
        (
            "relative flex w-full items-center overflow-hidden",
            "flex-none font-mono tabular-nums",
        )
    };
    let on_double_click = props.on_toggle.unwrap_or(props.on_play);
    let on_press = props.on_press;
    let on_context = props.on_context;

    rsx! {
        button {
            "type": "button",
            class: "{row_class}",
            style: "{row_style}",
            onclick: move |_| on_press.call(()),
            ondoubleclick: move |_| on_double_click.call(()),
            oncontextmenu: move |event: MouseEvent| context_press(&event, on_press, on_context),
            ..row_attrs,
            if let Some(thumb) = thumb {
                {thumb}
            }
            span {
                class: "flex min-w-0 flex-1 flex-col",
                style: "gap: 2px;",
                Truncate {
                    text: props.title.clone(),
                    css: title_css,
                    style: TextStyle::sans(TREE_ROW_FONT_PX, title_weight),
                    attrs: title_attrs,
                }
                if let Some(subtitle) = props.subtitle.clone() {
                    Truncate {
                        text: subtitle,
                        css: subtitle_css,
                        style: TextStyle::sans(12.0, 400.0),
                        attrs: subtitle_attrs,
                    }
                }
            }
            if paints_trail {
                span {
                    class: "{trail_class}",
                    style: "{trail_style}",
                    ..trail_attrs,
                    if let Some(mark) = played_mark {
                        {paint_played_mark(attr, mark)}
                    }
                    if let Some(trail) = trail_text {
                        "{trail}"
                    }
                }
            }
            if props.tree_guide {
                {parent_guide_start(TOP_GUIDE_X, props.height, 0.0)}
            }
            if let Some(open) = props.tree_open {
                TreeHint { open }
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

/// An item row's title colour: the shared top-level tone (`gui-unified-list-rows`
/// design D4), dimmed on a played row (design D12).
#[must_use]
pub fn name_color(state: &GuiHomeRowStateFrame) -> String {
    RowLevel::Top.title_tone(Some(state))
}

/// The mark a row paints only while it holds the state (the former client
/// `undefined` values painted nothing).
#[must_use]
pub fn mark(name: &'static str, on: bool) -> Vec<Attribute> {
    on.then(|| Attribute::new(name, "", None, false))
        .into_iter()
        .collect()
}
