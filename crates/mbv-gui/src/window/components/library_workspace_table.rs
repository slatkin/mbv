//! The Hero's Workspace table (`replace-tauri-with-blitz` task 7.3, the port
//! of `LibraryWorkspaceTable`; add-gui-library-panel design D15,
//! invariant 30): the one table every destination's Workspace paints —
//! Music's tracklist, Shows' episodes, Audiobooks' chapters, and Feeds'
//! entries. The header row, the row grid, the playing glyph, the trailing cell,
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

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::window::components::library_attr::library_attr;
use crate::window::components::row_hover_actions::RowHoverActions;
use crate::window::components::typed_text::TypedText;
use crate::window::icons::Play;
use crate::window::input::f32_from_f64;
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
    /// The row's played state, when the destination carries one.
    pub state: Option<WorkspaceRowState>,
    /// The resume position percent, when the row is partly played.
    pub progress: Option<f32>,
}

/// A row's played state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRowState {
    /// The row is fully played.
    Played,
    /// The row is partly played.
    Active,
    /// The row carries no played state the destination marks.
    Ordinary,
}

impl WorkspaceRowState {
    /// The value the row's state hook paints.
    #[must_use]
    pub fn wire(self) -> &'static str {
        match self {
            Self::Played => "played",
            Self::Active => "active",
            Self::Ordinary => "ordinary",
        }
    }
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

/// A row's title colour: the playing row's heading tone, the muted tone once
/// played, else the text tone.
fn name_color(row: &WorkspaceRow) -> &'static str {
    if row.playing {
        return "var(--canvas-heading)";
    }
    match row.state {
        Some(WorkspaceRowState::Played) => "var(--canvas-muted)",
        _ => "var(--canvas-text)",
    }
}

/// The play glyph the playing row paints beside its title: twelve pixels,
/// filled in the accent.
fn play_glyph() -> Element {
    rsx! {
        Play {
            size: 12,
            fill: canvas_var("accent"),
            color: canvas_var("accent"),
        }
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
    if let Some(state) = row.state {
        attrs.push(Attribute::new(
            library_attr(hook, "state"),
            state.wire(),
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
    rsx! {
        span {
            class: "flex-none font-mono tabular-nums",
            style: "font-size: 11px; color: var(--canvas-ordinal);",
            ..number_attrs,
            "{row.number}"
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
    let glyph_attrs = vec![Attribute::new(library_attr(hook, "glyph"), "", None, false)];
    let label = format!("{noun} {}", row.name);
    let row_id = row.id.clone();
    let double_click_id = row.id.clone();
    let color = name_color(row);
    rsx! {
        button {
            "type": "button",
            class: "flex w-full min-w-0 items-center",
            style: "background: none; border: 0; padding: 0; text-align: left; cursor: pointer;",
            "aria-label": "{label}",
            onclick: move |_| on_press.call(row_id.clone()),
            ondoubleclick: move |_| on_play.call(double_click_id.clone()),
            ..name_attrs,
            span {
                class: "flex min-w-0 flex-1 items-center",
                style: "gap: 6px;",
                if row.playing {
                    // The playing row's play glyph sits beside its title: a
                    // highlight marks focus only.
                    span {
                        ..glyph_attrs,
                        { play_glyph() }
                    }
                }
                Truncate {
                    text: row.name.clone(),
                    class: "min-w-0 flex-1".to_owned(),
                    css: format!("font-size: 13.5px; color: {color};"),
                    style: TextStyle::sans(13.5, 400.0),
                }
            }
        }
    }
}

/// The trailing cell (design D5): the row's duration at the repeat opacity,
/// which rests as the `row-hover-trail` the stylesheet hides while the hover
/// controls show, and the controls themselves over it. Play always paints, and
/// Add paints only where the destination passes `on_queue`.
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
    rsx! {
        span {
            class: "flex items-center justify-end",
            span {
                class: "row-hover-trail font-mono tabular-nums",
                style: "font-size: 12px; text-align: right;",
                ..duration_attrs,
                TypedText {
                    segments: segments,
                    repeat: true,
                }
            }
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
fn paint_row(row: &WorkspaceRow, paint: &RowPaint) -> Element {
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
    let row_style = format!("{GRID_COLUMNS} height: 34px; padding: 0 8px; border-radius: 4px;");
    let context_id = row.id.clone();

    rsx! {
        li {
            key: "{row.id}",
            class: "row-hover-host grid items-center",
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
            if let Some(progress) = row.progress {
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
                    for row in props.rows.iter() {
                        { paint_row(row, &paint) }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
