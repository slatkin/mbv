//! The Hero's Workspace table (`replace-tauri-with-blitz` task 7.3, the port
//! of `LibraryWorkspaceTable`; add-gui-library-panel design D15,
//! invariant 30): the one table every destination's Workspace paints —
//! Music's tracklist, Shows' episodes, Audiobooks' chapters, and Feeds'
//! entries. The header row, the row grid, the playing glyph, the duration cell,
//! the resume run, the hover controls, and the loading line are painted here
//! once, so a fix lands in every list.
//!
//! A destination supplies its rows, its hover actions, and its hook prefix; it
//! never restates the layout. The hover is the table's own paint: the
//! pointer-over row swaps its number for the play control and shows its actions.
//! The table keeps no cursor (`gui-pointer-target-interaction` D6): a press runs
//! the destination's press act, and a double-click on a row, like the play
//! control, plays it.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::window::components::library_attr::library_attr;
use crate::window::components::typed_text::TypedText;
use crate::window::icons::{ListEnd, Play, Plus};
use crate::window::input::f32_from_f64;
use crate::window::pointer_target::{LIST_ATTRIBUTE, PointerList, ROW_ATTRIBUTE};
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The row grid the header and every row paint (the former client
/// `grid-template-columns`).
const GRID_COLUMNS: &str = "grid-template-columns: 40px minmax(0, 1fr) 88px 64px;";

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

/// The icon a hover action paints: the
/// Lucide glyphs the destinations' actions carry. An enum, not a function
/// pointer: the props' derived `PartialEq` over fn pointers trips the
/// `unpredictable_function_pointer_comparisons` lint the workspace denies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceActionIcon {
    /// The Lucide `list-end` glyph.
    ListEnd,
    /// The Lucide `plus` glyph: add to queue.
    Plus,
}

impl WorkspaceActionIcon {
    fn paint(self, size: u16) -> Element {
        match self {
            Self::ListEnd => rsx! { ListEnd { size: size } },
            Self::Plus => rsx! { Plus { size: size } },
        }
    }
}

/// One hover action of a row.
#[derive(Clone, PartialEq)]
pub struct WorkspaceAction {
    /// The action's hook value and its accessible name's source.
    pub key: String,
    /// The action's accessible name.
    pub label: String,
    /// The action's icon.
    pub icon: WorkspaceActionIcon,
    /// Run the action on the row's id.
    pub run: Callback<String>,
}

impl std::fmt::Debug for WorkspaceAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceAction")
            .field("key", &self.key)
            .field("label", &self.label)
            .field("icon", &self.icon)
            .finish_non_exhaustive()
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
    /// The rows' hover actions.
    #[props(default)]
    pub actions: Vec<WorkspaceAction>,
    /// A press on a row: the destination's press act, or a callback that does
    /// nothing on a Workspace that keeps no cursor.
    pub on_press: Callback<String>,
    /// Play a row: the number cell's play control and the row's double-click.
    pub on_play: Callback<String>,
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
            .field("actions", &self.actions)
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

/// The play glyph the row's play control and the playing row paint: twelve
/// pixels, filled in the accent.
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

/// The table facts one row's paint borrows: the destination's hooks, the hover
/// state, the actions, and the press and play callbacks.
struct RowPaint<'a> {
    hook: &'static str,
    noun: &'static str,
    hovered: Signal<Option<String>>,
    actions: &'a [WorkspaceAction],
    on_press: Callback<String>,
    on_play: Callback<String>,
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

/// The hover swap's display styles, as (number span,
/// play control): the two trade places while the pointer is over the row.
/// The control is `display: none` off-hover — a missing style would leave it
/// painted beside the number instead of swapping for it (the port review's
/// hidden-control finding, which the hover-swap regression test guards).
fn hover_styles(is_hovered: bool) -> (&'static str, &'static str) {
    if is_hovered {
        ("display: none;", "display: flex;")
    } else {
        ("", "display: none;")
    }
}

/// The number cell: the ordinal that swaps for the
/// play control while the pointer is over the row.
fn paint_number_cell(
    hook: &'static str,
    row: &WorkspaceRow,
    is_hovered: bool,
    on_play: Callback<String>,
) -> Element {
    let (number_style, hover_display) = hover_styles(is_hovered);
    let number_attrs = vec![Attribute::new(
        library_attr(hook, "number"),
        "",
        None,
        false,
    )];
    let play_attrs = vec![Attribute::new(
        library_attr(hook, "action"),
        "play".to_owned(),
        None,
        false,
    )];
    let play_label = format!("Play {}", row.name);
    let row_id = row.id.clone();
    rsx! {
        span {
            class: "flex-none font-mono tabular-nums",
            style: "font-size: 11px; color: var(--canvas-ordinal);",
            span {
                style: "{number_style}",
                ..number_attrs,
                "{row.number}"
            }
            button {
                "type": "button",
                class: "cursor-pointer items-center hover:drop-shadow-[0_0_4px_var(--canvas-accent)]",
                style: "{hover_display}",
                "aria-label": "{play_label}",
                onclick: move |_| on_play.call(row_id.clone()),
                ..play_attrs,
                { play_glyph() }
            }
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

/// One hover action's button.
fn paint_action(hook: &'static str, action: &WorkspaceAction, row_id: &str) -> Element {
    let own_attrs = vec![Attribute::new(
        library_attr(hook, "action"),
        action.key.clone(),
        None,
        false,
    )];
    let icon = action.icon;
    let label = action.label.clone();
    let run = action.run;
    let action_row_id = row_id.to_owned();
    rsx! {
        button {
            "type": "button",
            class: "flex items-center justify-center",
            style: "width: 28px; height: 28px; background: none; border: 0; border-radius: 4px; color: var(--canvas-text); cursor: pointer;",
            "aria-label": "{label}",
            onclick: move |_| run.call(action_row_id.clone()),
            ..own_attrs,
            { icon.paint(14) }
        }
    }
}

/// The actions cell: the row's hover actions,
/// shown only while the pointer is over the row.
fn paint_actions_cell(
    hook: &'static str,
    row_id: &str,
    is_hovered: bool,
    actions: &[WorkspaceAction],
) -> Element {
    let region_attrs = vec![Attribute::new(
        library_attr(hook, "actions"),
        "",
        None,
        false,
    )];
    let display = if is_hovered { "flex" } else { "none" };
    rsx! {
        span {
            class: "flex justify-end",
            style: "gap: 2px;",
            span {
                class: "justify-end",
                style: "gap: 2px; display: {display};",
                ..region_attrs,
                for action in actions.iter() {
                    { paint_action(hook, action, row_id) }
                }
            }
        }
    }
}

/// The duration cell: the time token at
/// the repeat opacity.
fn paint_duration_cell(hook: &'static str, row: &WorkspaceRow) -> Element {
    let own_attrs = vec![Attribute::new(
        library_attr(hook, "duration"),
        "",
        None,
        false,
    )];
    let segments = vec![GuiMetaSegmentFrame {
        text: row.duration.clone(),
        kind: GuiMetaKindFrame::Time,
    }];
    rsx! {
        span {
            class: "font-mono tabular-nums",
            style: "font-size: 12px; text-align: right;",
            ..own_attrs,
            TypedText {
                segments: segments,
                repeat: true,
            }
        }
    }
}

/// One row of the table: the cells over the row
/// grid, with the hover swap and the resume run.
fn paint_row(row: &WorkspaceRow, paint: &RowPaint<'_>) -> Element {
    let RowPaint {
        hook,
        noun,
        hovered,
        actions,
        on_press,
        on_play,
        on_context,
    } = paint;
    let progress_attrs = vec![Attribute::new(
        library_attr(hook, "progress"),
        "",
        None,
        false,
    )];
    let is_hovered = hovered.cloned().as_deref() == Some(row.id.as_str());
    let row_style = format!("{GRID_COLUMNS} height: 34px; padding: 0 8px; border-radius: 4px;");
    let row_id = row.id.clone();
    let enter_id = row_id.clone();
    let context_id = row_id;
    let mut enter_hovered = *hovered;
    let mut leave_hovered = *hovered;
    let context_on_context = *on_context;
    let row_play = *on_play;

    rsx! {
        li {
            key: "{row.id}",
            class: "relative grid items-center",
            style: "{row_style}",
            onpointerenter: move |_| enter_hovered.set(Some(enter_id.clone())),
            onpointerleave: move |_| leave_hovered.set(None),
            oncontextmenu: move |event: MouseEvent| {
                if let Some(on_context) = context_on_context {
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
            { paint_number_cell(hook, row, is_hovered, *on_play) }
            { paint_name_button(hook, row, noun, *on_press, row_play) }
            { paint_actions_cell(hook, &row.id, is_hovered, actions) }
            { paint_duration_cell(hook, row) }
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
    // The row the pointer is over: the table's own paint, never shared
    // state.
    let hovered: Signal<Option<String>> = use_signal(|| None);
    let paint = RowPaint {
        hook: props.hook,
        noun: props.noun,
        hovered,
        actions: &props.actions,
        on_press: props.on_press,
        on_play: props.on_play,
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
                    span {}
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
