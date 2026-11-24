//! The GUI Library panel's item menu (add-gui-library-panel design D8): the
//! frame's carried menu types, and the GUI core's half that builds the shared
//! context menu for one item and runs a chosen entry. The entry indexes are the
//! shared builder's, so a window that paints only some of the entries still
//! executes the one it painted. The TUI's own menu paths do not change.
//!
//! The block runs one step over its own `App` and reads and writes the GUI
//! Client's state for the row menu the window last named (design D4), so each
//! entry is a free function that takes the `App` and that state and the
//! `impl GuiCore` block hands it both.

use mbv_queue::QueueItem;
use mbv_ui_model::context_menu::AudiobookshelfMenuTarget;
use mbv_ui_model::context_menu::ContextAction;
use mbv_ui_model::context_menu::ContextMenu;
use mbv_ui_model::context_menu::ContextMenuEntry;
use mbv_ui_model::settings::PanelFocus;

use super::GuiDestinationKind;
use super::GuiHomeRowId;
use super::item_actions::GuiLibraryMenuTarget;
use crate::app::App;
use crate::gui::GuiCore;
use crate::gui::gui_client_state::GuiClientState;

/// The row one Library menu names: the window sends it with `rowMenu` and
/// echoes it back with `menuEntry`, so the App rebuilds the same entries the
/// frame painted. A destination row keeps its stable Service id; a Home row
/// keeps its own stable identity, because its menu resolves through the Home
/// sources rather than the destination's stores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiLibraryMenuRow {
    /// A Library destination row (Music, Shows, a plain Emby library,
    /// Audiobooks, Podcasts, or Feeds), by the row's Service id.
    Item(String),
    /// A Home row, by its stable identity.
    Home(GuiHomeRowId),
}

/// The Library item menu the frame's `rowMenu` section carries: the row the
/// entries are about, and the TUI menu builder's entries in the builder's
/// order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiLibraryRowMenu {
    pub row: GuiLibraryMenuRow,
    pub entries: Vec<GuiLibraryMenuItem>,
}

/// One entry of the GUI Library panel's item menu (design D8): the label the
/// row menu paints, the key hint its surface shows when the entry has one, and
/// the entry's index in the shared builder's order — the index a chosen entry
/// sends back, so a window that paints only some of the entries (the
/// Hero's More actions drops its duplicated ones) still executes the entry it
/// painted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiLibraryMenuItem {
    pub label: String,
    pub key_hint: Option<String>,
    pub index: usize,
}

impl GuiCore {
    /// The frame's form of the shared builder's menu entries (design D8): the
    /// builder's labels and key hints, indexed by the entry's position in the
    /// builder's order — the order the execution rebuild and every carried
    /// menu share.
    fn gui_library_menu_items(entries: Vec<ContextMenuEntry>) -> Vec<GuiLibraryMenuItem> {
        entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| GuiLibraryMenuItem {
                label: entry.label.to_string(),
                key_hint: entry.key_hint.map(str::to_string),
                index,
            })
            .collect()
    }

    /// The item's menu entries for the frame (design D8): the shared builder's
    /// entries for `item_id` while the Library panel has Panel focus, which is
    /// also what `execute_context_action` needs to find the library. Entry
    /// indexes are the builder's, shared by every carried menu and by the
    /// execution rebuild — the Hero's More actions drops its own duplicate
    /// labels at paint time, never here. Empty when the item is gone, no menu
    /// applies, or another panel holds focus.
    pub fn gui_library_menu(&mut self, item_id: &str) -> Vec<GuiLibraryMenuItem> {
        let entries = library_item_menu(&mut self.app, &self.gui, item_id)
            .map(|menu| menu.entries)
            .unwrap_or_default();
        Self::gui_library_menu_items(entries)
    }

    /// The right-click item menu for the frame's `rowMenu` section: the
    /// TUI menu builder's full entries for the row the row-menu command
    /// named, whichever destination painted it, deferring to nothing the Hero
    /// carries. `None` while no right-click has named a row, the named row is
    /// gone, or another panel holds focus.
    pub fn gui_library_row_menu(&mut self) -> Option<GuiLibraryRowMenu> {
        let app = &mut self.app;
        let gui = &self.gui;
        let row = gui.library_row_menu_target.clone()?;
        if !matches!(app.effective_panel_focus(), PanelFocus::Library) {
            return None;
        }
        let target = menu_target(app, gui, &row)?;
        let entries = library_menu_entries(app, &target)?;
        if entries.iter().all(|entry| entry.action.is_none()) {
            return None;
        }
        Some(GuiLibraryRowMenu {
            row,
            entries: Self::gui_library_menu_items(entries),
        })
    }

    /// Run the row menu's entry at `index` (design D8): the entry the frame
    /// painted, rebuilt from the row it names so the action targets that
    /// row, resolved by stable id and never by the TUI's library cursor. The
    /// two actions whose shared executor resolves through the cursor (bare
    /// Play and bare Enqueue) run through the row's own destination path: an
    /// Emby Library row takes the GUI Library's explicit play and add (with
    /// the autoload rule), and a Home row takes the Home play and enqueue
    /// paths. The GUI never arms the TUI's `pending_overlay`.
    pub(super) fn run_gui_library_menu_entry(&mut self, row: &GuiLibraryMenuRow, index: usize) {
        run_library_menu_entry(&mut self.app, &self.gui, row, index);
    }
}

/// Run the row menu's entry at `index` (design D8): the entry the frame
/// painted, rebuilt from the row it names so the action targets that
/// row, resolved by stable id and never by the TUI's library cursor. The
/// two actions whose shared executor resolves through the cursor (bare
/// Play and bare Enqueue) run through the row's own destination path: an
/// Emby Library row takes the GUI Library's explicit play and add (with
/// the autoload rule), and a Home row takes the Home play and enqueue
/// paths. The GUI never arms the TUI's `pending_overlay`.
fn run_library_menu_entry(
    app: &mut App,
    gui: &GuiClientState,
    row: &GuiLibraryMenuRow,
    index: usize,
) {
    let Some(target) = menu_target(app, gui, row) else {
        super::log_ignored_library_command(
            "menuEntry",
            format_args!("the row names nothing the Service data holds"),
        );
        return;
    };
    let action = library_menu_entries(app, &target)
        .and_then(|entries| entries.into_iter().nth(index))
        .and_then(|entry| entry.action);
    match (action, &target) {
        (Some(ContextAction::Play), GuiLibraryMenuTarget::Library { lib_idx, item }) => {
            super::item_actions::play_library_item(app, gui, *lib_idx, item.clone());
        }
        (Some(ContextAction::Enqueue), GuiLibraryMenuTarget::Library { item, .. }) => {
            super::item_actions::add_library_item(app, gui, &item.id);
        }
        (
            Some(ContextAction::Play),
            GuiLibraryMenuTarget::Home {
                item,
                continue_watching,
            },
        ) => {
            app.home_play_target(QueueItem::Emby(Box::new(item.clone())), *continue_watching);
        }
        (
            Some(ContextAction::Enqueue),
            GuiLibraryMenuTarget::Home {
                item,
                continue_watching,
            },
        ) => {
            app.home_enqueue_target(QueueItem::Emby(Box::new(item.clone())), *continue_watching);
        }
        // The Continue Watching removal needs the row's item, as the TUI's
        // own Home menu passes it; every other Home action carries its own
        // targets.
        (action, GuiLibraryMenuTarget::Home { item, .. }) => {
            app.execute_context_action(action, Some(item.clone()));
        }
        (action, _) => app.execute_context_action(action, None),
    }
}

/// The entries the row's destination builder produces for `target`
/// (standard-media-context-menus design D3): one builder per Service, so
/// the GUI carries the same standard action set the TUI's row menus do.
fn library_menu_entries(
    app: &mut App,
    target: &GuiLibraryMenuTarget,
) -> Option<Vec<ContextMenuEntry>> {
    match target {
        GuiLibraryMenuTarget::Library { item, .. } => app
            .build_context_menu_for(Some(item.clone()), false, None)
            .map(|menu| menu.entries),
        GuiLibraryMenuTarget::Home {
            item,
            continue_watching,
        } => {
            let menu = if *continue_watching {
                app.build_continue_watching_context_menu(item)
            } else {
                app.build_explicit_item_context_menu(item)
            };
            menu.map(|menu| menu.entries)
        }
        GuiLibraryMenuTarget::Audiobookshelf(target) => {
            Some(app.build_audiobookshelf_menu_entries(std::slice::from_ref(target)))
        }
        GuiLibraryMenuTarget::Feed(entry) => {
            Some(App::build_feeds_menu_entries(std::slice::from_ref(entry)))
        }
    }
}

/// The row a command named, resolved to its destination's menu target
/// (design D2): the Home row's stable identity, or the Service id probed
/// against the active destination first — the row the press named is the
/// row the active list painted — then every store, so a press whose
/// destination changed under a stale frame still addresses the row it
/// painted.
fn menu_target(
    app: &App,
    gui: &GuiClientState,
    row: &GuiLibraryMenuRow,
) -> Option<GuiLibraryMenuTarget> {
    match row {
        GuiLibraryMenuRow::Home(home) => {
            let target = home.clone().into_home_item_target();
            match app.home_stable_target(&target)? {
                QueueItem::Emby(item) => Some(GuiLibraryMenuTarget::Home {
                    item: *item,
                    continue_watching: target.is_continue(),
                }),
                QueueItem::Audiobookshelf(item) => Some(GuiLibraryMenuTarget::Audiobookshelf(
                    AudiobookshelfMenuTarget::for_item(&item),
                )),
                QueueItem::Feed(entry) => Some(GuiLibraryMenuTarget::Feed(entry)),
                QueueItem::Twitch(_) => None,
            }
        }
        GuiLibraryMenuRow::Item(item_id) => item_target(app, gui, item_id),
    }
}

/// The destination menu target for a Library row's Service id, active
/// destination first.
fn item_target(app: &App, gui: &GuiClientState, item_id: &str) -> Option<GuiLibraryMenuTarget> {
    match super::tabs::active_destination(app) {
        Some(GuiDestinationKind::Audiobooks { .. } | GuiDestinationKind::Podcasts { .. }) => {
            if let Some(target) = super::item_actions::audiobookshelf_target(app, item_id) {
                return Some(GuiLibraryMenuTarget::Audiobookshelf(target));
            }
        }
        Some(GuiDestinationKind::Feeds) => {
            if let Some(entry) = super::feeds::feed_entry(app, gui, item_id) {
                return Some(GuiLibraryMenuTarget::Feed(entry));
            }
        }
        _ => {
            if let Some(target) = destination_target(app, gui, item_id) {
                return Some(target);
            }
        }
    }
    destination_target(app, gui, item_id)
        .or_else(|| {
            super::item_actions::audiobookshelf_target(app, item_id)
                .map(GuiLibraryMenuTarget::Audiobookshelf)
        })
        .or_else(|| super::feeds::feed_entry(app, gui, item_id).map(GuiLibraryMenuTarget::Feed))
}

/// An Emby Library row's menu target: the item's own library store.
fn destination_target(
    app: &App,
    gui: &GuiClientState,
    item_id: &str,
) -> Option<GuiLibraryMenuTarget> {
    let (lib_idx, item) = super::item_actions::library_item(app, gui, item_id)?;
    Some(GuiLibraryMenuTarget::Library { lib_idx, item })
}

/// The item's menu from the shared builder (design D8): the Library panel's
/// focus and an Emby library tab are what both the builder's target
/// resolution and the executor's library lookup need.
fn library_item_menu(app: &mut App, gui: &GuiClientState, item_id: &str) -> Option<ContextMenu> {
    if !matches!(app.effective_panel_focus(), PanelFocus::Library) {
        return None;
    }
    let (_lib_idx, item) = super::item_actions::library_item(app, gui, item_id)?;
    app.build_context_menu_for(Some(item), false, None)
}
