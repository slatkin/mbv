//! The Hero's Workspace table (`replace-tauri-with-blitz` task 7.3, the port
//! of `LibraryWorkspaceTable`; add-gui-library-panel design D15,
//! invariant 30): the one table every destination's Workspace paints —
//! Music's tracklist, Shows' episodes, Audiobooks' chapters, and Feeds'
//! entries. The header row, the row grid, the playing mark, the trailing cell,
//! the resume run, the hover controls, and the loading line are painted here
//! once, so a fix lands in every list.
//!
//! A destination supplies its rows, its Add path, and its hook prefix; it never
//! restates the layout. A row's hover controls (gui-row-hover-actions design
//! D1, D2, D5) take no window hover state: the `li` is the `row-hover-host`,
//! and the stylesheet reveals the part over the trailing cell, which rests as
//! the row's duration and hides while the part shows. Play comes from the
//! table's `on_play`, and Add from `on_queue` when the destination has one; no
//! table row offers Shuffle or Expand. The table keeps no cursor
//! (`gui-pointer-target-interaction` D6): a press runs the destination's press
//! act, and a double-click or the Play control plays the row.
//!
//! A row that carries a play state derives its title's tone and weight from
//! [`RowPlay`] (gui-played-row-marks design D1, D4), and its trailing cell
//! paints the shared cluster — the play mark, then the duration — that the
//! hover controls hide together (design D3). The playing row keeps its own
//! heading tone and weight 600, paints the bars mark in its number cell, and
//! paints the mark's empty slot in its trailing cell, so its duration
//! stays aligned with the other rows. A row that carries no play state keeps
//! the bare duration it always painted.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiHomeRowStateFrame, GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::window::components::library_attr::library_attr;
use crate::window::components::list_row::row_host_style;
use crate::window::components::play_mark::{PlayCluster, RowPlay};
use crate::window::components::row_hover_actions::RowHoverActions;
use crate::window::components::typed_text::TypedText;
use crate::window::icons::PlayingBars;
use crate::window::input::f32_from_f64;
use crate::window::library::episode::{episode_progress, is_played_episode};
use crate::window::pointer_target::{LIST_ATTRIBUTE, PointerList, ROW_ATTRIBUTE};
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The row grid the header and every row paint (the former client
/// `grid-template-columns`): the number, the title, and the trailing cell that
/// holds the duration and the hover controls.
const GRID_COLUMNS: &str = "grid-template-columns: 40px minmax(0, 1fr) 88px;";

/// One row of the table, as the destination maps its rows (the former client
/// `WorkspaceRow` interface).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceRow {
    /// The row's stable identity, the value the destination's commands name.
    pub id: String,
    /// The row's number cell.
    pub number: String,
    /// The row's title.
    pub name: String,
    /// The row's duration cell.
    pub duration: String,
    /// Whether the row is the one playing at the playhead.
    pub playing: bool,
    /// The row's play state, when the destination carries one: the frame the
    /// list row parts carry, and the one derivation of the row's title tone and
    /// its resume run (`gui-shared-workspace-row-state` design D2).
    pub state: Option<GuiHomeRowStateFrame>,
}

/// The loading line's facts: the line's text and
/// the destination's own attribute for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLoading {
    /// The loading line's text.
    pub text: String,
    /// The destination's own attribute for the line, as in
    /// `data-library-shows-episodes-loading`.
    pub attr: &'static str,
}

/// The props of the table (the `LibraryWorkspaceTable` props, in
/// order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryWorkspaceTableProps {
    /// The section's own attribute: the hook the destination's tests take.
    pub root: &'static str,
    /// The section's accessible name.
    pub label: &'static str,
    /// The destination's row hook, as in `track`, `shows-episode`,
    /// `abs-chapter`: every part paints `data-library-<hook>-<part>`.
    pub hook: &'static str,
    /// The word that names a row's press target: `Track`, `Episode`,
    /// `Chapter`.
    pub noun: &'static str,
    /// The pointer list the table names (`gui-pointer-target-interaction`
    /// D1): `tracklist` for the Music Hero, and `workspace` for a Hero whose
    /// rows are the Shows episodes, the Audiobooks chapters, or the Feeds
    /// entries.
    pub pointer_list: PointerList,
    /// The rows in painted order.
    pub rows: Vec<WorkspaceRow>,
    /// A press on a row: the destination's press act, or a callback that does
    /// nothing on a Workspace that keeps no cursor.
    pub on_press: Callback<String>,
    /// Play a row: the row's Play control and the row's double-click.
    pub on_play: Callback<String>,
    /// Add a row to the queue: the row's Add control, painted only when the
    /// destination has an Add path (design D5).
    #[props(default)]
    pub on_queue: Option<Callback<String>>,
    /// A right-click on a row, with the pointer's client position and the
    /// row's id, when the destination has a row menu.
    #[props(default)]
    pub on_context: Option<Callback<(f32, f32, String)>>,
    /// The loading line, while the rows have not landed.
    #[props(default)]
    pub loading: Option<WorkspaceLoading>,
}

impl std::fmt::Debug for LibraryWorkspaceTableProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryWorkspaceTableProps")
            .field("root", &self.root)
            .field("label", &self.label)
            .field("hook", &self.hook)
            .field("noun", &self.noun)
            .field("pointer_list", &self.pointer_list)
            .field("rows", &self.rows)
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}

/// A row's title tone (`gui-shared-workspace-row-state` design D1, D2;
/// gui-played-row-marks design D1, D4): the playing row's own heading tone —
/// the playing mark in the number cell, which no carried state changes —
/// otherwise the tone [`RowPlay`] derives from the row's carried state, or the
/// text tone on a row that carries none.
fn title_tone(row: &WorkspaceRow) -> String {
    if row.playing {
        return canvas_var("heading");
    }
    match row.state.as_ref() {
        Some(state) => RowPlay::of_row(state).title_tone().to_owned(),
        None => canvas_var("text"),
    }
}

/// A row's title weight (gui-played-row-marks design D1, D4; gui-hero-design-polish
/// design D6): the playing row's 600, otherwise the weight [`RowPlay`] derives
/// from the row's carried state; a row that carries no state keeps 400.
fn title_weight(row: &WorkspaceRow) -> f32 {
    if row.playing {
        return 600.0;
    }
    match row.state.as_ref() {
        Some(state) => RowPlay::of_row(state).title_weight(),
        None => 400.0,
    }
}

/// The value a row's state hook paints (`gui-shared-workspace-row-state` design
/// D3): `played` for a played row, `active` for a partly played row that
/// carries a resume percent, and `ordinary` for every other row — including a
/// partly played row with no percent, the unknown-runtime case.
fn state_wire(state: &GuiHomeRowStateFrame) -> &'static str {
    if is_played_episode(state) {
        return "played";
    }
    if episode_progress(state).is_some() {
        return "active";
    }
    "ordinary"
}

/// The row's resume run percent (`gui-shared-workspace-row-state` design D2):
/// the bounded percent the row's play state carries, when it carries one.
fn resume_progress(row: &WorkspaceRow) -> Option<f32> {
    row.state.as_ref().and_then(episode_progress).map(f32::from)
}

/// The playing mark the playing row paints in its number cell: twelve pixels,
/// in the accent (gui-hero-design-polish design D6).
fn playing_mark() -> Element {
    rsx! {
        PlayingBars {
            size: 12,
            color: canvas_var("accent"),
        }
    }
}

/// A row's fill (gui-hero-design-polish design D6): the playing row's
/// `--canvas-playing-row`, the striped rows' lighter panel on every second row
/// from the second, and the column's own fill on the rest. The fill lives on
/// the host as `--row-fill`, so the hover controls' cover never goes
/// transparent over the title.
fn row_fill(row: &WorkspaceRow, index: usize) -> String {
    if row.playing {
        return canvas_var("playing-row");
    }
    if index.is_multiple_of(2) {
        canvas_var("library-column")
    } else {
        canvas_var("library-panel")
    }
}

/// The loading line, while the rows have not landed.
fn paint_loading(loading: &WorkspaceLoading) -> Element {
    let attrs = vec![Attribute::new(loading.attr, "", None, false)];
    rsx! {
        span {
            class: "font-mono",
            style: "font-size: 13px; color: var(--canvas-muted);",
            ..attrs,
            "{loading.text}"
        }
    }
}

/// The table facts one row's paint borrows: the destination's hooks and its
/// press, play, queue, and context callbacks.
#[derive(Clone, Copy)]
struct RowPaint {
    hook: &'static str,
    noun: &'static str,
    on_press: Callback<String>,
    on_play: Callback<String>,
    on_queue: Option<Callback<String>>,
    on_context: Option<Callback<(f32, f32, String)>>,
}

/// One row's own hooks: the id always paints — and, with it, the row's pointer
/// marker — the playing and state marks only while they hold.
fn row_attrs(row: &WorkspaceRow, hook: &str) -> Vec<Attribute> {
    let mut attrs = vec![
        Attribute::new(library_attr(hook, ""), row.id.clone(), None, false),
        // The row marker (`gui-pointer-target-interaction` D1): the row's own
        // id, the value the destination's commands name.
        Attribute::new(ROW_ATTRIBUTE, row.id.clone(), None, false),
    ];
    if row.playing {
        attrs.push(Attribute::new(
            library_attr(hook, "playing"),
            "",
            None,
            false,
        ));
    }
    if let Some(state) = row.state.as_ref() {
        attrs.push(Attribute::new(
            library_attr(hook, "state"),
            state_wire(state),
            None,
            false,
        ));
    }
    attrs
}

/// The number cell: the row's ordinal, always painted. The hover controls sit
/// in the trailing cell, so the number never swaps (design D5).
fn paint_number_cell(hook: &'static str, row: &WorkspaceRow) -> Element {
    let number_attrs = vec![Attribute::new(
        library_attr(hook, "number"),
        "",
        None,
        false,
    )];
    // The playing row's mark stands in for the number (gui-hero-design-polish
    // design D6): the cell still paints, with the mark as its content.
    let content: Element = if row.playing {
        playing_mark()
    } else {
        rsx! {
            "{row.number}"
        }
    };
    rsx! {
        span {
            class: "row-hover-face flex flex-none items-center font-mono tabular-nums",
            style: "font-size: 11px; color: var(--canvas-ordinal);",
            ..number_attrs,
            {content}
        }
    }
}

/// The name button: the row's press target, with
/// the playing row's glyph beside its title.
fn paint_name_button(
    hook: &'static str,
    row: &WorkspaceRow,
    noun: &str,
    on_press: Callback<String>,
    on_play: Callback<String>,
) -> Element {
    let name_attrs = vec![Attribute::new(library_attr(hook, "name"), "", None, false)];
    let label = format!("{noun} {}", row.name);
    let row_id = row.id.clone();
    let double_click_id = row.id.clone();
    let color = title_tone(row);
    let weight = title_weight(row);
    rsx! {
        button {
            "type": "button",
            class: "row-hover-face flex w-full min-w-0 items-center",
            style: "background: none; border: 0; padding: 0; text-align: left; cursor: pointer;",
            "aria-label": "{label}",
            onclick: move |_| on_press.call(row_id.clone()),
            ondoubleclick: move |_| on_play.call(double_click_id.clone()),
            ..name_attrs,
            span {
                class: "flex min-w-0 flex-1 items-center",
                Truncate {
                    text: row.name.clone(),
                    class: "min-w-0 flex-1".to_owned(),
                    css: format!("font-size: 13.5px; color: {color}; font-weight: {weight};"),
                    style: TextStyle::sans(13.5, weight),
                }
            }
        }
    }
}

/// The trailing cell (design D5; gui-played-row-marks design D3, D4): the
/// controls themselves, and over the cell the `row-hover-trail` the stylesheet
/// hides while they show. A row that carries a play state paints the one
/// trailing cluster — the play mark, then the duration — and the cluster
/// carries the hover swap, so the mark and the text hide together. The playing
/// row paints the mark's empty slot, so its duration stays aligned with the
/// other rows. A played row's duration is a plain span in the played trail
/// tone, because `TypedText` paints each segment in its kind's colour and
/// cannot dim; an unplayed row keeps `TypedText`. A row that carries no play
/// state keeps the bare duration at the repeat opacity. Play always paints,
/// and Add paints only where the destination passes `on_queue`.
fn paint_trailing_cell(
    hook: &'static str,
    row: &WorkspaceRow,
    on_play: Callback<String>,
    on_queue: Option<Callback<String>>,
) -> Element {
    let duration_attrs = vec![Attribute::new(
        library_attr(hook, "duration"),
        "",
        None,
        false,
    )];
    let segments = vec![GuiMetaSegmentFrame {
        text: row.duration.clone(),
        kind: GuiMetaKindFrame::Time,
    }];
    let play_id = row.id.clone();
    let on_hover_play = Callback::new(move |()| on_play.call(play_id.clone()));
    let on_hover_queue = on_queue.map(|queue| {
        let queue_id = row.id.clone();
        Callback::new(move |()| queue.call(queue_id.clone()))
    });
    // The trailing end (gui-played-row-marks design D3): the cluster carries
    // the hover swap in place of the text span, and the text keeps its hook,
    // its mono face, and its size.
    let trailing: Element = if let Some(state) = row.state.as_ref() {
        let row_play = RowPlay::of_row(state);
        let mark = if row.playing { None } else { Some(row_play) };
        let tone = row_play
            .trail_tone()
            .map_or_else(String::new, |tone| format!(" color: {tone};"));
        let duration_style = format!("font-size: 12px; text-align: right;{tone}");
        let duration_text: Element = match row_play {
            RowPlay::Played => rsx! {
                "{row.duration}"
            },
            RowPlay::Unplayed => rsx! {
                TypedText {
                    segments: segments,
                    repeat: true,
                }
            },
        };
        rsx! {
            PlayCluster { class: "row-hover-trail", play: mark,
                span {
                    class: "flex-none font-mono tabular-nums",
                    style: "{duration_style}",
                    ..duration_attrs,
                    {duration_text}
                }
            }
        }
    } else {
        rsx! {
            span {
                class: "row-hover-trail font-mono tabular-nums",
                style: "font-size: 12px; text-align: right;",
                ..duration_attrs,
                TypedText {
                    segments: segments,
                    repeat: true,
                }
            }
        }
    };
    rsx! {
        span {
            class: "flex items-center justify-end",
            {trailing}
            RowHoverActions {
                on_play: Some(on_hover_play),
                on_queue: on_hover_queue,
                row_name: Some(row.name.clone()),
            }
        }
    }
}

/// One row of the table: the cells over the row grid, the hover host, and the
/// resume run.
fn paint_row(row: &WorkspaceRow, index: usize, paint: &RowPaint) -> Element {
    let RowPaint {
        hook,
        noun,
        on_press,
        on_play,
        on_queue,
        on_context,
    } = *paint;
    let progress_attrs = vec![Attribute::new(
        library_attr(hook, "progress"),
        "",
        None,
        false,
    )];
    let fill = row_fill(row, index);
    let host_style = row_host_style(&fill, 0.0);
    let row_style = format!(
        "{GRID_COLUMNS} height: 34px; padding: 0 8px; border-radius: 4px; overflow: hidden; background: var(--row-fill); {host_style}"
    );
    let context_id = row.id.clone();
    let resume = resume_progress(row);

    rsx! {
        li {
            key: "{row.id}",
            class: "row-hover-host row-hover-row grid items-center",
            style: "{row_style}",
            oncontextmenu: move |event: MouseEvent| {
                if let Some(on_context) = on_context {
                    event.prevent_default();
                    let coordinates = event.client_coordinates();
                    on_context.call((
                        f32_from_f64(coordinates.x),
                        f32_from_f64(coordinates.y),
                        context_id.clone(),
                    ));
                }
            },
            ..row_attrs(row, hook),
            { paint_number_cell(hook, row) }
            { paint_name_button(hook, row, noun, on_press, on_play) }
            { paint_trailing_cell(hook, row, on_play, on_queue) }
            if let Some(progress) = resume {
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

/// The Hero's Workspace table.
#[component]
pub fn LibraryWorkspaceTable(props: LibraryWorkspaceTableProps) -> Element {
    let paint = RowPaint {
        hook: props.hook,
        noun: props.noun,
        on_press: props.on_press,
        on_play: props.on_play,
        on_queue: props.on_queue,
        on_context: props.on_context,
    };
    let root_attrs = vec![
        Attribute::new(props.root, "", None, false),
        // The list marker (`gui-pointer-target-interaction` D1): the table
        // names the list its rows belong to, so a key that acts on a hovered
        // row knows whether it is the tracklist or a Workspace.
        Attribute::new(LIST_ATTRIBUTE, props.pointer_list.token(), None, false),
    ];

    rsx! {
        section {
            class: "flex flex-col",
            "aria-label": "{props.label}",
            ..root_attrs,
            if let Some(loading) = props.loading.as_ref() {
                { paint_loading(loading) }
            } else {
                div {
                    class: "grid items-center font-mono",
                    style: "{GRID_COLUMNS} height: 32px; padding: 0 8px; border-bottom: 1px solid var(--canvas-border); font-size: 12px; letter-spacing: 0.08em; color: var(--canvas-type-label);",
                    span { "#" }
                    span { "TITLE" }
                    span { style: "text-align: right;", "TIME" }
                }
                ol {
                    class: "flex flex-col",
                    style: "margin: 4px 0 0; padding: 0; list-style: none;",
                    for (index, row) in props.rows.iter().enumerate() {
                        { paint_row(row, index, &paint) }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
