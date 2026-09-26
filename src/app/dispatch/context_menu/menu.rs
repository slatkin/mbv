use crate::app::state::context_menu_capabilities::ItemCapabilities;
use crate::app::state::types::context_menu::BulkRemoveTarget;
use crate::app::state::types::context_menu::ContextMenu;
use crate::app::state::types::overlay::OverlayRequest;
use crate::app::{App, ContextAction, ContextMenuAnchor, ContextMenuEntry, PanelFocus};
use mbv_core::api::EmbyItem;

impl App {
    // --- Context menu framing (formerly `input_context_menu.rs`) -----------
    //
    // Builds the menu content and raises it through `pending_overlay`; the
    // shell mounts the `ContextMenuComponent` and owns placement (task 5.3c).
    // `App::context_menu` and `layout.context_menu_rect` are gone.

    fn push_context_action(
        entries: &mut Vec<ContextMenuEntry>,
        label: &'static str,
        action: ContextAction,
    ) {
        entries.push(ContextMenuEntry {
            label,
            action: Some(action),
        });
    }

    /// Build the context menu for the current panel/destination, or `None`
    /// when no menu applies or it would be empty.
    ///
    /// `home_cw_selected` is the authoritative "is Continue Watching selected?"
    /// fact, resolved by the shell from the mounted `HomeComponent` (task
    /// 5.3d, Home context-menu section decoupling) — never copied into an App
    /// field. It is consulted only by the Queue-focus arm below (the odd
    /// queue-menu coupling): with the Queue panel focused while Home is the
    /// active Tab selection, "Remove from Continue Watching" appears exactly
    /// when the Home component has Continue Watching selected.
    ///
    /// `cw_item` is the resolved Continue Watching column item (Model-owned
    /// `home_content.continue_items[continue_cursor]`, resolved at the Model
    /// boundary, task 5.3d) that the Home arm builds its entries from — the
    /// App no longer holds the deleted `home.continue_items` to re-read.
    fn build_context_menu(
        &mut self,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) -> Option<ContextMenu> {
        self.build_context_menu_for(None, home_cw_selected, cw_item)
    }

    /// `build_context_menu` with an explicitly resolved Emby-library item
    /// (task 5.3d, Album track focus): while an inline album track is
    /// focused, the shell resolves the track (the component owns the cursor)
    /// and passes it here so '.' targets the focused track instead of the
    /// album row. All other arms resolve exactly as `build_context_menu`.
    fn build_context_menu_for(
        &mut self,
        tracked_item: Option<EmbyItem>,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) -> Option<ContextMenu> {
        let mut entries = Vec::new();
        let cw_focused = matches!(
            self.effective_panel_focus(),
            crate::app::PanelFocus::Library
        ) && self.tab.is_home();
        let (current_item, queue_cursor) =
            self.resolve_context_menu_target(tracked_item, cw_item)?;

        if let Some(item) = current_item.as_ref() {
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

        if entries.iter().all(|entry| entry.action.is_none()) {
            return None;
        }

        let anchor = match self.effective_panel_focus() {
            crate::app::PanelFocus::Library => {
                ContextMenuAnchor::SelectedItem(crate::app::PanelFocus::Library)
            }
            crate::app::PanelFocus::Queue => {
                ContextMenuAnchor::SelectedItem(crate::app::PanelFocus::Queue)
            }
        };
        Some(ContextMenu {
            anchor,
            cursor: ContextMenu::first_selectable(&entries),
            entries,
        })
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
            (crate::app::PanelFocus::Library, crate::app::TabSelection::Home) => cw_item,
            (crate::app::PanelFocus::Library, crate::app::TabSelection::EmbyLibrary(lib_idx)) => {
                tracked_item.or_else(|| {
                    let cursor = self
                        .libs
                        .get(lib_idx)
                        .and_then(|lib| lib.nav_stack.last())
                        .map_or(0, |level| level.resting().cursor());
                    self.current_lib_item(lib_idx, cursor)
                })
            }
            (
                crate::app::PanelFocus::Library,
                crate::app::TabSelection::AudiobookshelfLibrary(_)
                | crate::app::TabSelection::Feeds,
            ) => return None,
            (crate::app::PanelFocus::Queue, _) => {
                let cursor = self.displayed_queue().queue_cursor;
                queue_cursor = Some(cursor);
                self.displayed_queue().clone_emby_item_at(cursor)
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
        if App::context_menu_play_state(item) {
            Self::push_context_action(
                entries,
                "Mark Unwatched",
                ContextAction::MarkUnplayed(item.id.clone()),
            );
        } else {
            Self::push_context_action(
                entries,
                "Mark Watched",
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
            || !matches!(self.effective_panel_focus(), crate::app::PanelFocus::Queue)
        {
            Self::push_context_action(entries, "Add to Queue", ContextAction::Enqueue);
        }
        // Audio items (music tracks) don't get mark-played.
        if item.media_type != "Audio" && item.item_type != "Audio" {
            Self::push_play_state_context_action(entries, item);
        }
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
        if !cw_focused && matches!(self.effective_panel_focus(), crate::app::PanelFocus::Queue) {
            let pos = self.displayed_queue().queue_cursor;
            Self::push_context_action(
                entries,
                "Remove from Queue",
                ContextAction::RemoveFromQueue(pos),
            );
        }
        if matches!(self.effective_panel_focus(), crate::app::PanelFocus::Queue) {
            Self::push_context_action(
                entries,
                "Go to Library",
                ContextAction::GoToLibrary(item.id.clone(), item.item_type.clone()),
            );
        }
    }

    pub(in crate::app) fn open_feeds_context_menu(
        &mut self,
        entries: Vec<mbv_core::playback_queue::FeedEntry>,
        anchor: Option<(u16, u16)>,
    ) {
        if entries.is_empty() {
            return;
        }
        let mut menu_entries = Vec::new();
        Self::push_context_action(
            &mut menu_entries,
            "Play",
            ContextAction::FeedsPlay(entries.clone()),
        );
        Self::push_context_action(
            &mut menu_entries,
            "Add to Queue",
            ContextAction::FeedsEnqueue(entries.clone()),
        );
        Self::push_context_action(
            &mut menu_entries,
            "Mark Played",
            ContextAction::FeedsMarkPlayed(entries.clone()),
        );
        Self::push_context_action(
            &mut menu_entries,
            "Mark Unplayed",
            ContextAction::FeedsMarkUnplayed(entries),
        );
        let menu = ContextMenu {
            anchor: anchor.map_or(
                ContextMenuAnchor::SelectedItem(PanelFocus::Library),
                |(x, y)| ContextMenuAnchor::Pointer { x, y },
            ),
            cursor: 0,
            entries: menu_entries,
        };
        self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
    }

    /// Keyboard '.' entry (the shared `handle_global_view_key` front door
    /// reached by Home/library/queue views). `home_cw_selected` is the
    /// authoritative Continue-Watching-selected fact, threaded from the shell
    /// (which resolves it from the mounted `HomeComponent`) and reaches this
    /// method via the typed path the central keyboard router dispatches
    /// (task 5.3d, Home context-menu section decoupling; design §5).
    /// It is load-bearing under Queue panel focus while Home is
    /// the active Tab selection; the `self.tab.is_home()` guard short-circuits
    /// it on all other paths.
    pub(in crate::app) fn open_context_menu(
        &mut self,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) {
        if let Some(menu) = self.build_context_menu(home_cw_selected, cw_item) {
            self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
        }
    }

    /// Open the context menu targeted at an explicitly resolved item (a
    /// focused inline album track reached through the shell boundary). This
    /// is never a Home-tab menu, so `home_cw_selected` is a harmless `false`
    /// and `cw_item` a harmless `None` (the `self.tab.is_home()` guard
    /// short-circuits both).
    pub(in crate::app) fn open_context_menu_for(&mut self, item: EmbyItem) {
        if let Some(menu) = self.build_context_menu_for(Some(item), false, None) {
            self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
        }
    }

    /// Build the common multi-selection menu. Capability derivation is an
    /// intersection: an action is present only when every selected item has
    /// the corresponding backend.
    pub(in crate::app) fn open_context_menu_for_selection(
        &mut self,
        items: &[EmbyItem],
        anchor: Option<(u16, u16)>,
        focus: PanelFocus,
        capabilities: Vec<ItemCapabilities>,
        remove_targets: Vec<BulkRemoveTarget>,
    ) {
        let Some(capabilities) =
            crate::app::state::context_menu_capabilities::intersect(capabilities)
        else {
            return;
        };
        let mut entries = Vec::new();
        if matches!(focus, PanelFocus::Library)
            && capabilities.playable
            && capabilities.queue_admissible
        {
            Self::push_context_action(
                &mut entries,
                "Play",
                ContextAction::PlaySelection(items.to_vec()),
            );
            Self::push_context_action(
                &mut entries,
                "Shuffle",
                ContextAction::ShuffleSelection(items.to_vec()),
            );
            Self::push_context_action(
                &mut entries,
                "Add to Queue",
                ContextAction::EnqueueSelection(items.to_vec()),
            );
        }
        if capabilities.removable && !remove_targets.is_empty() {
            Self::push_context_action(
                &mut entries,
                "Remove",
                ContextAction::RemoveSelection(remove_targets),
            );
        }
        let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
        if !ids.is_empty() && capabilities.played_state_capable {
            Self::push_context_action(
                &mut entries,
                "Mark Played",
                ContextAction::MarkPlayedSelection(ids.clone()),
            );
            Self::push_context_action(
                &mut entries,
                "Mark Unplayed",
                ContextAction::MarkUnplayedSelection(ids),
            );
        }
        if entries.is_empty() {
            return;
        }
        self.pending_overlay = Some(OverlayRequest::ContextMenu(ContextMenu {
            anchor: anchor.map_or(ContextMenuAnchor::SelectedItem(focus), |(x, y)| {
                ContextMenuAnchor::Pointer { x, y }
            }),
            cursor: 0,
            entries,
        }));
    }

    /// [`open_context_menu_for`](Self::open_context_menu_for) anchored at a
    /// pointer position (narrow grouped-Music album right-click).
    pub(in crate::app) fn open_context_menu_for_at(&mut self, item: EmbyItem, x: u16, y: u16) {
        if let Some(mut menu) = self.build_context_menu_for(Some(item), false, None) {
            menu.anchor = ContextMenuAnchor::Pointer { x, y };
            self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
        }
    }

    /// Pointer right-click entry. `home_cw_selected` is the authoritative
    /// Continue-Watching-selected fact resolved by the shell from the mounted
    /// `HomeComponent` (task 5.3d, Home context-menu section decoupling). On
    /// the Home/Queue right-click paths it is genuinely load-bearing for the
    /// Queue-focus coupling above; for non-Home right-clicks it is a harmless
    /// `false` (the `self.tab.is_home()` guard already short-circuits it).
    /// `cw_item` is the resolved Continue Watching column item for the Home
    /// arm (task 5.3d); the Queue-focus right-click path (which renders the
    /// queue item, not the CW item) passes `None` — execution resolves it at
    /// the Model boundary instead.
    pub(in crate::app) fn open_context_menu_at(
        &mut self,
        x: u16,
        y: u16,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) {
        self.open_context_menu_at_for_item(x, y, home_cw_selected, cw_item, None);
    }

    pub(in crate::app) fn open_context_menu_at_for_item(
        &mut self,
        x: u16,
        y: u16,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
        tracked_item: Option<EmbyItem>,
    ) {
        let Some(mut menu) = self.build_context_menu_for(tracked_item, home_cw_selected, cw_item)
        else {
            return;
        };
        menu.anchor = ContextMenuAnchor::Pointer { x, y };
        self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
    }
}
