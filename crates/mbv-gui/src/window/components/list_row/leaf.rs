//! The leaf row part (`gui-tree-row-roles` design D1, D3, D7): a tree list's
//! terminal row — a Music track, a Shows episode, a Playlists item. The row
//! paints one segment of its parent's leaf well: an outer strip in the group
//! fill with the muted rail at its left edge, and the recessed well segment
//! inside the strip, whose rounded corners, top shade, and bottom lip are the
//! solid depth cues the brief's inner shadow falls back to (design D3, R1).
//! The well segment is the hover host, so the pointer look lands on the row's
//! own 28 px button.
//!
//! A destination names the row's data and its state; it never names a height,
//! colour, fill, weight, tone, or style string, so a restyle lands here and
//! reaches every list (invariant 30). The shared level rules — the title's
//! tone and weight over the play state and the selection fill — come from
//! [`super::RowLevel`]. A row that carries a play state derives its look from
//! [`RowPlay`](crate::window::components::play_mark::RowPlay) and paints the
//! one trailing cluster the hover controls hide together; a row that carries
//! none paints the bare duration.
//!
//! The strip's segments of adjacent rows meet: every row's height is a whole
//! number of pixels and the well's gaps are part of it —
//! [`WellEdge::height`] —, so the well reads whole under any scroll (design
//! D2, D3). The group's last row paints the root's divider on the strip,
//! which the open root gave up (design D2).

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeRowStateFrame;

use super::{
    ROW_PADDING_PX, RowLevel, TRACK_ROW_HEIGHT, WELL_GAP_BOTTOM_PX, WELL_GAP_TOP_PX, WellEdge,
    context_press, hover_trail_class, row_host_style,
};
use crate::window::components::library_attr::library_attr;
use crate::window::components::play_mark::{PlayCluster, RowPlay};
use crate::window::components::row_hover_actions::RowHoverActions;
use crate::window::components::tree_guide::tail_divider_style;
use crate::window::components::tree_rail::{RailTone, TreeRail};
use crate::window::components::tree_row::{LEAF_ROW_FONT_PX, leaf_label_css};
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The leaf well segment's left edge in the strip, in logical pixels (design
/// D3; Open Question 11): it aligns with the Branch swatch's left edge, as
/// the brief's text asks.
pub const WELL_X: f32 = 20.0;

/// The gap the strip keeps between the well segment and its right edge, in
/// logical pixels (design D3): the brief's 12 px, so the well stops short of
/// the list's right edge.
pub const WELL_RIGHT_GAP_PX: f32 = 12.0;

/// The leaf well segment's corner radius, in logical pixels (design D6): the
/// brief's 6 px, on the well's first and last rows' outer corners.
pub const WELL_RADIUS_PX: f32 = 6.0;

/// The height of every 1 px line the leaf well paints: the line between two
/// leaf rows of one well (design D6) and the solid 1 px depth cues on the
/// well's top and bottom edges (design D3).
pub const WELL_RULE_PX: f32 = 1.0;

/// The gap between the leaf row's number, title, and duration, in logical
/// pixels (design D3).
const ROW_GAP_PX: f32 = 8.0;

/// The number cell's width on a leaf row, in logical pixels (design D3): the
/// right-aligned ordinal before the title.
const NUMBER_CELL_PX: f32 = 18.0;

/// The number cell's and the duration's font size, in CSS pixels (design D3).
const TRAIL_FONT_PX: f32 = 12.0;

/// The props of a leaf row (design D1, D3).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LeafRowProps {
    /// The destination's hook prefix, as in `shows` or `playlists`; an empty
    /// prefix is the Music list.
    pub attr: &'static str,
    /// The row's own kind value, as in `track`, `episode`, or `item`.
    pub kind: String,
    /// The row's stable id.
    pub id: String,
    /// The row's primary label.
    pub title: String,
    /// The row's place in its parent's leaf well: it names the row's height,
    /// the well segment's rounded corners, and the edge lines it paints
    /// (design D3).
    pub well: WellEdge,
    /// The row's number cell, `None` on a row that paints no number.
    #[props(default)]
    pub number: Option<String>,
    /// The row's runtime, `None` while the row paints none.
    #[props(default)]
    pub duration: Option<String>,
    /// The row's play state, `None` on a list that carries none: it picks the
    /// title's tone and the trailing cluster (design D4).
    #[props(default)]
    pub play: Option<GuiHomeRowStateFrame>,
    /// Whether the selection marks the row.
    pub selected: bool,
    /// Whether the row closes an open subtree: the strip then paints the
    /// root's divider under itself, in the height the open root gave up
    /// (design D2).
    #[props(default)]
    pub tail: bool,
    /// A press on the row: it picks the row's item.
    pub on_press: Callback<()>,
    /// A double-click on the row: it plays the row.
    pub on_play: Callback<()>,
    /// The hover controls' Play press: it runs the row's play path, the same
    /// one Enter runs (gui-row-hover-actions design D3, D4). `None` on a row
    /// that cannot play.
    #[props(default)]
    pub on_hover_play: Option<Callback<()>>,
    /// The hover controls' Add press: it runs the row's queue path, the same
    /// one `a` runs. `None` on a row that offers no Add.
    #[props(default)]
    pub on_hover_queue: Option<Callback<()>>,
    /// A right-press on the row, with the pointer's client position; `None`
    /// while the row names no item a menu could open.
    #[props(default)]
    pub on_context: Option<Callback<(f32, f32)>>,
}

/// The leaf row: one strip segment of the group, with the well segment and
/// the row's button inside it.
#[component]
pub fn LeafRow(props: LeafRowProps) -> Element {
    let attr = props.attr;
    let well = props.well;

    // The strip (design D3): the group's fill, the well's gaps as padding so
    // the row's height holds them, the well's insets, and the 1 px divider
    // under the group's last row, which the open root gave up (design D2).
    let height = well.height();
    let top_gap = match well {
        WellEdge::Only | WellEdge::First => WELL_GAP_TOP_PX,
        WellEdge::Middle | WellEdge::Last => 0.0,
    };
    let bottom_gap = match well {
        WellEdge::Only | WellEdge::Last => WELL_GAP_BOTTOM_PX,
        WellEdge::First | WellEdge::Middle => 0.0,
    };
    let mut strip_style = format!(
        "height: {height}px; box-sizing: border-box; padding: {top_gap}px {WELL_RIGHT_GAP_PX}px {bottom_gap}px {WELL_X}px; background: {};",
        canvas_var("tree-subtree")
    );
    strip_style.push_str(&tail_divider_style(props.tail));

    // The well segment (design D3, D6): the well's fill and the rounded
    // corners of the edges the row sits on, square on the inner seams. The
    // edge segments clip their content, so the selection fill and the hover
    // cover take the well's corners too.
    let radii = well_radii(well);
    let clips = well != WellEdge::Middle;
    let segment_style = format!(
        "height: {TRACK_ROW_HEIGHT}px; border-radius: {radii}; background: {};",
        canvas_var("tree-well")
    );

    // The row's fill (gui-row-hover-actions design D2): the well's fill, or
    // the selection's fill while the selection marks the row. It lives on the
    // host as `--row-fill`, so the row button and the hover controls that
    // cover its trailing end paint the same fill, and the segment's clip
    // rounds both. The tail divider is the strip's, below the segment, so the
    // cover never reaches it.
    let fill = RowLevel::Leaf
        .selection_fill(props.selected)
        .unwrap_or_else(|| canvas_var("tree-well"));
    let host_style = row_host_style(&fill, 0.0);

    // The row's play look (gui-played-row-marks design D1): one derivation
    // decides the title's tone and weight and the trail's tone. A row that
    // carries no play state keeps the level's own look.
    let row_play = props.play.as_ref().map(RowPlay::of_row);
    let title_weight = RowLevel::Leaf.title_weight(props.play.as_ref());
    let title_tone = RowLevel::Leaf.title_tone(props.play.as_ref());
    let title_attrs = vec![Attribute::new(library_attr(attr, "title"), "", None, false)];
    let number = props.number.clone().filter(|number| !number.is_empty());
    let duration = props
        .duration
        .clone()
        .filter(|duration| !duration.is_empty());
    let duration_attrs = duration.clone().map(|duration| {
        vec![Attribute::new(
            library_attr(attr, "duration"),
            duration,
            None,
            false,
        )]
    });

    // The hover controls (gui-row-hover-actions design D1-D3): the trailing
    // duration steps aside for them. A row that offers no control paints
    // neither the part nor the trailing text's swap.
    let on_hover_play = props.on_hover_play;
    let on_hover_queue = props.on_hover_queue;
    let paints_actions = on_hover_play.is_some() || on_hover_queue.is_some();
    let trail_class = hover_trail_class(paints_actions);

    // The trailing end (gui-played-row-marks design D3): a row that carries a
    // play state paints the one trailing cluster — the mark, then the
    // duration — and the cluster carries the hover swap, so the mark and the
    // duration hide together while the hover controls show. A row that
    // carries no play state keeps the bare duration it always painted. A leaf
    // row paints no resume mark, so it keeps its play mark (user request
    // 2026-11-26).
    let trail_tone = row_play.and_then(RowPlay::trail_tone);
    let trailing: Option<Element> = if let Some(play) = row_play {
        // The text inside the cluster never carries the trail marker — the
        // cluster does — so it paints the trail class of a row nothing covers.
        let inner_class = hover_trail_class(false);
        let text = duration.clone().map(|duration| {
            let color = trail_tone.map_or_else(|| canvas_var("muted"), str::to_owned);
            let attrs = duration_attrs.clone().unwrap_or_default();
            rsx! {
                span {
                    class: "{inner_class}",
                    style: "font-size: {TRAIL_FONT_PX}px; color: {color};",
                    ..attrs,
                    "{duration}"
                }
            }
        });
        Some(rsx! {
            PlayCluster { class: trail_class, play: Some(play), {text} }
        })
    } else {
        duration.map(|duration| {
            let attrs = duration_attrs.unwrap_or_default();
            let color = canvas_var("muted");
            rsx! {
                span {
                    class: "{trail_class}",
                    style: "font-size: {TRAIL_FONT_PX}px; color: {color};",
                    ..attrs,
                    "{duration}"
                }
            }
        })
    };

    // The row button (design D3): the 28 px face, with the 1 px separator
    // inside its height on the well's inner rows, so the line never blanks a
    // rounded corner.
    let separator = match well {
        WellEdge::Middle | WellEdge::Last => {
            format!("{}px solid {}", WELL_RULE_PX, canvas_var("tree-well-rule"))
        }
        WellEdge::Only | WellEdge::First => "0".to_owned(),
    };
    let button_style = format!(
        "height: {TRACK_ROW_HEIGHT}px; box-sizing: border-box; padding: 0 {ROW_PADDING_PX}px; gap: {ROW_GAP_PX}px; border: 0; border-top: {separator}; border-radius: 0; text-align: left; font-size: {LEAF_ROW_FONT_PX}px; cursor: pointer; background: var(--row-fill);"
    );

    // The title's tone (design D7): the shared level rule's tone — the play
    // state's on a row that carries one, the text tone otherwise. A row that
    // carries a play state also paints the weight the state derives, over the
    // leaf label's own 500.
    let title_css = if props.play.is_some() {
        format!("color: {title_tone}; font-weight: {title_weight};")
    } else {
        format!("color: {title_tone};")
    };
    let on_press = props.on_press;
    let on_play = props.on_play;
    let on_context = props.on_context;
    let title = props.title.clone();

    // The strip is the well's own box (design D3): the rail paints in it, and
    // the well segment — the hover host — sits inside its padding, so the
    // pointer look covers only the row's own 28 px.
    rsx! {
        div {
            class: "relative",
            style: "{strip_style}",
            TreeRail { tone: RailTone::Group }
            div {
                class: if clips { "relative overflow-hidden" } else { "relative" },
                style: "{segment_style}",
                "data-tree-well": well_edge_mark(well),
                div {
                    class: "row-hover-host row-hover-row",
                    style: "{host_style}",
                    button {
                        "type": "button",
                        class: "row-hover-face relative flex w-full items-center",
                        style: "{button_style}",
                        onclick: move |_| on_press.call(()),
                        ondoubleclick: move |_| on_play.call(()),
                        oncontextmenu: move |event: MouseEvent| context_press(&event, on_press, on_context),
                        ..row_attrs(props.kind.clone(), props.id.clone(), props.selected, attr),
                        if let Some(number) = number.clone() {
                            {number_cell(attr, &number)}
                        }
                        div {
                            class: "min-w-0 flex-1",
                            style: leaf_label_css(),
                            Truncate {
                                text: title,
                                css: title_css,
                                style: TextStyle::mono(LEAF_ROW_FONT_PX, title_weight),
                                attrs: title_attrs,
                            }
                        }
                        if let Some(trailing) = trailing {
                            {trailing}
                        }
                    }
                    if paints_actions {
                        RowHoverActions {
                            on_play: on_hover_play,
                            on_queue: on_hover_queue,
                            on_toggle: None,
                            open: false,
                            row_name: Some(props.title.clone()),
                        }
                    }
                }
                if let Some(edges) = well_edge_lines(well) {
                    {edges}
                }
            }
        }
    }
}

/// The row button's own hooks: the kind and the id always paint, the
/// selection mark only while it holds.
fn row_attrs(kind: String, id: String, selected: bool, attr: &'static str) -> Vec<Attribute> {
    let mut attrs = vec![
        Attribute::new(library_attr(attr, "row"), kind, None, false),
        Attribute::new(library_attr(attr, "row-id"), id, None, false),
    ];
    if selected {
        attrs.push(Attribute::new(
            library_attr(attr, "selected"),
            "",
            None,
            false,
        ));
    }
    attrs
}

/// The mark a well segment carries in `data-tree-well` (design D9).
#[must_use]
fn well_edge_mark(edge: WellEdge) -> &'static str {
    match edge {
        WellEdge::Only => "only",
        WellEdge::First => "first",
        WellEdge::Middle => "middle",
        WellEdge::Last => "last",
    }
}

/// The well segment's corner radii (design D3): the well's radius on the
/// outer corners of the edges the row sits on, square on the inner seams.
#[must_use]
fn well_radii(edge: WellEdge) -> String {
    let r = WELL_RADIUS_PX;
    match edge {
        WellEdge::Only => format!("{r}px {r}px {r}px {r}px"),
        WellEdge::First => format!("{r}px {r}px 0px 0px"),
        WellEdge::Middle => "0px".to_owned(),
        WellEdge::Last => format!("0px 0px {r}px {r}px"),
    }
}

/// The 1 px edge lines a well segment paints over the row's fill (design D3,
/// D6): the darker shade on the well's top edge on the first row, the root
/// divider's lighter line on its bottom edge on the last row, and both on a
/// one-row well. The lines are absolute spans the segment's clip rounds, so a
/// rounded corner beside a zero-width border paints no tapered arc
/// (design R3).
#[must_use]
fn well_edge_lines(edge: WellEdge) -> Option<Element> {
    let shade = matches!(edge, WellEdge::Only | WellEdge::First);
    let lip = matches!(edge, WellEdge::Only | WellEdge::Last);
    if !shade && !lip {
        return None;
    }
    let shade_line = shade.then(|| {
        rsx! {
            span {
                class: "pointer-events-none absolute",
                style: shade_line_style(),
            }
        }
    });
    let lip_line = lip.then(|| {
        rsx! {
            span {
                class: "pointer-events-none absolute",
                style: lip_line_style(),
            }
        }
    });
    Some(rsx! {
        if let Some(shade_line) = shade_line {
            {shade_line}
        }
        if let Some(lip_line) = lip_line {
            {lip_line}
        }
    })
}

/// The style of the well's top-edge line (design D6): the 1 px shade across
/// the segment's top edge.
#[must_use]
fn shade_line_style() -> String {
    format!(
        "left: 0px; right: 0px; top: 0px; height: {WELL_RULE_PX}px; background: {};",
        canvas_var("tree-well-shade")
    )
}

/// The style of the well's bottom-edge line (design D6): the root divider's
/// 1 px lighter line across the segment's bottom edge.
#[must_use]
fn lip_line_style() -> String {
    format!(
        "left: 0px; right: 0px; bottom: 0px; height: {WELL_RULE_PX}px; background: {};",
        canvas_var("border")
    )
}

/// A leaf row's number cell (design D3): the right-aligned ordinal before the
/// title, in the ordinal tone.
fn number_cell(attr: &'static str, number: &str) -> Element {
    let attrs = vec![Attribute::new(
        library_attr(attr, "number"),
        number.to_owned(),
        None,
        false,
    )];
    rsx! {
        span {
            class: "flex flex-none items-center justify-end font-mono tabular-nums",
            style: "width: {NUMBER_CELL_PX}px; font-size: {TRAIL_FONT_PX}px; color: var(--canvas-ordinal);",
            ..attrs,
            "{number}"
        }
    }
}
