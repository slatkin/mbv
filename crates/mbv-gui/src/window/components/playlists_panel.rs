//! The Playlists screen: its own main-UI screen in the Library panel's slot,
//! with the Queue to its right. The saved playlists paint as a tree — a
//! playlist expands to its items, one open playlist at a time, as the core
//! holds one — and the hero beside it (above it in a narrow column) is the
//! selected row's control set. A right-click on a row opens the same actions
//! as a menu.
//!
//! A playlist row is a tree parent (`gui-tree-double-click` design D1 to D3):
//! a press picks it, a double-click opens it when closed and closes it when
//! open, its count gives way to the hover hint, and it paints no chevron. An
//! item row stays a leaf whose double-click plays it, and the playlist's own
//! Play control and menu Play entry still play the playlist.
//!
//! The tree paints the shared indent guide (`gui-tree-indent-guides`): an open
//! playlist that shows item rows hangs one under its label, and its item rows
//! and its rename field carry it on to the subtree's last row.
//!
//! The core owns the data (the `playlists` section); the selection, the
//! right-click menu, and the rename draft are this screen's own window state.
//! The rename draft lives in the shared [`SettingsState`] beside the other
//! text fields, so the router's one text-field step covers its keys.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiRowMenuEntry;
use crate::bridge::frame::playlists::{GuiOpenPlaylist, GuiPlaylistRow, GuiPlaylistsFrame};
use crate::bridge::playlists::{GuiPlaylistAction, GuiPlaylistsCommand};
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::components::library_hero_actions::{
    ACCENT_STYLE, DESTRUCTIVE_STYLE, RAISED_STYLE,
};
use crate::window::components::panel_divider::{DividerName, DividerSide, PanelDivider};
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::components::settings_button::{SettingsButton, SettingsButtonVariant};
use crate::window::components::tree_guide::{
    GuideTone, PLAYLIST_GUIDE_X, PLAYLIST_LABEL_X, TreeGuide, parent_guide_start,
};
use crate::window::components::tree_hint::TreeHint;
use crate::window::components::tree_row::{TREE_ROW_FONT_PX, tree_row_text};
use crate::window::input::f32_from_f64;
use crate::window::library::state::LibraryState;
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::Sections;
use crate::window::settings::focus::focus_on_mount;
use crate::window::settings::state::{SettingsForm, SettingsState};
use crate::window::truncate::Truncate;

/// The row the hero shows and a menu addresses: a playlist, or one item of the
/// open playlist. Stable ids, never positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pick {
    Playlist(String),
    Item { playlist: String, item: String },
}

/// One control, in the hero's and the menu's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Play,
    Shuffle,
    Add,
    Rename,
    Refresh,
    Delete,
}

impl Control {
    /// The playlist's controls.
    const PLAYLIST: [Self; 6] = [
        Self::Play,
        Self::Shuffle,
        Self::Add,
        Self::Rename,
        Self::Refresh,
        Self::Delete,
    ];

    /// An item's controls: the three whole-row actions.
    const ITEM: [Self; 3] = [Self::Play, Self::Shuffle, Self::Add];

    fn label(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Shuffle => "Shuffle",
            Self::Add => "Add to queue",
            Self::Rename => "Rename",
            Self::Refresh => "Refresh",
            Self::Delete => "Delete",
        }
    }

    /// The playlist action the control sends, `None` for a control that is not
    /// one of the three whole-row actions.
    fn action(self) -> Option<GuiPlaylistAction> {
        match self {
            Self::Play => Some(GuiPlaylistAction::Play),
            Self::Shuffle => Some(GuiPlaylistAction::Shuffle),
            Self::Add => Some(GuiPlaylistAction::Enqueue),
            Self::Rename | Self::Refresh | Self::Delete => None,
        }
    }
}

/// The right-click menu: the row it names and where it opened.
#[derive(Debug, Clone, PartialEq)]
struct RowMenu {
    pick: Pick,
    x: f32,
    y: f32,
}

/// The controls the row `pick` names offers.
pub(crate) fn controls_for(pick: &Pick) -> &'static [Control] {
    match pick {
        Pick::Playlist(_) => &Control::PLAYLIST,
        Pick::Item { .. } => &Control::ITEM,
    }
}

/// The command one control sends for the row `pick` names. `None` for the
/// Rename control, which opens a local draft instead of sending.
pub(crate) fn command_for(pick: &Pick, control: Control) -> Option<GuiPlaylistsCommand> {
    if let Some(action) = control.action() {
        return Some(match pick {
            Pick::Playlist(id) => GuiPlaylistsCommand::Action {
                id: id.clone(),
                action,
            },
            Pick::Item { playlist, item } => GuiPlaylistsCommand::ItemAction {
                id: playlist.clone(),
                item_id: item.clone(),
                action,
            },
        });
    }
    let Pick::Playlist(id) = pick else {
        return None;
    };
    match control {
        Control::Refresh => Some(GuiPlaylistsCommand::Refresh {
            id: Some(id.clone()),
        }),
        Control::Delete => Some(GuiPlaylistsCommand::Delete { id: id.clone() }),
        Control::Play | Control::Shuffle | Control::Add | Control::Rename => None,
    }
}

/// Run one control on the row `pick` names.
fn run_control(
    link: &CoreLink,
    settings: &SettingsState,
    frame: &GuiPlaylistsFrame,
    pick: &Pick,
    control: Control,
) {
    if control == Control::Rename {
        if let Pick::Playlist(id) = pick
            && let Some((name, _)) = pick_title(frame, pick)
        {
            settings.open_rename_draft(id, &name);
        }
        return;
    }
    if let Some(command) = command_for(pick, control) {
        link.playlists(command);
    }
}

/// The command a double-click on the playlist `id` sends: it opens the
/// playlist while it is closed and closes the one the core holds open.
fn toggle_command(is_open: bool, id: &str) -> GuiPlaylistsCommand {
    if is_open {
        GuiPlaylistsCommand::Collapse
    } else {
        GuiPlaylistsCommand::Open { id: id.to_owned() }
    }
}

/// The name and detail line the hero titles itself with, `None` when the
/// pick names a row the frame no longer holds.
pub(crate) fn pick_title(frame: &GuiPlaylistsFrame, pick: &Pick) -> Option<(String, String)> {
    match pick {
        Pick::Playlist(id) => frame
            .playlists
            .iter()
            .find(|playlist| &playlist.id == id)
            .map(|playlist| {
                (
                    playlist.name.clone(),
                    format!("{} items", playlist.item_count),
                )
            }),
        Pick::Item { playlist, item } => {
            let open = frame.open.as_ref().filter(|open| &open.id == playlist)?;
            let row = open.items.iter().find(|row| &row.id == item)?;
            Some((row.name.clone(), format!("In {}", open.name)))
        }
    }
}

/// The props of the screen.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct PlaylistsPanelProps {
    /// The arrangement picked from the Library column width: Wide puts the
    /// hero beside the tree, every other one above it.
    pub arrangement: GuiArrangement,
    /// The tree column's painted width in Wide.
    pub list_width: f32,
    /// The column the tree sits in: the divider drag's clamp.
    pub library_column: f32,
}

/// The Playlists screen.
#[component]
pub fn PlaylistsPanel(props: PlaylistsPanelProps) -> Element {
    let sections = consume_context::<Sections>();
    let library = consume_context::<LibraryState>();
    let settings = consume_context::<SettingsState>();
    let widths_store = consume_context::<PanelWidthsStore>();
    let mut drag_widths = widths_store.clone();
    let mut reset_widths = widths_store;
    let pick: Signal<Option<Pick>> = use_signal(|| None);
    let mut menu: Signal<Option<RowMenu>> = use_signal(|| None);

    let Some(frame) = sections.playlists.cloned() else {
        return rsx! {};
    };
    let link = library.link.clone();
    let wide = props.arrangement == GuiArrangement::Wide;
    // The first playlist is picked until the user picks a row.
    let picked = pick
        .cloned()
        .filter(|pick| pick_title(&frame, pick).is_some())
        .or_else(|| {
            frame
                .playlists
                .first()
                .map(|playlist| Pick::Playlist(playlist.id.clone()))
        });

    let tree = rsx! {
        div {
            "data-playlists-region": "tree",
            class: if wide {
                "flex min-h-0 flex-col flex-none border-r"
            } else {
                "flex min-h-0 flex-col min-w-0 flex-1"
            },
            style: if wide {
                format!(
                    "width: {}px; background: var(--canvas-library-panel); border-color: var(--canvas-border);",
                    props.list_width
                )
            } else {
                "background: var(--canvas-library-panel); border-color: var(--canvas-border);".to_owned()
            },
            { tree_column(&frame, &link, &settings, picked.as_ref(), pick, menu) }
        }
    };
    let hero = rsx! {
        div {
            "data-playlists-region": "hero",
            class: if wide {
                "flex min-h-0 min-w-0 flex-1 flex-col"
            } else {
                "flex flex-none flex-col"
            },
            style: "background: var(--canvas-library-column); overflow-y: auto;",
            { hero_pane(&frame, &link, &settings, picked.as_ref()) }
        }
    };

    let menu_open = menu
        .cloned()
        .filter(|row_menu| pick_title(&frame, &row_menu.pick).is_some());
    let menu_view = menu_open.map(|row_menu| {
        let controls = controls_for(&row_menu.pick);
        let entries: Vec<GuiRowMenuEntry> = controls
            .iter()
            .map(|control| GuiRowMenuEntry {
                label: control.label().to_owned(),
                key_hint: None,
                destructive: *control == Control::Delete,
                checked: None,
                index: None,
            })
            .collect();
        let select_link = link.clone();
        let select_settings = settings.clone();
        let select_frame = frame.clone();
        let target = row_menu.pick.clone();
        let on_select = Callback::new(move |index: usize| {
            if let Some(control) = controls.get(index) {
                run_control(
                    &select_link,
                    &select_settings,
                    &select_frame,
                    &target,
                    *control,
                );
            }
            menu.set(None);
        });
        rsx! {
            div {
                class: "fixed",
                style: "left: 0; top: 0; width: 0; height: 0;",
                // A press on the menu does not close it.
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                QueueRowMenu {
                    cursor: None,
                    entries,
                    top: format!("{}px", row_menu.y),
                    left: Some(format!("{}px", row_menu.x)),
                    on_select: Some(on_select),
                    label: "Playlist actions".to_owned(),
                }
            }
        }
    });

    rsx! {
        div {
            class: if wide {
                "relative flex h-full min-h-0 min-w-0 flex-1 flex-row"
            } else {
                "relative flex h-full min-h-0 min-w-0 flex-1 flex-col"
            },
            style: "background: var(--canvas-library-column);",
            "data-playlists-panel": "",
            "data-playlists-arrangement": props.arrangement.name(),
            // A press outside the menu closes it.
            onpointerdown: move |_| {
                if menu.peek().is_some() {
                    menu.set(None);
                }
            },
            if wide {
                {tree}
                // The tree column's divider: dragging resizes it, a
                // double-click returns the canvas width.
                PanelDivider {
                    side: DividerSide::Left,
                    name: DividerName::List,
                    width: props.list_width,
                    on_drag: move |width| {
                        drag_widths.drag_list_width(width, props.library_column);
                    },
                    on_reset: move |()| {
                        reset_widths.reset_list_width();
                    },
                }
                {hero}
            } else {
                {hero}
                {tree}
            }
            if let Some(menu_view) = menu_view {
                {menu_view}
            }
        }
    }
}

/// Pick a row and open its right-click menu at the pointer.
fn open_menu(
    event: &MouseEvent,
    this: Pick,
    mut pick: Signal<Option<Pick>>,
    mut menu: Signal<Option<RowMenu>>,
) {
    event.prevent_default();
    let at = event.client_coordinates();
    pick.set(Some(this.clone()));
    menu.set(Some(RowMenu {
        pick: this,
        x: f32_from_f64(at.x),
        y: f32_from_f64(at.y),
    }));
}

/// The tree: the header with the list's refresh, then one row per playlist,
/// the open one followed by its items.
fn tree_column(
    frame: &GuiPlaylistsFrame,
    link: &CoreLink,
    settings: &SettingsState,
    picked: Option<&Pick>,
    pick: Signal<Option<Pick>>,
    menu: Signal<Option<RowMenu>>,
) -> Element {
    let refresh_link = link.clone();
    let refresh_label = if frame.loading {
        "Refreshing…"
    } else {
        "Refresh"
    };
    rsx! {
        div {
            class: "flex flex-none items-center justify-between",
            style: "height: 48px; padding: 0 12px 0 16px;",
            span {
                style: "font-size: 15px; font-weight: 600; color: var(--canvas-heading);",
                "Playlists"
            }
            SettingsButton {
                variant: SettingsButtonVariant::Ghost,
                onpress: Some(Callback::new(move |()| {
                    refresh_link.playlists(GuiPlaylistsCommand::Refresh { id: None });
                })),
                "{refresh_label}"
            }
        }
        div {
            class: "flex min-h-0 flex-1 flex-col overflow-y-auto",
            style: "padding: 0 8px 8px 8px;",
            "data-playlists-tree": "",
            if frame.playlists.is_empty() {
                div {
                    style: "padding: 8px; font-size: 13px; color: var(--canvas-muted);",
                    if frame.loading { "Loading playlists…" } else { "No saved playlists" }
                }
            }
            for playlist in &frame.playlists {
                { playlist_node(playlist, frame, link, settings, picked, pick, menu) }
            }
        }
    }
}

/// One playlist's node: its row, the rename field while renaming it, and its
/// items while it is the open one.
fn playlist_node(
    playlist: &GuiPlaylistRow,
    frame: &GuiPlaylistsFrame,
    link: &CoreLink,
    settings: &SettingsState,
    picked: Option<&Pick>,
    pick: Signal<Option<Pick>>,
    menu: Signal<Option<RowMenu>>,
) -> Element {
    let rename = settings.ui.cloned().rename;
    let id = playlist.id.clone();
    let open = frame.open.as_ref().filter(|open| open.id == id);
    let is_open = open.is_some();
    // The open playlist holds at least one item row, so it starts its guide
    // (`gui-tree-indent-guides` design D3, D4).
    let shows_items = open.is_some_and(|open| !open.items.is_empty());
    let selected = picked == Some(&Pick::Playlist(id.clone()));
    let toggle_link = link.clone();
    let press_id = id.clone();
    let toggle_id = id.clone();
    let context_id = id.clone();
    let rename_here = rename.as_ref().is_some_and(|draft| draft.id == id);
    let rename_name = rename.as_ref().map(|draft| draft.name.clone());
    let mut press_pick = pick;
    rsx! {
        div {
            key: "{id}",
            class: "flex flex-col",
            button {
                "type": "button",
                class: "tree-parent relative flex w-full items-center text-left",
                style: row_style(selected, 0.0),
                "aria-expanded": "{is_open}",
                "data-playlists-row": "playlist",
                onclick: move |_| {
                    press_pick.set(Some(Pick::Playlist(press_id.clone())));
                },
                ondoubleclick: move |_| {
                    toggle_link.playlists(toggle_command(is_open, &toggle_id));
                },
                oncontextmenu: move |event: MouseEvent| {
                    open_menu(&event, Pick::Playlist(context_id.clone()), pick, menu);
                },
                Truncate {
                    text: playlist.name.clone(),
                    class: "min-w-0 flex-1".to_owned(),
                    style: tree_row_text(),
                    css: "color: var(--canvas-text);".to_owned(),
                }
                span {
                    class: "tree-trailing flex-none font-mono tabular-nums",
                    style: "font-size: 12px; color: var(--canvas-muted);",
                    "{playlist.item_count}"
                }
                if shows_items {
                    {parent_guide_start(PLAYLIST_GUIDE_X, PLAYLIST_ROW_HEIGHT_PX, 0.0)}
                }
                TreeHint { open: is_open }
            }
            if rename_here {
                { rename_field(settings, rename_name, shows_items) }
            }
            if is_open {
                { items_block(frame.open.as_ref(), link, picked, pick, menu) }
            }
        }
    }
}

/// The open playlist's item rows, indented under it. Each item row is a leaf of
/// the tree, so it carries its playlist's guide from its top to its bottom
/// (`gui-tree-indent-guides` design D3).
fn items_block(
    open: Option<&GuiOpenPlaylist>,
    link: &CoreLink,
    picked: Option<&Pick>,
    pick: Signal<Option<Pick>>,
    menu: Signal<Option<RowMenu>>,
) -> Element {
    let Some(open) = open else {
        return rsx! {};
    };
    rsx! {
        div {
            class: "flex flex-col",
            "data-playlists-items": "",
            for item in &open.items {
                {
                    let this = Pick::Item {
                        playlist: open.id.clone(),
                        item: item.id.clone(),
                    };
                    let selected = picked == Some(&this);
                    let press = this.clone();
                    let context = this.clone();
                    let play_link = link.clone();
                    let play_playlist = open.id.clone();
                    let play_item = item.id.clone();
                    let mut press_pick = pick;
                    rsx! {
                        button {
                            key: "{item.id}",
                            "type": "button",
                            class: "relative flex w-full items-center text-left",
                            style: row_style(selected, 24.0),
                            "data-playlists-row": "item",
                            onclick: move |_| press_pick.set(Some(press.clone())),
                            ondoubleclick: move |_| {
                                play_link.playlists(GuiPlaylistsCommand::ItemAction {
                                    id: play_playlist.clone(),
                                    item_id: play_item.clone(),
                                    action: GuiPlaylistAction::Play,
                                });
                            },
                            oncontextmenu: move |event: MouseEvent| {
                                open_menu(&event, context.clone(), pick, menu);
                            },
                            Truncate {
                                text: item.name.clone(),
                                class: "min-w-0 flex-1".to_owned(),
                                style: tree_row_text(),
                                css: "color: var(--canvas-text);".to_owned(),
                            }
                            TreeGuide {
                                x: PLAYLIST_GUIDE_X,
                                tone: GuideTone::Top,
                                top: 0.0,
                                bottom: 0.0,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A Playlists tree row's height, in pixels (`gui-tree-indent-guides` design
/// D3): [`row_style`] paints it and the open playlist's guide start is measured
/// from its bottom.
const PLAYLIST_ROW_HEIGHT_PX: f32 = 36.0;

/// One tree row's box: a [`PLAYLIST_ROW_HEIGHT_PX`] row, indented for an item,
/// filled while picked.
fn row_style(selected: bool, indent: f32) -> String {
    format!(
        "height: {PLAYLIST_ROW_HEIGHT_PX}px; padding: 0 {PLAYLIST_LABEL_X}px 0 {}px; border: 0; border-radius: 6px; font-size: {TREE_ROW_FONT_PX}px; background: {}; cursor: pointer;",
        PLAYLIST_LABEL_X + indent,
        if selected {
            "var(--canvas-selected)"
        } else {
            "none"
        },
    )
}

/// The hero: the picked row's title and its control set, or a hint while no
/// row is picked.
fn hero_pane(
    frame: &GuiPlaylistsFrame,
    link: &CoreLink,
    settings: &SettingsState,
    picked: Option<&Pick>,
) -> Element {
    let Some(picked) = picked else {
        return rsx! {
            div {
                style: "padding: 24px; font-size: 13px; color: var(--canvas-muted);",
                "data-playlists-hero-empty": "",
                "Select a playlist"
            }
        };
    };
    let Some((title, detail)) = pick_title(frame, picked) else {
        return rsx! {};
    };
    rsx! {
        div {
            class: "flex flex-col",
            style: "padding: 24px; gap: 16px;",
            "data-playlists-hero": "",
            div {
                class: "flex flex-col",
                style: "gap: 4px;",
                span {
                    style: "font-size: 22px; font-weight: 600; color: var(--canvas-heading);",
                    "{title}"
                }
                span {
                    style: "font-size: 13px; color: var(--canvas-muted);",
                    "{detail}"
                }
            }
            div {
                class: "flex flex-wrap",
                style: "gap: 8px;",
                for control in controls_for(picked) {
                    {
                        let control = *control;
                        let run_link = link.clone();
                        let run_settings = settings.clone();
                        let run_frame = frame.clone();
                        let target = picked.clone();
                        rsx! {
                            button {
                                key: "{control.label()}",
                                "type": "button",
                                class: "flex items-center flex-none whitespace-nowrap",
                                style: match control {
                                    Control::Play => ACCENT_STYLE,
                                    Control::Delete => DESTRUCTIVE_STYLE,
                                    _ => RAISED_STYLE,
                                },
                                onclick: move |_| {
                                    run_control(&run_link, &run_settings, &run_frame, &target, control);
                                },
                                "{control.label()}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The rename field: the draft's input under its focus host, mounted with
/// `focus_on_mount`, which arms the rename hold and focuses the input. It
/// carries its playlist's guide while `guide` holds — the playlist is open and
/// shows item rows — so the guide has no gap during a rename, and paints none
/// under a closed playlist (`gui-tree-indent-guides` design D3).
fn rename_field(settings: &SettingsState, name: Option<String>, guide: bool) -> Element {
    let focus_settings = settings.clone();
    let blur_settings = settings.clone();
    let input_settings = settings.clone();
    let arm_settings = settings.clone();
    rsx! {
        div {
            class: "relative flex",
            style: "padding: 4px 8px 4px 28px;",
            "data-playlists-rename": "",
            onfocusin: move |_| focus_settings.focus_settings_field(SettingsForm::Rename),
            onfocusout: move |_| blur_settings.blur_settings_field(SettingsForm::Rename),
            input {
                "type": "text",
                "aria-label": "New playlist name",
                style: "flex: 1;",
                value: name.unwrap_or_default(),
                "data-settings-field": "rename",
                onmounted: move |event: Event<MountedData>| {
                    focus_on_mount(event, &arm_settings, SettingsForm::Rename);
                },
                oninput: move |event: Event<FormData>| {
                    input_settings.set_rename_draft(&event.value());
                },
            }
            if guide {
                TreeGuide {
                    x: PLAYLIST_GUIDE_X,
                    tone: GuideTone::Top,
                    top: 0.0,
                    bottom: 0.0,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
