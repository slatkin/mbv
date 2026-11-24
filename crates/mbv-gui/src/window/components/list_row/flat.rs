//! The top-level row part (`gui-unified-list-rows` design D2): every flat-list
//! row, and the top-level row of every tree list. The look is the Music artist
//! row's: a fixed height, the row padding, the art thumb, the title over its
//! subtitle, the trailing text, and the square corners under the row divider.
//!
//! A destination passes its data, its state, and its handlers. It passes no
//! look value — no height, colour, fill, weight, or style string — so a
//! restyle lands here and reaches every list. The level rules (the title tone
//! and the selection fill) come from [`super`], and the art thumb from
//! [`super::thumb`]; this part composes them. The row sits in a
//! `row-hover-host` box with the shared hover controls as its later sibling
//! (gui-row-hover-actions design D2), and the box carries the row's fill as
//! `--row-fill` for both the row button and the controls.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeRowStateFrame;
use crate::window::components::library_attr::library_attr;
use crate::window::components::list_row::{
    LIBRARY_ROW_HEIGHT, LibraryRowThumb, LibraryTrailPart, ROW_DIVIDER_PX, ROW_PADDING_PX,
    RowLevel, context_press, hover_trail_class, paint_thumb, row_host_style,
};
use crate::window::components::row_hover_actions::RowHoverActions;
use crate::window::components::tree_guide::{TOP_GUIDE_X, parent_guide_start};
use crate::window::components::tree_row::{TREE_ROW_FONT_PX, tree_row_text};
use crate::window::library::LIBRARY_ROW_TITLE_WEIGHT;
use crate::window::library::episode::episode_progress;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The gap between the row's thumb and its text, in logical pixels (design D2).
const ROW_GAP_PX: f32 = 12.0;

/// The subtitle's and the trailing text's font size, in CSS pixels (design D2).
const SMALL_TEXT_PX: f32 = 12.0;

/// The props of the top-level row (design D2).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct FlatRowProps {
    /// The destination's hook prefix, as in `shows` or `playlists`; an empty
    /// prefix is the Music list.
    pub attr: &'static str,
    /// The row's own kind value, as in `artist`, `episode`, or `playlist`.
    pub kind: String,
    /// The row's stable id.
    pub id: String,
    /// The row's primary label.
    pub title: String,
    /// The row's secondary label, `None` while the row paints one line.
    pub subtitle: Option<String>,
    /// The trailing text, `None` while the row paints no gutter value.
    pub trail: Option<String>,
    /// The trailing text's hook part.
    pub trail_part: LibraryTrailPart,
    /// The row's art thumb, `None` on a row whose item has no art.
    #[props(default)]
    pub thumb: Option<LibraryRowThumb>,
    /// The row's play state, `None` on a list that carries none: it picks the
    /// title's tone (design D4) and the thumb's resume run.
    #[props(default)]
    pub play: Option<GuiHomeRowStateFrame>,
    /// Whether the selection marks the row.
    pub selected: bool,
    /// The row's open state as a tree parent, `None` on a row that is no tree
    /// parent. `Some(open)` adds the row's `aria-expanded`, picks the hover
    /// toggle control's glyph, and turns the row's double-click into
    /// [`Self::on_toggle`].
    #[props(default)]
    pub tree_open: Option<bool>,
    /// Whether the open top-level parent shows child rows: the row then paints
    /// its guide's start under its thumb instead of its divider, so the guide
    /// crosses no divider.
    #[props(default)]
    pub tree_guide: bool,
    /// A press on the row: it picks the row's item.
    pub on_press: Callback<()>,
    /// A double-click on the row.
    pub on_play: Callback<()>,
    /// A double-click on a tree parent: it opens or closes the node, in place
    /// of [`Self::on_play`]. `None` while the row is no tree parent. The hover
    /// controls paint their Expand/collapse control from it, so it is also
    /// that control's press.
    #[props(default)]
    pub on_toggle: Option<Callback<()>>,
    /// The hover controls' Play press: it runs the row's play path, the same
    /// one Enter runs (gui-row-hover-actions design D3, D4). `None` on a row
    /// that cannot play.
    #[props(default)]
    pub on_hover_play: Option<Callback<()>>,
    /// The hover controls' Add press: it runs the row's queue path, the same
    /// one `a` runs. `None` on a row that offers no Add.
    #[props(default)]
    pub on_hover_queue: Option<Callback<()>>,
    /// The hover controls' Shuffle press: it runs the row's shuffle path.
    /// `None` on a row that is no collection.
    #[props(default)]
    pub on_hover_shuffle: Option<Callback<()>>,
    /// A right-press on the row, with the pointer's client position; `None`
    /// while the row names no item a menu could open.
    #[props(default)]
    pub on_context: Option<Callback<(f32, f32)>>,
}

/// The top-level row: the destination's hooks paint from its prefix, and every
/// look value comes from the shared row module.
#[component]
pub fn FlatRow(props: FlatRowProps) -> Element {
    let attr = props.attr;
    // The row's own hooks: the kind and the id always paint, the selection mark
    // only while it holds.
    let mut row_attrs = vec![
        Attribute::new(library_attr(attr, "row"), props.kind.clone(), None, false),
        Attribute::new(library_attr(attr, "row-id"), props.id.clone(), None, false),
    ];
    if props.selected {
        row_attrs.push(Attribute::new(
            library_attr(attr, "selected"),
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

    // The row box (design D2): the fixed height, the row padding, the tight gap,
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
        "height: {LIBRARY_ROW_HEIGHT}px; box-sizing: border-box; padding: 0 {ROW_PADDING_PX}px; gap: {ROW_GAP_PX}px; border: 0; border-bottom: {divider_width}px solid {}; border-radius: 0; text-align: left; font-size: {TREE_ROW_FONT_PX}px; cursor: pointer;",
        canvas_var("border")
    );
    // The row's fill (gui-row-hover-actions design D2): it lives on the host as
    // `--row-fill`, so the row button and the hover controls that cover its
    // trailing end paint the same fill. A row the selection does not mark still
    // names one — the column it rests on — so the cover is never transparent
    // and the title it overlays never shows through.
    let fill = RowLevel::Top
        .selection_fill(props.selected)
        .unwrap_or_else(|| canvas_var("library-column"));
    let _ = write!(row_style, " background: var(--row-fill);");
    let host_style = row_host_style(&fill, divider_width);

    let title_attrs = vec![Attribute::new(library_attr(attr, "title"), "", None, false)];
    let title_css = format!(
        "color: {}; font-weight: {LIBRARY_ROW_TITLE_WEIGHT};",
        RowLevel::Top.title_tone(props.play.as_ref())
    );
    let subtitle_attrs = vec![Attribute::new(
        library_attr(attr, "subtitle"),
        "",
        None,
        false,
    )];
    let small_css = format!(
        "font-size: {SMALL_TEXT_PX}px; color: {};",
        canvas_var("muted")
    );
    let trail_text = props.trail.clone().filter(|trail| !trail.is_empty());
    let trail_attrs = trail_text.as_ref().map_or(Vec::new(), |trail| {
        vec![Attribute::new(
            library_attr(attr, props.trail_part.part()),
            trail.clone(),
            None,
            false,
        )]
    });

    // The art thumb, with the row's resume run along its bottom edge (design
    // D4): a row that carries no play state paints no run.
    let progress = props
        .play
        .as_ref()
        .and_then(episode_progress)
        .map(f32::from);
    let thumb = props
        .thumb
        .as_ref()
        .map(|thumb| paint_thumb(attr, thumb, progress));

    // The hover controls (gui-row-hover-actions design D1-D3): the trailing
    // text steps aside for them, and Expand reuses the row's own toggle, so a
    // tree parent offers it without a second handler. A row that offers no
    // control paints neither the part nor the trailing text's swap.
    let on_hover_play = props.on_hover_play;
    let on_hover_queue = props.on_hover_queue;
    let on_hover_shuffle = props.on_hover_shuffle;
    let on_toggle = props.on_toggle;
    let paints_actions = on_hover_play.is_some()
        || on_hover_queue.is_some()
        || on_hover_shuffle.is_some()
        || on_toggle.is_some();
    let row_class = if props.tree_open.is_some() {
        "row-hover-face relative flex w-full items-center"
    } else {
        "row-hover-face relative flex w-full items-center overflow-hidden"
    };
    let trail_class = hover_trail_class(paints_actions);
    let on_double_click = props.on_toggle.unwrap_or(props.on_play);
    let on_press = props.on_press;
    let on_context = props.on_context;
    let show_guide = props.tree_guide;
    let tree_open = props.tree_open;

    // The host is the hover box (design D2): the row button keeps its own
    // semantics, and the controls are its later sibling, never a child.
    rsx! {
        div {
            class: "row-hover-host row-hover-row",
            style: "{host_style}",
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
                        style: tree_row_text(),
                        attrs: title_attrs,
                    }
                    if let Some(subtitle) = props.subtitle.clone() {
                        Truncate {
                            text: subtitle,
                            css: small_css.clone(),
                            style: TextStyle::sans(SMALL_TEXT_PX, 400.0),
                            attrs: subtitle_attrs,
                        }
                    }
                }
                if let Some(trail) = trail_text {
                    span {
                        class: "{trail_class}",
                        style: "{small_css}",
                        ..trail_attrs,
                        "{trail}"
                    }
                }
                if show_guide {
                    {parent_guide_start(TOP_GUIDE_X, LIBRARY_ROW_HEIGHT, 0.0)}
                }
            }
            if paints_actions {
                RowHoverActions {
                    on_play: on_hover_play,
                    on_queue: on_hover_queue,
                    on_shuffle: on_hover_shuffle,
                    on_toggle,
                    open: tree_open.unwrap_or(false),
                    row_name: Some(props.title.clone()),
                }
            }
        }
    }
}
