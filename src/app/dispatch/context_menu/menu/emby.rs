//! The Emby context-menu entries: target resolution and the folder, leaf,
//! explicit-item, and Home mixed-selection entry families.
use super::menu_anchor;
use crate::app::{App, ContextAction, ContextMenuAnchor, ContextMenuEntry, PanelFocus};
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;
use mbv_ui_model::context_menu::ContextMenu;
use mbv_ui_model::overlay::OverlayRequest;

impl App {
    /// `build_context_menu` with an explicitly resolved Emby-library item
    /// (task 5.3d, Album track focus): while an inline album track is
    /// focused, the shell resolves the track (the component owns the cursor)
    /// and passes it here so '.' targets the focused track instead of the
    /// album row. All other arms resolve exactly as `build_context_menu`.
    ///
    /// The GUI Library panel calls it for its picked item (add-gui-library-
    /// panel design D8) and never raises the result through `pending_overlay`:
    /// the frame carries the entries to the window content.
    pub(in crate::app) fn build_context_menu_for(
        &mut self,
        tracked_item: Option<EmbyItem>,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) -> Option<ContextMenu> {
        let cw_focused = matches!(
            self.effective_panel_focus(),
            mbv_ui_model::settings::PanelFocus::Library
        ) && self.tab.is_home();
        let (current_item, queue_cursor) =
            self.resolve_context_menu_target(tracked_item, cw_item)?;
        self.context_menu_from_resolved_target(
            current_item.as_ref(),
            queue_cursor,
            cw_focused,
            home_cw_selected,
        )
    }

    /// Menu content from an already-resolved target: the caller hands over
    /// the item and the queue cursor it resolved at the Model boundary, plus
    /// the two Continue-Watching facts (`cw_focused`, `home_cw_selected`) the
    /// leaf entries consult. Tab-independent, so a Home `Recently added`
    /// Emby row can open the destination row menu through
    /// [`App::open_explicit_item_context_menu`] without the Home tab's
    /// `cw_item` re-read.
    fn context_menu_from_resolved_target(
        &self,
        current_item: Option<&EmbyItem>,
        queue_cursor: Option<usize>,
        cw_focused: bool,
        home_cw_selected: bool,
    ) -> Option<ContextMenu> {
        let mut entries = Vec::new();
        if let Some(item) = current_item {
            if item.is_folder {
                Self::push_folder_context_actions(&mut entries, item);
            } else {
                self.push_leaf_context_actions(
                    &mut entries,
                    item,
                    queue_cursor,
                    cw_focused,
                    home_cw_selected,
                );
            }
        }

        // The Queue list Shuffle entry (queue-content-shuffle design D6):
        // every Queue row's menu offers it, appended last after the Emby
        // entries, and a Feed or Audiobookshelf Queue row gets a menu with
        // only the entry. The entry reorders the existing queue once. It hides
        // entirely when the owner lacks the `queue-shuffle` ctrl capability.
        if matches!(
            self.effective_panel_focus(),
            mbv_ui_model::settings::PanelFocus::Queue
        ) && self.player.supports_queue_shuffle()
        {
            Self::push_context_action(&mut entries, "Shuffle", ContextAction::ShuffleQueue);
        }

        if entries.iter().all(|entry| entry.action.is_none()) {
            return None;
        }

        let anchor = match self.effective_panel_focus() {
            mbv_ui_model::settings::PanelFocus::Library => {
                ContextMenuAnchor::SelectedItem(mbv_ui_model::settings::PanelFocus::Library)
            }
            mbv_ui_model::settings::PanelFocus::Queue => {
                ContextMenuAnchor::SelectedItem(mbv_ui_model::settings::PanelFocus::Queue)
            }
        };
        Some(ContextMenu {
            anchor,
            cursor: ContextMenu::first_selectable(&entries),
            entries,
            bounds: None,
        })
    }

    /// The destination row menu for an explicitly resolved Emby item
    /// (spec `home-latest-sections`, "Home actions resolve the row's own
    /// section and Service identity"): a Home `Recently added` Emby row
    /// opens the same menu its row offers on the destination library — Play,
    /// Add to Queue, and the played-state entry — and never the Continue
    /// Watching removal. The bare `Play` and `Enqueue` actions execute
    /// through the shell's `home_context_item`, which the caller sets to
    /// this item.
    pub(in crate::app) fn open_explicit_item_context_menu(
        &mut self,
        item: &EmbyItem,
        anchor: Option<(u16, u16)>,
    ) {
        let Some(mut menu) = self.build_explicit_item_context_menu(item) else {
            return;
        };
        menu.anchor = menu_anchor(anchor, mbv_ui_model::settings::PanelFocus::Library);
        self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
    }

    /// The destination row menu for an explicitly resolved Emby item, without
    /// raising it: the TUI's Home `Recently added` opener and the GUI Library
    /// panel's carried row menu share these entries, so the two surfaces
    /// cannot drift.
    pub(in crate::app) fn build_explicit_item_context_menu(
        &self,
        item: &EmbyItem,
    ) -> Option<ContextMenu> {
        self.context_menu_from_resolved_target(Some(item), None, false, false)
    }

    /// The Continue Watching row menu for an explicitly resolved Emby item,
    /// without raising it: the same entries the TUI's Home Continue row opens
    /// (Play, Add to Queue, the one mark entry, and Remove from Continue
    /// Watching), shared with the GUI Library panel's Home rows.
    pub(in crate::app) fn build_continue_watching_context_menu(
        &self,
        item: &EmbyItem,
    ) -> Option<ContextMenu> {
        self.context_menu_from_resolved_target(Some(item), None, true, false)
    }

    /// The Home mixed-Service multi-selection menu (spec
    /// `home-latest-sections`, "Home actions resolve the row's own section and
    /// Service identity"): a selection spanning Services offers Play and Add to Queue only,
    /// and both entries carry the shell-resolved queue items in display
    /// order. An empty resolution opens nothing.
    pub(in crate::app) fn open_home_mixed_context_menu(
        &mut self,
        items: Vec<QueueItem>,
        anchor: Option<(u16, u16)>,
    ) {
        if items.is_empty() {
            return;
        }
        let mut entries = Vec::new();
        Self::push_context_action(
            &mut entries,
            "Play",
            ContextAction::HomePlaySelection(items.clone()),
        );
        Self::push_context_action(
            &mut entries,
            "Add to Queue",
            ContextAction::HomeEnqueueSelection(items),
        );
        self.pending_overlay = Some(OverlayRequest::ContextMenu(ContextMenu {
            anchor: menu_anchor(anchor, PanelFocus::Library),
            cursor: ContextMenu::first_selectable(&entries),
            entries,
            bounds: None,
        }));
    }

    fn resolve_context_menu_target(
        &mut self,
        tracked_item: Option<EmbyItem>,
        cw_item: Option<EmbyItem>,
    ) -> Option<(Option<EmbyItem>, Option<usize>)> {
        // Exhaustive dispatch by panel and destination (design §5): a context
        // menu opens only for Home (library focus), an explicitly selected
        // Emby library, or an Emby queue item. Audiobookshelf and Feeds browse
        // rows, non-Emby queue items, and absent or stale targets produce no
        // Emby menu.
        // On the queue, the resolved index (the right-clicked slot, written by
        // `handle_mouse_single_click_queue` before the menu opens) is retained
        // once and carried into every menu action that targets it (D2): the
        // `PlayQueue(index)` and `RemoveFromQueue(index)` actions close over
        // the clicked slot, so a follow update to `queue_cursor` cannot
        // redirect them.
        let mut queue_cursor = None;
        let current_item = match (self.effective_panel_focus(), self.tab) {
            (
                mbv_ui_model::settings::PanelFocus::Library,
                mbv_ui_model::tab_selection::TabSelection::Home,
            ) => cw_item,
            (
                mbv_ui_model::settings::PanelFocus::Library,
                mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(lib_idx),
            ) => tracked_item.or_else(|| {
                let cursor = self
                    .libs
                    .get(lib_idx)
                    .and_then(|lib| lib.nav_stack.last())
                    .map_or(0, |level| level.resting().cursor());
                self.current_lib_item(lib_idx, cursor)
            }),
            (
                mbv_ui_model::settings::PanelFocus::Library,
                mbv_ui_model::tab_selection::TabSelection::AudiobookshelfLibrary(_)
                | mbv_ui_model::tab_selection::TabSelection::Feeds,
            ) => return None,
            (mbv_ui_model::settings::PanelFocus::Queue, _) => {
                let cursor = self.displayed_queue().cursor();
                queue_cursor = Some(cursor);
                self.displayed_queue().emby_item_at(cursor).cloned()
            }
        };
        Some((current_item, queue_cursor))
    }

    fn push_folder_context_actions(entries: &mut Vec<ContextMenuEntry>, item: &EmbyItem) {
        Self::push_context_action(
            entries,
            "Play All",
            ContextAction::PlayFolder(item.id.clone()),
        );
        Self::push_context_action(
            entries,
            "Shuffle",
            ContextAction::ShuffleFolder(item.id.clone()),
        );
        Self::push_context_action(
            entries,
            "Add to Queue",
            ContextAction::EnqueueFolder(Box::new(item.clone())),
        );
        Self::push_play_state_context_action(entries, item);
    }

    fn push_play_state_context_action(entries: &mut Vec<ContextMenuEntry>, item: &EmbyItem) {
        // Music is fire-and-forget in mbv: its played state is ignored
        // everywhere, so a mark entry would have no visible effect (context-menu
        // spec, music rule). `is_music` covers tracks, albums, and artists.
        if item.is_music() {
            return;
        }
        if App::context_menu_play_state(item) {
            Self::push_context_action(
                entries,
                "Mark Unplayed",
                ContextAction::MarkUnplayed(item.id.clone()),
            );
        } else {
            Self::push_context_action(
                entries,
                "Mark Played",
                ContextAction::MarkPlayed(item.id.clone()),
            );
        }
    }

    fn push_leaf_context_actions(
        &self,
        entries: &mut Vec<ContextMenuEntry>,
        item: &EmbyItem,
        queue_cursor: Option<usize>,
        cw_focused: bool,
        home_cw_selected: bool,
    ) {
        // Queue menus carry the resolved index in the action itself
        // (ContextAction::PlayQueue, D2); Home/library menus keep bare Play.
        match queue_cursor {
            Some(pos) => Self::push_context_action(entries, "Play", ContextAction::PlayQueue(pos)),
            None => Self::push_context_action(entries, "Play", ContextAction::Play),
        }
        if cw_focused
            || self.context_menu_lib_idx().is_some()
            || !matches!(
                self.effective_panel_focus(),
                mbv_ui_model::settings::PanelFocus::Queue
            )
        {
            Self::push_context_action(entries, "Add to Queue", ContextAction::Enqueue);
        }
        // Music items (tracks, albums, artists) don't get a mark entry;
        // `push_play_state_context_action` enforces the music rule itself.
        Self::push_play_state_context_action(entries, item);
        // `home_cw_selected` is the component-derived authoritative
        // fact (resolved at the Model boundary), replacing the deleted
        // numeric `App.home.section == 0` read. `cw_focused` (Library
        // focus + Home tab) already subsumes it on the Home keyboard / Home
        // right-click paths; preserve the odd Queue-focus coupling: with the
        // Queue panel focused while Home is the active Tab selection, the
        // entry appears exactly when the Home component has Continue Watching
        // selected.
        if cw_focused || (self.tab.is_home() && home_cw_selected) {
            Self::push_context_action(
                entries,
                "Remove from Continue Watching",
                ContextAction::RemoveFromContinueWatching,
            );
        }
        if !cw_focused
            && matches!(
                self.effective_panel_focus(),
                mbv_ui_model::settings::PanelFocus::Queue
            )
        {
            let pos = self.displayed_queue().cursor();
            Self::push_context_action(
                entries,
                "Remove from Queue",
                ContextAction::RemoveFromQueue(pos),
            );
        }
        if matches!(
            self.effective_panel_focus(),
            mbv_ui_model::settings::PanelFocus::Queue
        ) {
            Self::push_context_action(
                entries,
                "Go to Library",
                ContextAction::GoToLibrary(item.id.clone(), item.item_type.clone()),
            );
        }
    }
}
