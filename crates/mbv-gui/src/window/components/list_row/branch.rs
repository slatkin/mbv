//! The branch row part (`gui-tree-row-roles` design D1, D2, D5, D7): a tree
//! list's expandable row under a root — a Music album, a Shows season. The
//! row paints the group fill it sits on, the muted rail segment at its left
//! edge, its Branch swatch, the title over its facts line, and the state mark
//! that says whether the row is open, and it is the hover host with the
//! shared controls, whose Expand control is the row's own toggle.
//!
//! A destination names the row's data and its state; it never names a height,
//! colour, fill, weight, tone, or style string, so a restyle lands here and
//! reaches every list (invariant 30). The shared level rules — the title's
//! rest tone and weight and the selection fill — come from
//! [`super::RowLevel`]; the open state's look is this part's own derivation
//! from design D7, because [`super::RowLevel`] carries no open state.
//!
//! The row's fill lives on the hover host as `--row-fill`
//! (gui-row-hover-actions design D2), so the row button and the controls that
//! cover its trailing end paint the same fill, with the precedence
//! selection, open, group (design D7). The group's last row paints the root's
//! divider, which the open root gave up (design D2).

use dioxus_native::prelude::*;

use super::{
    BRANCH_ROW_HEIGHT, ROW_DIVIDER_PX, ROW_PADDING_PX, RowLevel, context_press, row_host_style,
};
use crate::window::components::library_attr::library_attr;
use crate::window::components::row_hover_actions::RowHoverActions;
use crate::window::components::tree_guide::tail_divider_style;
use crate::window::components::tree_rail::{RailTone, TreeRail, swatch_index, swatch_var};
use crate::window::components::tree_row::TREE_ROW_FONT_PX;
use crate::window::icons::{ChevronDown, ChevronRight};
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The Branch swatch's left edge in the row box, in logical pixels (design
/// D5): the mockup's 20 px inset, which clears the 3 px group rail the row
/// paints at its left edge.
pub const SWATCH_X: f32 = 20.0;

/// The Branch swatch's side, in logical pixels (design D4): the brief's 28 px
/// square, centred in the 42 px row.
pub const SWATCH_PX: f32 = 28.0;

/// The Branch swatch's corner radius, in logical pixels (design D4): the
/// brief's 4 px rounded square.
pub const SWATCH_RADIUS_PX: f32 = 4.0;

/// The gap between the Branch swatch and the text column, in logical pixels
/// (design D5): the mockup's 12 px.
pub const SWATCH_TEXT_GAP_PX: f32 = 12.0;

/// The facts line's font size, in CSS pixels (design D5): the brief's mono
/// 11 px secondary line.
pub const FACTS_FONT_PX: f32 = 11.0;

/// The state mark's glyph size, in CSS pixels (design D5): the brief's 14 px
/// chevron.
const MARK_PX: u16 = 14;

/// The open branch title's weight (design D7): the brief's brighter, heavier
/// open title. The rest weight is [`RowLevel::Branch`]'s own.
const OPEN_TITLE_WEIGHT: f32 = 500.0;

/// The props of a branch row (design D1, D5).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct BranchRowProps {
    /// The destination's hook prefix, as in `shows`; an empty prefix is the
    /// Music list.
    pub attr: &'static str,
    /// The row's own kind value, as in `album` or `season`.
    pub kind: String,
    /// The row's stable id.
    pub id: String,
    /// The row's primary label.
    pub title: String,
    /// The row's position among the painted siblings: it picks the Branch
    /// swatch (design D4).
    pub sibling: usize,
    /// The row's secondary line, as in `1984 · 20 tracks · 1:12:09`; an empty
    /// string paints none and the title centres (design D5).
    pub facts: String,
    /// The row's open state, `None` on a branch that opens nothing.
    /// `Some(open)` adds the row's `aria-expanded`, picks the state mark's
    /// glyph, and turns the row's double-click into [`Self::on_toggle`].
    #[props(default)]
    pub open: Option<bool>,
    /// Whether the selection marks the row.
    pub selected: bool,
    /// Whether the row closes an open subtree: it then paints the root's
    /// divider under itself, in the height the open root gave up (design D2).
    #[props(default)]
    pub tail: bool,
    /// A press on the row: it picks the row's item.
    pub on_press: Callback<()>,
    /// A double-click on the row.
    pub on_play: Callback<()>,
    /// A double-click on the row that opens: it opens or closes the node, in
    /// place of [`Self::on_play`]. `None` while the row opens nothing. The
    /// hover controls paint their Expand control from it, so it is also that
    /// control's press.
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

/// The branch row: the group's fill and rail, the Branch swatch, the title
/// over its facts line, and the state mark, in the hover host.
#[component]
pub fn BranchRow(props: BranchRowProps) -> Element {
    let attr = props.attr;
    let open = props.open;
    let is_open = open == Some(true);

    // The row's own hooks: the kind and the id always paint, the selection
    // mark only while it holds, and the open state as `aria-expanded`.
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
    if let Some(open) = open {
        row_attrs.push(Attribute::new(
            "aria-expanded",
            if open { "true" } else { "false" },
            None,
            false,
        ));
    }

    // The row's fill (design D7): the selection, then the open state, then the
    // group's fill. The fill lives on the host as `--row-fill`
    // (gui-row-hover-actions design D2), so the row button and the hover
    // controls that cover its trailing end paint the same fill.
    let fill = if props.selected {
        canvas_var(if is_open {
            "tree-branch-open-picked"
        } else {
            "library-raise"
        })
    } else if is_open {
        canvas_var("tree-branch-open")
    } else {
        canvas_var("tree-subtree")
    };

    // The row box (design D5): the 42 px height, the swatch's left inset over
    // the rail, the 12 px text gap, the square corners, and the 1 px divider
    // under the group's last row, which the open root gave up (design D2).
    let mut row_style = format!(
        "height: {BRANCH_ROW_HEIGHT}px; box-sizing: border-box; padding: 0 {ROW_PADDING_PX}px; padding-left: {SWATCH_X}px; gap: {SWATCH_TEXT_GAP_PX}px; border: 0; border-radius: 0; text-align: left; font-size: {TREE_ROW_FONT_PX}px; cursor: pointer; background: var(--row-fill);"
    );
    row_style.push_str(&tail_divider_style(props.tail));
    let host_style = row_host_style(&fill, if props.tail { ROW_DIVIDER_PX } else { 0.0 });

    // The title's look (design D7): the rest tone and weight are the shared
    // level rule's; an open row paints the brighter heading tone at the open
    // weight. A branch carries no play state.
    let title_tone = canvas_var(if is_open { "heading" } else { "text" });
    let title_weight = if is_open {
        OPEN_TITLE_WEIGHT
    } else {
        RowLevel::Branch.title_weight(None)
    };
    let title_attrs = vec![Attribute::new(library_attr(attr, "title"), "", None, false)];
    let facts_attrs = vec![Attribute::new(
        library_attr(attr, "facts"),
        props.facts.clone(),
        None,
        false,
    )];
    let facts = props.facts.clone();
    let facts_style = format!("color: {};", canvas_var("tree-facts"));

    // The Branch swatch (design D4): the flat colour square that tells the row
    // apart from its siblings. It carries the `data-tree-swatch` hook with the
    // swatch number and loads nothing.
    let swatch = swatch_var(props.sibling);
    let swatch_mark = (swatch_index(props.sibling) + 1).to_string();

    // The hover controls (gui-row-hover-actions design D1-D3): Expand reuses
    // the row's own toggle, so a branch offers it without a second handler. A
    // row that offers no control paints neither the part nor the state mark's
    // swap.
    let on_hover_play = props.on_hover_play;
    let on_hover_queue = props.on_hover_queue;
    let on_hover_shuffle = props.on_hover_shuffle;
    let on_toggle = props.on_toggle;
    let paints = on_hover_play.is_some()
        || on_hover_queue.is_some()
        || on_hover_shuffle.is_some()
        || on_toggle.is_some();

    // The state mark (design D5): the chevron that names the row's open state,
    // in the mark tone of that state. It carries `row-hover-trail` while the
    // hover controls cover the trailing end, so it hides with the trail, and
    // it is never a press target. A branch that opens nothing paints none.
    let mark: Option<Element> = open.map(|open| {
        let (glyph, tone) = if open {
            (
                rsx! { ChevronDown { size: MARK_PX, color: canvas_var("tree-rail") } },
                canvas_var("tree-rail"),
            )
        } else {
            (
                rsx! { ChevronRight { size: MARK_PX, color: canvas_var("muted") } },
                canvas_var("muted"),
            )
        };
        rsx! {
            span {
                class: if paints { "row-hover-trail flex-none" } else { "flex-none" },
                style: "display: flex; align-items: center; pointer-events: none; color: {tone};",
                {glyph}
            }
        }
    });

    // A branch that opens toggles on a double-click; one that opens nothing
    // plays.
    let on_double_click = on_toggle.unwrap_or(props.on_play);
    let on_press = props.on_press;
    let on_context = props.on_context;
    let row_class = if open.is_some() {
        "row-hover-face relative flex w-full items-center"
    } else {
        "row-hover-face relative flex w-full items-center overflow-hidden"
    };

    // The host is the hover box (gui-row-hover-actions design D2): the row
    // button keeps its own semantics, and the controls are its later sibling,
    // never a child. The rail and the swatch paint inside the button, so the
    // button is their containing block.
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
                span {
                    class: "flex-none",
                    style: "width: {SWATCH_PX}px; height: {SWATCH_PX}px; border-radius: {SWATCH_RADIUS_PX}px; background: {swatch};",
                    "data-tree-swatch": "{swatch_mark}",
                }
                div {
                    class: "flex min-w-0 flex-1 flex-col",
                    style: "gap: 2px;",
                    Truncate {
                        text: props.title.clone(),
                        class: "min-w-0 flex-1",
                        css: format!("color: {title_tone}; font-weight: {title_weight};"),
                        style: TextStyle::sans(TREE_ROW_FONT_PX, title_weight),
                        attrs: title_attrs,
                    }
                    if !facts.is_empty() {
                        Truncate {
                            text: facts,
                            css: facts_style,
                            style: TextStyle::mono(FACTS_FONT_PX, 400.0),
                            attrs: facts_attrs,
                        }
                    }
                }
                if let Some(mark) = mark {
                    {mark}
                }
                TreeRail { tone: RailTone::Group }
            }
            if paints {
                RowHoverActions {
                    on_play: on_hover_play,
                    on_queue: on_hover_queue,
                    on_shuffle: on_hover_shuffle,
                    on_toggle,
                    open: open.unwrap_or(false),
                    row_name: Some(props.title.clone()),
                }
            }
        }
    }
}
