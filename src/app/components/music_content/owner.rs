// The module's documentation and shared imports live in the parent module.
use super::{
    AlbumCursorKind, EmbyItem, EmbySelectorKey, HeroContentData, HeroImageState, InlineSearch,
    InlineSearchHost, Key, KeyEvent, KeyModifiers, LeafKeyResult, LibraryContentOwner,
    LibraryItemIdentity, LibraryPanelContent, LibrarySlotEvent, MediaListSurfaceInput, Msg,
    MusicContent, MusicTreeAction, MusicTreeTarget, RowIntent, SelectorIdentity, ShellRequest,
    TreeConsumed, TreeOperation,
};

impl InlineSearchHost for MusicContent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }
    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }

    fn uses_local_filter(&self) -> bool {
        true
    }

    fn open_inline_search(&mut self) {
        if !self.inline_search.is_active() {
            self.inline_search.open();
            // Arming the shared filter session anchors it on the current
            // selection and shows the complete tree for the empty query.
            self.browser.apply(TreeOperation::EditFilter(String::new()));
        }
    }

    fn close_inline_search(&mut self) {
        if self.inline_search.is_active() {
            self.inline_search.close();
            self.browser.apply(TreeOperation::ClearFilter);
        }
    }

    fn inline_search_debounced(&mut self) {
        if !self.browser.filter_active() {
            return;
        }
        let query = self.inline_search.query().to_string();
        self.browser.apply(TreeOperation::EditFilter(query));
    }
}

/// The plain Music content owner. Its tree browser owns the Grouped Music
/// album selection, expansion, viewport, and marks (one owner across Wide,
/// Narrow, and Mini); the track `MediaList` carrier retains its Workspace
/// cursor/scroll/selection locally. Shell pushes replace only the content
/// snapshot and never mirror those interaction values.
impl LibraryContentOwner for MusicContent {
    fn clear_selection(&mut self) {
        // The destination switch clears the shared owner's ordered marks so
        // no stale selection mark survives into a new destination.
        self.browser.apply(TreeOperation::ClearMarks);
    }

    fn double_click_opens_hero_overlay(&mut self) -> bool {
        // Grouped Music resolves double-clicks in the tree owner: expandable
        // rows toggle locally and track rows retain their pre-U4 no-op path,
        // so the panel must not pre-empt them with a Hero overlay.
        false
    }

    fn hero_overlay_target_available(&mut self) -> bool {
        // Both hero-bearing tree rows can own the overlay before their Hero
        // snapshot materializes: an album leaf and an artist root.
        self.selected_item().is_some() || self.selected_is_artist()
    }

    fn hero_overlay_enter_available(&mut self) -> bool {
        // An unfiltered artist root enters the same Hero/Workspace path as
        // Right on an expanded root. While filtering, Enter remains local to
        // the tree so the panel cannot bypass the filter interaction.
        (!self.browser.filter_active() && self.selected_is_artist())
            || self.selected_item().is_some()
    }

    fn inline_search_session(&mut self) -> Option<&mut dyn InlineSearchHost> {
        Some(self)
    }

    fn inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        Some(self)
    }

    fn set_selection_origin(
        &mut self,
        origin: crate::app::components::media_list::SelectionOrigin,
    ) {
        // The panel's active-owner projection records the stable identity
        // used by both the status pill and bulk-action clear routing. Direct
        // tree actions carry the same identity without exposing membership.
        self.selection_origin = Some(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
        // The tree keeps membership locally; expose only the same read-only
        // count/origin projection used by every canonical list. Music rows
        // never inspect played/unplayed state here.
        let count = self.selected_album_targets().len();
        let origin = self.selection_origin.clone()?;
        (count > 0)
            .then_some(crate::app::components::media_list::SelectionSummary { count, origin })
    }

    fn launch_selector(
        &self,
        state: &mbv_core::config::TuiLaunchState,
    ) -> Option<crate::app::components::library_panel::owner::LaunchSelector> {
        let target = self.group_cursor_for_launch_state(state);
        (self.context.group_cursor != target).then_some(
            crate::app::components::library_panel::owner::LaunchSelector::Emby { index: target },
        )
    }

    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        if self.context.list.loading && self.context.list.items.is_empty() {
            return false;
        }
        let saved_latest = matches!(
            state.selector.as_ref(),
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Latest,
            })
        );
        if !self.context.groups.is_empty() {
            self.context.group_cursor = self.group_cursor_for_launch_state(state);
        }
        // Latest is not a Music selector. A legacy saved Latest selector
        // falls back to the first normal group and its default item.
        let selected = if saved_latest {
            false
        } else {
            match state.item.as_ref() {
                Some(LibraryItemIdentity::Emby { id }) => {
                    self.browser
                        .apply(TreeOperation::AnchorSelection {
                            target: MusicTreeTarget::Album(id.clone()),
                            flow_offset: 0,
                        })
                        .disposition
                        == TreeConsumed::Consumed
                }
                _ => false,
            }
        };
        if !selected {
            if let Some(target) = self.context.album_targets.first().cloned() {
                self.browser.apply(TreeOperation::AnchorSelection {
                    target: MusicTreeTarget::Album(target),
                    flow_offset: 0,
                });
            } else {
                self.browser.apply(TreeOperation::First);
            }
        }
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let selector = self
            .context
            .groups
            .get(self.context.group_cursor)
            .cloned()
            .map(|group| SelectorIdentity::Emby {
                key: EmbySelectorKey::Group(group.id),
            });
        let item = self
            .selected_album_target()
            .map(|id| LibraryItemIdentity::Emby { id });
        (selector, item)
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.panel_content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.on_slot_event(event)
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.inline_search.is_active() {
            return self.on_key_inline_search(key);
        }
        // The LibraryPanel is the framework focus boundary; reaching this
        // method already proves Music is focused.
        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused() {
            return self.on_key_ctrl_chord(key);
        }
        if let KeyOutcome::Handled(result) = self.on_key_track_focused(key) {
            return result;
        }
        if let KeyOutcome::Handled(result) = self.on_key_library_command(key) {
            return result;
        }
        match self.on_key_album_navigation(key) {
            KeyOutcome::Handled(result) => result,
            KeyOutcome::Unhandled => match key.code {
                Key::Enter => self.on_key_enter(),
                // Right on an album node expands only cached track children;
                // missing data remains the shell's responsibility.
                Key::Right if !self.track_focused() && !self.selected_is_artist() => {
                    if let Some(target) = self.browser.selected_target().cloned() {
                        if !self.browser.is_expanded(&target) {
                            self.browser
                                .apply(TreeOperation::ToggleExpansionTarget(target));
                        }
                    }
                    None
                }
                // Right on an artist root expands first, then enters its
                // Workspace on a later keypress.
                Key::Right if !self.track_focused() && self.selected_is_artist() => {
                    self.tree_right_artist()
                }
                _ => None,
            },
        }
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        let active = self.inline_search.is_active();
        match self.on_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None if (active
                && matches!(
                    key.code,
                    Key::Esc
                        | Key::Enter
                        | Key::Backspace
                        | Key::Up
                        | Key::Down
                        | Key::Left
                        | Key::Right
                        | Key::Char(_)
                ))
                || (self.track_focused()
                    && matches!(
                        key.code,
                        Key::Enter
                            | Key::Esc
                            | Key::Backspace
                            | Key::Up
                            | Key::Down
                            | Key::Char('k' | 'j' | '.')
                    )) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    fn inline_search_active(&self) -> bool {
        self.inline_search.is_active()
    }

    fn focus_hero_workspace(&mut self) -> bool {
        self.enter_track_focus();
        true
    }

    fn clear_hero_workspace_focus(&mut self) {
        self.clear_track_focus();
    }

    fn set_hero_overlay_open(&mut self, open: bool) {
        // The overlay takes the Workspace focus exactly once, on its open
        // transition (bit false->true) -- never again on the sync pass's
        // bit re-assert or an ordinary push, so shell and local focus
        // clears stay authoritative while the overlay is open.
        if open && !self.hero_overlay_open {
            self.enter_track_focus();
        }
        self.hero_overlay_open = open;
    }

    fn post_paint_message(&mut self) -> Option<Msg> {
        // Task 6.5 (design D4): the owner translates the neighbour album
        // artwork window from the frame the shared browser just painted; the
        // shell applies the existing idle gate and fetches the typed targets.
        // Source pagination for this album level is unconditional and no
        // longer depends on the painted viewport.
        self.neighbour_prefetch_targets()
            .map(|targets| Msg::Shell(Box::new(ShellRequest::MusicNeighbourPrefetch { targets })))
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.set_hero_image(state);
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Outcome of a key handler: whether the key was consumed, and the optional
/// message a consumed key emits.
enum KeyOutcome {
    Unhandled,
    Handled(Option<Msg>),
}

impl KeyOutcome {
    fn or_else(self, f: impl FnOnce() -> KeyOutcome) -> KeyOutcome {
        match self {
            KeyOutcome::Handled(message) => KeyOutcome::Handled(message),
            KeyOutcome::Unhandled => f(),
        }
    }
}

impl MusicContent {
    fn group_cursor_for_launch_state(&self, state: &mbv_core::config::TuiLaunchState) -> usize {
        match state.selector.as_ref() {
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Group(id),
            }) => self
                .context
                .groups
                .iter()
                .position(|group| &group.id == id)
                .unwrap_or(0),
            _ => 0,
        }
    }
    /// Track-pane key handling that takes precedence over album navigation.
    /// `Unhandled` distinguishes an unhandled key from a handled key whose
    /// local action emits no message.
    fn on_key_track_focused(&mut self, key: &KeyEvent) -> KeyOutcome {
        self.on_key_track_focus_exit(key)
            .or_else(|| self.on_key_track_move(key))
            .or_else(|| self.on_key_track_overlay_navigation(key))
            .or_else(|| self.on_key_track_context_menu(key))
    }

    fn on_key_track_focus_exit(&mut self, key: &KeyEvent) -> KeyOutcome {
        match key.code {
            Key::Esc | Key::Backspace if self.track_focused() => {
                self.clear_track_focus();
                KeyOutcome::Handled(None)
            }
            _ => KeyOutcome::Unhandled,
        }
    }

    fn on_key_track_move(&mut self, key: &KeyEvent) -> KeyOutcome {
        match key.code {
            Key::Up | Key::Char('k') if self.track_focused() => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Move(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                KeyOutcome::Handled(None)
            }
            Key::Down | Key::Char('j') if self.track_focused() => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Move(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                KeyOutcome::Handled(None)
            }
            _ => KeyOutcome::Unhandled,
        }
    }

    /// Overlay pager/jump chords move the focused track list, never the
    /// covered album browser. Outside this guard, album navigation still sees
    /// Home/End/PageUp/PageDown even while a track is focused.
    fn on_key_track_overlay_navigation(&mut self, key: &KeyEvent) -> KeyOutcome {
        match key.code {
            Key::PageUp if self.hero_overlay_open && self.track_focused() => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Page(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                KeyOutcome::Handled(None)
            }
            Key::PageDown if self.hero_overlay_open && self.track_focused() => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Page(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                KeyOutcome::Handled(None)
            }
            Key::Home if self.hero_overlay_open && self.track_focused() => {
                self.track_list.select_first();
                KeyOutcome::Handled(None)
            }
            Key::End if self.hero_overlay_open && self.track_focused() => {
                self.track_list.select_last();
                KeyOutcome::Handled(None)
            }
            _ => KeyOutcome::Unhandled,
        }
    }

    /// The focused track's own menu wins over the album menu.
    fn on_key_track_context_menu(&mut self, key: &KeyEvent) -> KeyOutcome {
        match key.code {
            Key::Char('.') if self.track_focused() => {
                KeyOutcome::Handled(self.workspace_track_context_menu())
            }
            _ => KeyOutcome::Unhandled,
        }
    }

    /// Search, context-menu, refresh, and selector-row commands.
    fn on_key_library_command(&mut self, key: &KeyEvent) -> KeyOutcome {
        match key.code {
            Key::Char('/') => {
                if !self.inline_search.is_active() {
                    self.inline_search.open();
                    self.browser.apply(TreeOperation::EditFilter(String::new()));
                }
                KeyOutcome::Handled(Some(Msg::Shell(Box::new(ShellRequest::OpenInlineSearch))))
            }
            Key::Char('.') => KeyOutcome::Handled(self.album_context_menu_msg()),
            Key::Char('r')
                if !self.track_focused()
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                KeyOutcome::Handled(Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryRefresh))))
            }
            Key::Char('[' | ']')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !self.context.groups.is_empty() =>
            {
                let count = self.context.groups.len();
                let current = self.context.group_cursor;
                let delta = if key.code == Key::Char('[') { -1 } else { 1 };
                let count = i64::try_from(count).unwrap_or(i64::MAX);
                let current = i64::try_from(current).unwrap_or(i64::MAX);
                let next =
                    usize::try_from((current + delta).rem_euclid(count)).unwrap_or(usize::MAX);
                KeyOutcome::Handled(self.on_slot_event(LibrarySlotEvent::SelectorPicked(next)))
            }
            _ => KeyOutcome::Unhandled,
        }
    }

    /// Album-browser movement and jumps, unless a focused track-pane arm
    /// consumed the key first.
    fn on_key_album_navigation(&mut self, key: &KeyEvent) -> KeyOutcome {
        match key.code {
            Key::Up | Key::Char('k') if !self.track_focused() => {
                KeyOutcome::Handled(self.move_album(-1, AlbumCursorKind::Move))
            }
            Key::Down | Key::Char('j') if !self.track_focused() => {
                KeyOutcome::Handled(self.move_album(1, AlbumCursorKind::Move))
            }
            Key::Home => {
                self.browser.apply(TreeOperation::First);
                KeyOutcome::Handled(self.album_selection_request(AlbumCursorKind::Jump))
            }
            Key::End => {
                self.browser.apply(TreeOperation::Last);
                KeyOutcome::Handled(self.album_selection_request(AlbumCursorKind::Jump))
            }
            Key::PageUp => KeyOutcome::Handled(self.page_album(-1, AlbumCursorKind::Page)),
            Key::PageDown => KeyOutcome::Handled(self.page_album(1, AlbumCursorKind::Page)),
            Key::Left if !self.track_focused() => KeyOutcome::Handled(self.tree_move_left()),
            _ => KeyOutcome::Unhandled,
        }
    }

    /// The inline-search takeover branch of `on_key`: while inline search is
    /// active it owns every key.
    fn on_key_inline_search(&mut self, key: &KeyEvent) -> Option<Msg> {
        // The production Grouped Music session never populates the flat
        // carrier. Keep the legacy host hook usable for focused harnesses
        // that explicitly seed that carrier while exercising unrelated
        // activation plumbing; the panel still always paints the tree.
        if self.local_filter_owns_input() {
            return self.on_filter_key(key);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Some(item) = self.inline_search.selected_item() {
                let request = match key.code {
                    Key::Char('p') => Some(ShellRequest::EmbyLibraryPlay { item }),
                    Key::Char('a') => Some(ShellRequest::EmbyLibraryEnqueue { item }),
                    Key::Char('s') => Some(ShellRequest::EmbyLibraryShuffle { item }),
                    _ => None,
                };
                if let Some(request) = request {
                    self.inline_search.close();
                    self.browser.apply(TreeOperation::ClearFilter);
                    return Some(Msg::Shell(Box::new(request)));
                }
            }
        }
        match self.inline_search.handle_key(key) {
            Some(crate::app::components::inline_search::InlineSearchAction::Activate {
                id,
                item_type,
            }) => Some(Msg::Shell(Box::new(ShellRequest::InlineSearchActivate {
                id,
                item_type,
            }))),
            Some(crate::app::components::inline_search::InlineSearchAction::Dismiss) => {
                self.inline_search.close();
                self.browser.apply(TreeOperation::ClearFilter);
                None
            }
            Some(crate::app::components::inline_search::InlineSearchAction::QueryStarted) => {
                Some(Msg::Shell(Box::new(ShellRequest::InlineSearchQueryStarted)))
            }
            None => None,
        }
    }

    /// The ctrl-chord branch of `on_key` (no inline search, track pane not
    /// locally focused).
    fn on_key_ctrl_chord(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.selected_is_artist() {
            return match key.code {
                Key::Char('p') => self.artist_action(MusicTreeAction::Play),
                Key::Char('a') => self.artist_action(MusicTreeAction::Enqueue),
                Key::Char('s') => self.artist_action(MusicTreeAction::Shuffle),
                // Artist roots are grouping rows, so watched-state is
                // unavailable while the library-wide rescan remains
                // available from every focused library row.
                Key::Char('r') => Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryRescan))),
                _ => None,
            };
        }
        let item = self.selected_item();
        match key.code {
            Key::Char('p') => {
                item.map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryPlay { item })))
            }
            Key::Char('a') => {
                item.map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryEnqueue { item })))
            }
            Key::Char('s') => {
                item.map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryShuffle { item })))
            }
            Key::Char('w') => item
                .map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryToggleWatched { item }))),
            Key::Char('r') => Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryRescan))),
            _ => None,
        }
    }

    /// The `Enter` family of `on_key`, in the original arm order.
    fn on_key_enter(&mut self) -> Option<Msg> {
        // Unfiltered artist Enter uses the same Hero entry as Right on an
        // expanded root in Wide geometry. The panel owns the non-Wide
        // Enter interception and opens the Library Hero overlay before
        // this owner sees the chord; the filter keeps its local expansion
        // behavior in `on_filter_key`. Once the focused pane is this
        // root's own artist Workspace, Enter belongs to the focused track
        // below.
        if self.selected_is_artist() && !self.artist_workspace_focused() {
            if self.inline_track_focus_enabled {
                self.enter_artist_workspace_focus();
            }
            return None;
        }
        if self.track_focused() {
            return self.workspace_track_activation();
        }
        if self.selected_is_track() {
            let (album_target, track_id) = self.selected_tree_track()?;
            return Some(Msg::Shell(Box::new(ShellRequest::MusicTreeTrackActivate {
                album_target,
                track_id,
            })));
        }
        if self.track_list.rows().is_empty() {
            return self
                .selected_item()
                .map(|item| Msg::Shell(Box::new(ShellRequest::MusicAlbumActivate { item })));
        }
        // Narrow geometry has no inline track pane: the chord opens (or
        // re-focuses) the Library Hero overlay instead of focusing a list
        // nothing paints.
        if self.inline_track_focus_enabled {
            self.enter_track_focus();
            return None;
        }
        self.selected_item()
            .map(|item| Msg::Shell(Box::new(ShellRequest::MusicAlbumActivate { item })))
    }

    /// `.` on the focused track pane: the track list's own context menu.
    fn workspace_track_context_menu(&mut self) -> Option<Msg> {
        match self
            .track_list
            .delegate_operation(
                MediaListSurfaceInput::Context
                    .into_operation(None)
                    .expect("resolved media-list pointer target"),
            )
            .external_intent
        {
            Some(RowIntent::ContextSelection(targets)) => {
                let items: Vec<EmbyItem> = targets
                    .into_iter()
                    .filter_map(|target| self.workspace_track_item(&target))
                    .collect();
                (!items.is_empty()).then_some(Msg::Shell(Box::new(
                    ShellRequest::MusicRowContextMenu(
                        crate::app::state::types::context_menu::ContextMenuTargets::Emby(items),
                        None,
                    ),
                )))
            }
            Some(RowIntent::Context(target)) => self.workspace_track_item(&target).map(|track| {
                Msg::Shell(Box::new(ShellRequest::MusicRowContextMenu(
                    crate::app::state::types::context_menu::ContextMenuTargets::Emby(vec![track]),
                    None,
                )))
            }),
            _ => None,
        }
    }

    /// `.` on the album/tree pane: the artist's or album's context menu.
    fn album_context_menu_msg(&mut self) -> Option<Msg> {
        if self.selected_is_artist() {
            let (items, _unresolved_targets) = self.selected_artist_items()?;
            if items.is_empty() {
                return None;
            }
            Some(Msg::Shell(Box::new(ShellRequest::MusicRowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Emby(items),
                None,
            ))))
        } else {
            self.selected_item().map(|item| {
                Msg::Shell(Box::new(ShellRequest::MusicRowContextMenu(
                    crate::app::state::types::context_menu::ContextMenuTargets::Emby(vec![item]),
                    None,
                )))
            })
        }
    }

    /// Left in the tree (task 2.4): collapse a focused expanded artist root,
    /// or return a leaf to its artist parent.
    fn tree_move_left(&mut self) -> Option<Msg> {
        if self.selected_is_artist() {
            if let Some(root) = self.browser.selected_target().cloned() {
                if self.browser.is_expanded(&root) {
                    self.browser
                        .apply(TreeOperation::ToggleExpansionTarget(root));
                }
            }
            return None;
        }
        if self
            .browser
            .selected_target()
            .and_then(|selected| self.browser.node(selected))
            .and_then(|node| node.parent.clone())
            .is_some()
        {
            // The parent is an artist root, so no album selection
            // crosses: artist focus never overwrites album persistence.
            self.browser.apply(TreeOperation::Parent);
            self.album_selection_request(AlbumCursorKind::Move)
        } else {
            None
        }
    }

    /// Right on an artist root (task 2.4/task 6.4): a collapsed root
    /// expands first; only a later Right on the already expanded root
    /// enters its artist Workspace — Wide takes the inline pane's
    /// cursor locally, non-Wide asks the shell to open the Library
    /// Hero overlay and focus the same Workspace.
    fn tree_right_artist(&mut self) -> Option<Msg> {
        let root = self.browser.selected_target().cloned()?;
        if !self.browser.is_expanded(&root) {
            self.browser
                .apply(TreeOperation::ToggleExpansionTarget(root));
            return None;
        }
        if self.inline_track_focus_enabled {
            self.enter_artist_workspace_focus();
            return None;
        }
        self.artist_detail_target()
            .map(|target| Msg::Shell(Box::new(ShellRequest::MusicArtistActivate { target })))
    }
}
