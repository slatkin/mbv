//! The nested row part (`gui-unified-list-rows` design D3): a tree list's
//! nested parent and leaf rows. A parent row paints the Music album look and a
//! leaf row the Music track look, so an album, a season, a track, an episode,
//! and a playlist item each look the same in every list that paints them.
//!
//! A destination names the row's level, its depth, and its data; it never names
//! a height, inset, fill, weight, tone, or guide. [`TreeLevel`] picks the look
//! and maps onto the shared [`RowLevel`] rules, [`TreeDepth`] picks the inset
//! and the guides that pass through the row, and [`TreeTrail`] picks the
//! trailing text's kind and hook part.
//!
//! A row that carries a play state derives its title's tone and weight and its
//! trail's tone from [`RowPlay`] (gui-played-row-marks design D1, D4), and
//! paints the trailing cluster — the play mark, then the trailing text — that
//! the hover controls hide together (design D3); a played parent's date paints
//! as plain text in the played tone, because a typed segment paints its own
//! kind's colour over any tone the span sets. A row that carries none keeps the
//! bare trailing text.
//!
//! The row sits in a `row-hover-host` box with the shared hover controls as its
//! later sibling (gui-row-hover-actions design D2). The box carries the row's
//! fill as `--row-fill`, so the row button and the controls that cover its
//! trailing end paint the same fill, and the trailing text carries
//! `row-hover-trail` while the controls cover it.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiHomeRowStateFrame, GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::window::components::library_attr::library_attr;
use crate::window::components::list_row::{
    LibraryTrailPart, NODE_ROW_HEIGHT, ROW_DIVIDER_PX, ROW_PADDING_PX, RowLevel, TRACK_ROW_HEIGHT,
    context_press, hover_trail_class, row_host_style,
};
use crate::window::components::play_mark::{PlayCluster, RowPlay};
use crate::window::components::row_hover_actions::RowHoverActions;
use crate::window::components::tree_guide::{
    GuideTone, LEAF_INSET_X, NESTED_INSET_X, TOP_GUIDE_X, TreeGuide, leaf_guides,
    nested_parent_guides, tail_divider_style,
};
use crate::window::components::tree_row::{LEAF_ROW_FONT_PX, TREE_ROW_FONT_PX, leaf_label_css};
use crate::window::components::typed_text::TypedText;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The gap between a nested row's parts, in logical pixels (design D3).
const NESTED_GAP_PX: f32 = 8.0;

/// The number cell's width on a leaf row, in logical pixels (design D3).
const NUMBER_CELL_PX: f32 = 18.0;

/// The trailing text's and the number cell's font size, in CSS pixels (design
/// D3).
const TRAIL_FONT_PX: f32 = 12.0;

/// The level of a nested row (design D3): the look the row paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeLevel {
    /// A nested parent row: an album or a season. It paints the Music album
    /// look.
    Parent,
    /// A leaf row: a track, an episode, or a playlist item. It paints the Music
    /// track look.
    Leaf,
}

impl TreeLevel {
    /// The shared level rule this look derives its title tone and selection
    /// shape from (design D4).
    #[must_use]
    fn row_level(self) -> RowLevel {
        match self {
            TreeLevel::Parent => RowLevel::Parent,
            TreeLevel::Leaf => RowLevel::Leaf,
        }
    }

    /// The row box's height, in logical pixels (design D3).
    #[must_use]
    fn height(self) -> f32 {
        match self {
            TreeLevel::Parent => NODE_ROW_HEIGHT,
            TreeLevel::Leaf => TRACK_ROW_HEIGHT,
        }
    }

    /// The row's font size, in CSS pixels (design D3).
    #[must_use]
    fn font_px(self) -> f32 {
        match self {
            TreeLevel::Parent => TREE_ROW_FONT_PX,
            TreeLevel::Leaf => LEAF_ROW_FONT_PX,
        }
    }
}

/// The depth of a nested row (design D3): what its inset clears and which
/// ancestor guides pass through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeDepth {
    /// A child of a top-level row: an album, a season, or a playlist item. The
    /// row insets at [`NESTED_INSET_X`] and the top-level parent's guide passes
    /// through it.
    One,
    /// A child of a nested parent: a track or an episode. The row insets at
    /// [`LEAF_INSET_X`] and both the top-level and the nested parent's guides
    /// pass through it.
    Two,
}

impl TreeDepth {
    /// The row's label inset, in pixels from the box's left edge (design D3).
    #[must_use]
    fn inset(self) -> f32 {
        match self {
            TreeDepth::One => NESTED_INSET_X,
            TreeDepth::Two => LEAF_INSET_X,
        }
    }
}

/// A nested row's trailing text (design D3): the value and the kind that names
/// its look and its hook part. An empty value paints nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeTrail {
    /// A parent's date — an album's or a season's year — painted as typed text.
    Date(String),
    /// A leaf's runtime, painted in the muted mono tone.
    Duration(String),
}

impl TreeTrail {
    /// The trailing value.
    #[must_use]
    fn text(&self) -> &str {
        match self {
            TreeTrail::Date(text) | TreeTrail::Duration(text) => text,
        }
    }

    /// The hook part the trailing text paints (design D4).
    #[must_use]
    fn part(&self) -> LibraryTrailPart {
        match self {
            TreeTrail::Date(_) => LibraryTrailPart::Date,
            TreeTrail::Duration(_) => LibraryTrailPart::Duration,
        }
    }
}

/// The props of a nested row (design D3).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct TreeRowProps {
    /// The destination's hook prefix, as in `shows` or `playlists`; an empty
    /// prefix is the Music list.
    pub attr: &'static str,
    /// The row's own kind value, as in `album`, `track`, or `season`.
    pub kind: String,
    /// The row's stable id.
    pub id: String,
    /// The row's level: the look it paints.
    pub level: TreeLevel,
    /// The row's depth: its inset and the guides that pass through it.
    pub depth: TreeDepth,
    /// The row's primary label.
    pub title: String,
    /// The leaf's number cell, `None` on a parent and on a leaf that paints no
    /// number.
    #[props(default)]
    pub number: Option<String>,
    /// The row's trailing text, `None` while the row paints none.
    #[props(default)]
    pub trail: Option<TreeTrail>,
    /// The row's play state, `None` on a list that carries none: it picks the
    /// title's tone (design D4).
    #[props(default)]
    pub play: Option<GuiHomeRowStateFrame>,
    /// Whether the selection marks the row.
    pub selected: bool,
    /// The row's open state as a tree parent, `None` on a row that opens
    /// nothing. `Some(open)` adds the row's `aria-expanded`, picks the hover
    /// toggle control's glyph, and turns the row's double-click into
    /// [`Self::on_toggle`].
    #[props(default)]
    pub open: Option<bool>,
    /// Whether the open parent shows child rows: the row then starts its own
    /// guide under its label.
    #[props(default)]
    pub shows_children: bool,
    /// Whether the row closes an open subtree: it then paints the row divider
    /// under itself, in the height the parent gave up.
    #[props(default)]
    pub tail: bool,
    /// A press on the row: it picks the row's item.
    pub on_press: Callback<()>,
    /// A double-click on the row.
    pub on_play: Callback<()>,
    /// A double-click on a parent that opens: it opens or closes the node, in
    /// place of [`Self::on_play`]. `None` while the row opens nothing. The hover
    /// controls paint their Expand/collapse control from it, so it is also that
    /// control's press.
    #[props(default)]
    pub on_toggle: Option<Callback<()>>,
    /// The hover controls' Play press: it runs the row's play path, the same one
    /// Enter runs (gui-row-hover-actions design D3, D4). `None` on a row that
    /// cannot play.
    #[props(default)]
    pub on_hover_play: Option<Callback<()>>,
    /// The hover controls' Add press: it runs the row's queue path, the same one
    /// `a` runs. `None` on a row that offers no Add.
    #[props(default)]
    pub on_hover_queue: Option<Callback<()>>,
    /// The hover controls' Shuffle press: it runs the row's shuffle path. `None`
    /// on a row that is no collection.
    #[props(default)]
    pub on_hover_shuffle: Option<Callback<()>>,
    /// A right-press on the row, with the pointer's client position; `None`
    /// while the row names no item a menu could open.
    #[props(default)]
    pub on_context: Option<Callback<(f32, f32)>>,
}

/// The nested row: a parent in the Music album look or a leaf in the Music
/// track look, inset and guided by its depth.
#[component]
pub fn TreeRow(props: TreeRowProps) -> Element {
    let attr = props.attr;
    let level = props.level;
    let row_level = level.row_level();
    let height = level.height();

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
    if let Some(open) = props.open {
        row_attrs.push(Attribute::new(
            "aria-expanded",
            if open { "true" } else { "false" },
            None,
            false,
        ));
    }

    // The row box (design D3): the level's height, the row padding over the
    // depth's inset, the tight gap, the subtree's fill, and the selection's fill
    // and corner radius. The subtree's last row takes the divider the open
    // parent gave up.
    let mut row_style = format!(
        "height: {height}px; box-sizing: border-box; padding: 0 {ROW_PADDING_PX}px; padding-left: {}px; gap: {NESTED_GAP_PX}px; border: 0; border-radius: {}px; text-align: left; font-size: {}px; cursor: pointer;",
        props.depth.inset(),
        row_level.selection_radius(props.selected),
        level.font_px()
    );
    // The row's fill (gui-row-hover-actions design D2) lives on the host as
    // `--row-fill`, so the row button and the hover controls that cover its
    // trailing end paint the same fill.
    let fill = row_level
        .selection_fill(props.selected)
        .unwrap_or_else(|| canvas_var("tree-subtree"));
    row_style.push_str(" background: var(--row-fill);");
    row_style.push_str(&tail_divider_style(props.tail));
    let host_style = row_host_style(&fill, if props.tail { ROW_DIVIDER_PX } else { 0.0 });

    // The row's play look (gui-played-row-marks design D1): one derivation
    // decides the title's tone and weight and the trail's tone. A row that
    // carries no play state keeps the level's own look.
    let row_play = props.play.as_ref().map(RowPlay::of_row);
    let title_weight = row_level.title_weight(props.play.as_ref());
    let title_css = row_level.title_tone(props.play.as_ref());
    let title_attrs = vec![Attribute::new(library_attr(attr, "title"), "", None, false)];
    let number = props.number.clone().filter(|number| !number.is_empty());
    let trail = props.trail.clone().filter(|trail| !trail.text().is_empty());

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
    let trail_class = hover_trail_class(paints_actions);

    // The trailing end (gui-played-row-marks design D3): a row that carries a
    // play state paints the one trailing cluster — the mark, then the trailing
    // text — and the cluster carries the hover swap, so the mark and the text
    // hide together while the hover controls show. A row that carries no play
    // state keeps the bare trailing text it always painted.
    let trail_tone = row_play.and_then(RowPlay::trail_tone);
    let trailing: Option<Element> = if let Some(play) = row_play {
        // The text inside the cluster never carries the trail marker — the
        // cluster does — so it paints the trail class of a row nothing covers.
        let inner_class = hover_trail_class(false);
        let text = trail.as_ref().map(|trail| match level {
            TreeLevel::Parent => parent_trail(attr, trail, props.selected, inner_class, trail_tone),
            TreeLevel::Leaf => leaf_trail(attr, trail, inner_class, trail_tone),
        });
        Some(rsx! {
            PlayCluster { class: trail_class, play: Some(play), {text} }
        })
    } else {
        trail.as_ref().map(|trail| match level {
            TreeLevel::Parent => parent_trail(attr, trail, props.selected, trail_class, trail_tone),
            TreeLevel::Leaf => leaf_trail(attr, trail, trail_class, trail_tone),
        })
    };

    // The level's own body: a parent's title over its date, or a leaf's number,
    // title, and runtime.
    let body: Element = match level {
        TreeLevel::Parent => rsx! {
            {parent_title(props.title.clone(), &title_css, title_weight, title_attrs)}
            {trailing}
        },
        TreeLevel::Leaf => rsx! {
            if let Some(number) = number {
                {number_cell(attr, &number)}
            }
            {leaf_title(props.title.clone(), &title_css, title_weight, props.play.is_some(), title_attrs)}
            {trailing}
        },
    };

    // A parent that opens toggles on a double-click; a leaf, and a parent that
    // opens nothing, plays.
    let row_class = if props.open.is_some() {
        "row-hover-face relative flex w-full items-center"
    } else {
        "row-hover-face relative flex w-full items-center overflow-hidden"
    };
    let on_double_click = props.on_toggle.unwrap_or(props.on_play);
    let on_press = props.on_press;
    let on_context = props.on_context;
    let tree_open = props.open;
    let guides = row_guides(props.depth, level, height, props.shows_children);

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
                {body}
                {guides}
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

/// A parent row's title (design D3; gui-played-row-marks design D4): the sans
/// face at the row's weight — the play state's weight on a row that carries
/// one, the parent weight otherwise — in the level's tone, measured at the
/// painted weight.
fn parent_title(title: String, tone: &str, weight: f32, attrs: Vec<Attribute>) -> Element {
    rsx! {
        Truncate {
            text: title,
            class: "min-w-0 flex-1",
            css: format!("color: {tone}; font-weight: {weight};"),
            style: TextStyle::sans(TREE_ROW_FONT_PX, weight),
            attrs,
        }
    }
}

/// A leaf row's title (design D3; gui-played-row-marks design D4): the leaf
/// face at the level's tone. A row that carries a play state paints and
/// measures its weight, which the `Truncate` box's own `font-weight` sets over
/// the leaf label's 500; a row that carries none keeps the leaf label's weight.
fn leaf_title(
    title: String,
    tone: &str,
    weight: f32,
    carried: bool,
    attrs: Vec<Attribute>,
) -> Element {
    let css = if carried {
        format!("color: {tone}; font-weight: {weight};")
    } else {
        format!("color: {tone};")
    };
    rsx! {
        div {
            class: "min-w-0 flex-1",
            style: leaf_label_css(),
            Truncate {
                text: title,
                css,
                style: TextStyle::mono(LEAF_ROW_FONT_PX, weight),
                attrs,
            }
        }
    }
}

/// A leaf row's number cell (design D3): the right-aligned ordinal before the
/// title.
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

/// A parent row's trailing text (design D3): a date in the typed text's date
/// token, at the repeat opacity unless the selection marks the row, and in the
/// muted tone while it does. The hover controls replace it while the pointer
/// rests on the row. A play state's trail tone (gui-played-row-marks design
/// D4) paints the date as plain text in that tone, because a typed segment
/// paints its own kind's colour over any tone the span sets.
fn parent_trail(
    attr: &'static str,
    trail: &TreeTrail,
    selected: bool,
    class: &'static str,
    tone: Option<&'static str>,
) -> Element {
    let attrs = vec![Attribute::new(
        library_attr(attr, trail.part().part()),
        trail.text().to_owned(),
        None,
        false,
    )];
    if let Some(tone) = tone {
        return rsx! {
            span {
                class: "{class}",
                style: "font-size: {TRAIL_FONT_PX}px; color: {tone};",
                ..attrs,
                "{trail.text()}"
            }
        };
    }
    let style = if selected {
        format!(
            "font-size: {TRAIL_FONT_PX}px; color: {};",
            canvas_var("muted")
        )
    } else {
        format!("font-size: {TRAIL_FONT_PX}px;")
    };
    let segments = vec![GuiMetaSegmentFrame {
        text: trail.text().to_owned(),
        kind: GuiMetaKindFrame::Date,
    }];
    rsx! {
        span {
            class: "{class}",
            style: "{style}",
            ..attrs,
            TypedText { segments, repeat: !selected }
        }
    }
}

/// A leaf row's trailing text (design D3): the runtime in the muted mono tone,
/// or in the play state's trail tone on a row that carries one
/// (gui-played-row-marks design D4).
fn leaf_trail(
    attr: &'static str,
    trail: &TreeTrail,
    class: &'static str,
    tone: Option<&'static str>,
) -> Element {
    let attrs = vec![Attribute::new(
        library_attr(attr, trail.part().part()),
        trail.text().to_owned(),
        None,
        false,
    )];
    let color = tone.map_or_else(|| canvas_var("muted"), str::to_owned);
    rsx! {
        span {
            class: "{class}",
            style: "font-size: {TRAIL_FONT_PX}px; color: {color};",
            ..attrs,
            "{trail.text()}"
        }
    }
}

/// The guides a nested row paints (design D3): a child of a top-level row
/// passes its parent's guide through, and a child of a nested parent passes
/// both. An open parent that shows children starts its own guide under its
/// label; a row nested under a nested parent starts none of its own, because
/// the window has no third guide position.
fn row_guides(depth: TreeDepth, level: TreeLevel, height: f32, shows_children: bool) -> Element {
    match (depth, level) {
        (TreeDepth::One, TreeLevel::Parent) => nested_parent_guides(height, shows_children),
        (TreeDepth::One, TreeLevel::Leaf) => rsx! {
            TreeGuide { x: TOP_GUIDE_X, tone: GuideTone::Top, top: 0.0, bottom: 0.0 }
        },
        (TreeDepth::Two, _) => leaf_guides(),
    }
}
