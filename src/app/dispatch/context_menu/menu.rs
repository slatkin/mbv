use crate::app::state::context_menu_capabilities::ItemCapabilities;
use crate::app::{App, ContextAction, ContextMenuAnchor, ContextMenuEntry, PanelFocus};
use mbv_emby_model::EmbyItem;
use mbv_render::layout::rect_to_presentation;
use mbv_ui_model::context_menu::AudiobookshelfMenuTarget;
use mbv_ui_model::context_menu::BulkRemoveTarget;
use mbv_ui_model::context_menu::ContextMenu;
use mbv_ui_model::overlay::OverlayRequest;

mod emby;

/// The anchor a selection menu opens at: the selected row of `focus`. A
/// pointer-anchored menu (a right-click) sets the anchor at its own raise site,
/// so no core builder takes a pointer cell (design D6).
pub(super) fn menu_anchor(focus: PanelFocus) -> ContextMenuAnchor {
    ContextMenuAnchor::SelectedItem(focus)
}

impl App {
    // --- Context menu framing (formerly `input_context_menu.rs`) -----------
    //
    // Builds the menu content and raises it through `pending_overlay`; the
    // shell mounts the `ContextMenuComponent` and owns placement (task 5.3c).
    // `App::context_menu` and `layout.context_menu_rect` are gone.

    pub(super) fn push_context_action(
        entries: &mut Vec<ContextMenuEntry>,
        label: &'static str,
        action: ContextAction,
    ) {
        entries.push(ContextMenuEntry {
            label,
            action: Some(action),
            key_hint: None,
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
    /// `cw_item` is the resolved Continue Watching column item (the
    /// App-stored `home_content.continue_items[continue_cursor]`, resolved at
    /// the Model boundary, task 5.3d) that the Home arm builds its entries
    /// from — the App no longer holds the deleted `home.continue_items` to
    /// re-read.
    fn build_context_menu(
        &mut self,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) -> Option<ContextMenu> {
        self.build_context_menu_for(None, home_cw_selected, cw_item)
    }

    pub(in crate::app) fn open_feeds_context_menu(&mut self, entries: &[mbv_queue::FeedEntry]) {
        if entries.is_empty() {
            return;
        }
        let menu_entries = Self::build_feeds_menu_entries(entries);
        let menu = ContextMenu {
            anchor: menu_anchor(PanelFocus::Library),
            cursor: 0,
            entries: menu_entries,
            bounds: None,
        };
        self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
    }

    /// The Feeds row menu's standard entries for `entries` (standard-media-
    /// context-menus design D3), shared by the TUI's opener and the GUI
    /// Library panel's carried row menu: the split is on entry count, never
    /// on presentation.
    pub(in crate::app) fn build_feeds_menu_entries(
        entries: &[mbv_queue::FeedEntry],
    ) -> Vec<ContextMenuEntry> {
        let entries = entries.to_vec();
        let mut menu_entries = Vec::new();
        // Standard action set split on entry count (context-menu spec,
        // standard-media-context-menus design D3): a leaf row offers Play,
        // Add to Queue, then the one entry that changes its state; a
        // multi-selection offers Play, Shuffle, Add to Queue, and both mark
        // entries.
        Self::push_context_action(
            &mut menu_entries,
            "Play",
            ContextAction::FeedsPlay(entries.clone()),
        );
        if entries.len() > 1 {
            Self::push_context_action(
                &mut menu_entries,
                "Shuffle",
                ContextAction::FeedsShuffle(entries.clone()),
            );
        }
        Self::push_context_action(
            &mut menu_entries,
            "Add to Queue",
            ContextAction::FeedsEnqueue(entries.clone()),
        );
        if let [entry] = entries.as_slice() {
            if entry.played {
                Self::push_context_action(
                    &mut menu_entries,
                    "Mark Unplayed",
                    ContextAction::FeedsMarkUnplayed(entries),
                );
            } else {
                Self::push_context_action(
                    &mut menu_entries,
                    "Mark Played",
                    ContextAction::FeedsMarkPlayed(entries),
                );
            }
        } else {
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
        }
        menu_entries
    }

    /// Build the Audiobookshelf row menu from component-resolved targets
    /// (standard-media-context-menus design D3). The standard action set
    /// splits on target count: a leaf row offers Play, Add to Queue, then the
    /// one mark entry its cached progress state chooses (a missing entry
    /// counts as unfinished); a multi-selection adds Shuffle and offers both
    /// mark entries.
    pub(in crate::app) fn open_audiobookshelf_context_menu(
        &mut self,
        targets: &[AudiobookshelfMenuTarget],
    ) {
        if targets.is_empty() {
            return;
        }
        let menu_entries = self.build_audiobookshelf_menu_entries(targets);
        let menu = ContextMenu {
            anchor: menu_anchor(PanelFocus::Library),
            cursor: ContextMenu::first_selectable(&menu_entries),
            entries: menu_entries,
            bounds: None,
        };
        self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
    }

    /// The Audiobookshelf row menu's standard entries for `targets`
    /// (standard-media-context-menus design D3), shared by the TUI's opener
    /// and the GUI Library panel's carried row menu: a leaf row offers the one
    /// mark entry its cached progress state chooses, and a multi-selection
    /// adds Shuffle and both mark entries.
    pub(in crate::app) fn build_audiobookshelf_menu_entries(
        &self,
        targets: &[AudiobookshelfMenuTarget],
    ) -> Vec<ContextMenuEntry> {
        let targets = targets.to_vec();
        let mut menu_entries = Vec::new();
        Self::push_context_action(
            &mut menu_entries,
            "Play",
            ContextAction::AudiobookshelfPlay(targets.clone()),
        );
        if targets.len() > 1 {
            Self::push_context_action(
                &mut menu_entries,
                "Shuffle",
                ContextAction::AudiobookshelfShuffle(targets.clone()),
            );
        }
        Self::push_context_action(
            &mut menu_entries,
            "Add to Queue",
            ContextAction::AudiobookshelfEnqueue(targets.clone()),
        );
        if let [target] = targets.as_slice() {
            if self.audiobookshelf_target_finished(target) {
                Self::push_context_action(
                    &mut menu_entries,
                    "Mark Unplayed",
                    ContextAction::AudiobookshelfMarkUnplayed(targets),
                );
            } else {
                Self::push_context_action(
                    &mut menu_entries,
                    "Mark Played",
                    ContextAction::AudiobookshelfMarkPlayed(targets),
                );
            }
        } else {
            Self::push_context_action(
                &mut menu_entries,
                "Mark Played",
                ContextAction::AudiobookshelfMarkPlayed(targets.clone()),
            );
            Self::push_context_action(
                &mut menu_entries,
                "Mark Unplayed",
                ContextAction::AudiobookshelfMarkUnplayed(targets),
            );
        }
        menu_entries
    }

    /// Cached finished state for one menu target across every Audiobookshelf
    /// browse state. A missing progress entry counts as unfinished (design
    /// D3), matching how the server reports items without listening progress.
    pub(in crate::app) fn audiobookshelf_target_finished(
        &self,
        target: &AudiobookshelfMenuTarget,
    ) -> bool {
        match target {
            AudiobookshelfMenuTarget::Episode {
                library_item_id,
                episode_id,
            } => self
                .audiobookshelf_cached_episode_progress(library_item_id, episode_id)
                .is_some_and(|progress| progress.is_finished),
            AudiobookshelfMenuTarget::Book { library_item_id } => self
                .audiobookshelf_cached_book_progress(library_item_id)
                .is_some_and(|progress| progress.is_finished),
        }
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
        focus: PanelFocus,
        capabilities: Vec<ItemCapabilities>,
        remove_targets: Vec<BulkRemoveTarget>,
    ) {
        if let Some(menu) = Self::build_selection_menu(capabilities, items, remove_targets, focus) {
            self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
        }
    }

    /// The selection menu without the shell raise: the same entries
    /// `open_context_menu_for_selection` builds, returned instead of raised
    /// through `pending_overlay`. The GUI Client's queue row menu takes it for
    /// a multi-selection right-click (its canvas menu is a per-row menu, so
    /// the shared builder is the only bulk-entry source), and never raises
    /// it — the frame carries the entries to the window content, which
    /// anchors the menu on the first selected slot itself. The TUI's
    /// multi-selection right-click takes it too and sets the pointer anchor
    /// itself before the raise.
    pub(in crate::app) fn build_queue_selection_menu(
        &mut self,
        slot_ids: &[mbv_queue::QueueSlotId],
    ) -> Option<ContextMenu> {
        let link = self.playing_queue_link();
        let (items, remove_targets, capabilities) = slot_ids
            .iter()
            .filter_map(|sid| {
                let slot = self.queue_for_link(link).slot(*sid)?;
                let item = slot.item.as_emby().cloned();
                let remove = mbv_ui_model::context_menu::BulkRemoveTarget::Queue(*sid);
                let capability =
                    crate::app::state::context_menu_capabilities::queue_item_capabilities(
                        &slot.item,
                    );
                Some((item, remove, capability))
            })
            .fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |(mut items, mut removes, mut capabilities), (item, remove, capability)| {
                    if let Some(item) = item {
                        items.push(item);
                    }
                    removes.push(remove);
                    capabilities.push(capability);
                    (items, removes, capabilities)
                },
            );
        Self::build_selection_menu(
            capabilities,
            &items,
            remove_targets,
            mbv_ui_model::settings::PanelFocus::Queue,
        )
    }

    /// The common multi-selection menu's entries, without the shell raise:
    /// capability derivation is an intersection — an action is present only
    /// when every selected item has the corresponding backend.
    pub(in crate::app) fn build_selection_menu(
        capabilities: Vec<ItemCapabilities>,
        items: &[EmbyItem],
        remove_targets: Vec<BulkRemoveTarget>,
        focus: PanelFocus,
    ) -> Option<ContextMenu> {
        let capabilities = crate::app::state::context_menu_capabilities::intersect(capabilities)?;
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
        // List-specific removals always come last (context-menu spec,
        // removals follow the standard entries).
        if capabilities.removable && !remove_targets.is_empty() {
            Self::push_context_action(
                &mut entries,
                "Remove",
                ContextAction::RemoveSelection(remove_targets),
            );
        }
        if entries.is_empty() {
            return None;
        }
        Some(ContextMenu {
            anchor: menu_anchor(focus),
            cursor: 0,
            entries,
            bounds: None,
        })
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

    /// Open the queue visual slot's chrome menu at a pointer anchor: choose
    /// the visualizer or the artwork, or hide the slot. Pointer-only (the
    /// slot has no keyboard anchor and no row targets), so the menu clamps
    /// inside the queue column rather than the destination-derived panel.
    /// The label names the other content, never a toggle.
    pub(in crate::app) fn open_visual_slot_context_menu(&mut self, x: u16, y: u16) {
        if !self.visual_slot_shown() {
            return;
        }
        let mut entries = Vec::new();
        if self.visualizer_enabled {
            Self::push_context_action(&mut entries, "Show Media Art", ContextAction::ShowMediaArt);
        } else {
            Self::push_context_action(
                &mut entries,
                "Show Visualizer",
                ContextAction::ShowVisualizer,
            );
        }
        Self::push_context_action(&mut entries, "Hide", ContextAction::HideVisualSlot);
        let bounds = {
            let area = self.layout.left_area;
            // Neutral presentation `Rect` (`extract-client-core` task 1.2); the
            // render layout converts at the edge.
            (area.width > 0 && area.height > 0).then(|| rect_to_presentation(area))
        };
        self.pending_overlay = Some(OverlayRequest::ContextMenu(ContextMenu {
            anchor: ContextMenuAnchor::Pointer { x, y },
            cursor: 0,
            entries,
            bounds,
        }));
    }
}
