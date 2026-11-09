//! The GUI Library panel's item menu (add-gui-library-panel design D8): the
//! frame's carried menu types, and the App half that builds the shared context
//! menu for one item and runs a chosen entry. The entry indexes are the shared
//! builder's, so a front end that paints only some of the entries still
//! executes the one it painted. The TUI's own menu paths do not change.

use mbv_ui_model::context_menu::ContextAction;
use mbv_ui_model::context_menu::ContextMenu;
use mbv_ui_model::context_menu::ContextMenuEntry;
use mbv_ui_model::settings::PanelFocus;

use crate::app::App;

/// The Library item menu the frame's `rowMenu` section carries: the item
/// the entries are about, and the TUI menu builder's entries in the
/// builder's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiLibraryRowMenu {
    pub item_id: String,
    pub entries: Vec<GuiLibraryMenuItem>,
}

/// One entry of the GUI Library panel's item menu (design D8): the label the
/// row menu paints, the key hint its surface shows when the entry has one, and
/// the entry's index in the shared builder's order — the index a chosen entry
/// sends back, so a front end that paints only some of the entries (the
/// Hero's More actions drops its duplicated ones) still executes the entry it
/// painted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiLibraryMenuItem {
    pub label: String,
    pub key_hint: Option<String>,
    pub index: usize,
}

impl App {
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
        let entries = self
            .gui_library_item_menu(item_id)
            .map(|menu| menu.entries)
            .unwrap_or_default();
        Self::gui_library_menu_items(entries)
    }

    /// The right-click item menu for the frame's `rowMenu` section: the
    /// TUI menu builder's full entries for the item the row-menu command
    /// named, deferring to nothing the Hero carries. `None` while no
    /// right-click has named an item or the named item is gone.
    pub fn gui_library_row_menu(&mut self) -> Option<GuiLibraryRowMenu> {
        let item_id = self.gui_library_row_menu_target.clone()?;
        let entries = self
            .gui_library_item_menu(&item_id)
            .map(|menu| Self::gui_library_menu_items(menu.entries))?;
        Some(GuiLibraryRowMenu {
            item_id: item_id.clone(),
            entries,
        })
    }

    /// Run the item menu's entry at `index` (design D8): the entry the frame
    /// painted, rebuilt from the item it names so the action targets that
    /// item, resolved by stable id and never by the TUI's library cursor —
    /// the cursor may rest on any other row. The two actions whose shared
    /// executor resolves through the cursor (bare Play and bare Enqueue)
    /// route to the item's own play and add: the GUI Library's explicit-play
    /// replace and its add path apply the autoload rule. The GUI never arms
    /// the TUI's `pending_overlay`.
    pub(super) fn run_gui_library_menu_entry(&mut self, item_id: &str, index: usize) {
        let action = self
            .gui_library_item_menu(item_id)
            .and_then(|menu| menu.entries.into_iter().nth(index))
            .and_then(|entry| entry.action);
        if let Some((lib_idx, item)) = self.gui_library_item(item_id) {
            match action {
                Some(ContextAction::Play) => self.play_gui_library_item(lib_idx, item),
                Some(ContextAction::Enqueue) => self.add_gui_library_item_to_queue(&item.id),
                Some(action) => self.execute_context_action(Some(action), None),
                None => Self::log_ignored_library_command(
                    "menuEntry",
                    format_args!("the item's menu holds no entry at {index}"),
                ),
            }
        } else {
            Self::log_ignored_library_command(
                "menuEntry",
                format_args!("the item's menu holds no entry at {index}"),
            );
        }
    }

    /// The item's menu from the shared builder (design D8): the Library panel's
    /// focus and an Emby library tab are what both the builder's target
    /// resolution and the executor's library lookup need.
    fn gui_library_item_menu(&mut self, item_id: &str) -> Option<ContextMenu> {
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            return None;
        }
        let (_lib_idx, item) = self.gui_library_item(item_id)?;
        self.build_context_menu_for(Some(item), false, None)
    }
}
