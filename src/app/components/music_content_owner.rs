// Included into `music_content` via `include!` (the module's doc and
// imports live there, beside the split's other parts).

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
            self.browser.open_filter();
        }
    }

    fn close_inline_search(&mut self) {
        if self.inline_search.is_active() {
            self.inline_search.close();
            self.browser.close_filter();
        }
    }

    fn inline_search_debounced(&mut self) {
        let query = self.inline_search.query().to_string();
        self.browser.apply_filter_query(&query);
    }
}

/// The plain Music content owner. Its tree browser owns the Grouped Music
/// album selection, expansion, viewport, and marks (one owner across Wide,
/// Narrow, and Mini); the track `MediaList` carrier retains its Workspace
/// cursor/scroll/selection locally. Shell pushes replace only the content
/// snapshot and never mirror those interaction values.
impl LibraryContentOwner for MusicContent {
    fn clear_selection(&mut self) {
        // The tree's multi-selection is task 4.2 scope; the destination switch
        // clears the browser's stored marks so no stale selection mark
        // survives into a new destination.
        self.browser.clear_marks();
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
        self.selected_item().is_some() || self.browser.selected_is_artist()
    }

    fn hero_overlay_enter_available(&mut self) -> bool {
        // An unfiltered artist root enters the same Hero/Workspace path as
        // Right on an expanded root. While filtering, Enter remains local to
        // the tree so the panel cannot bypass the filter interaction.
        (!self.browser.filter_active()
            && self.browser.selected_is_artist())
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
        let count = self.browser.selected_album_targets().len();
        let origin = self.selection_origin.clone()?;
        (count > 0)
            .then_some(crate::app::components::media_list::SelectionSummary { count, origin })
    }

    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        if self.context.list.loading && self.context.list.items.is_empty() {
            return false;
        }
        if !self.context.groups.is_empty() {
            self.context.group_cursor = match state.selector.as_ref() {
                Some(SelectorIdentity::Emby {
                    key: EmbySelectorKey::Group(id),
                }) => self
                    .context
                    .groups
                    .iter()
                    .position(|group| &group.id == id)
                    .unwrap_or(0),
                _ => 0,
            };
        }
        let selected = match state.item.as_ref() {
            Some(LibraryItemIdentity::Emby { id }) => self.browser.select_album_target(id),
            _ => false,
        };
        if !selected {
            if let Some(target) = self.context.album_targets.first().cloned() {
                self.browser.select_album_target(&target);
            } else {
                self.browser.select_first_visible();
            }
        }
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        // Group pills are Music's main Selector. The artist/album/track tree
        // is Workspace content, so it contributes no selector identity; its
        // selected album target is the stable library-item identity.
        let selector = self
            .context
            .groups
            .get(self.context.group_cursor)
            .cloned()
            .map(|group| SelectorIdentity::Emby {
                key: EmbySelectorKey::Group(group.id),
            });
        let item = self
            .browser
            .selected_album_target()
            .map(str::to_owned)
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
                        self.browser.close_filter();
                        return Some(Msg::Shell(request));
                    }
                }
            }
            return match self.inline_search.handle_key(key) {
                Some(super::inline_search::InlineSearchAction::Activate { id, item_type }) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                        id,
                        item_type,
                    }))
                }
                Some(super::inline_search::InlineSearchAction::Dismiss) => {
                    self.inline_search.close();
                    self.browser.close_filter();
                    None
                }
                Some(super::inline_search::InlineSearchAction::QueryStarted) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchQueryStarted))
                }
                None => None,
            };
        }
        // The LibraryPanel is the framework focus boundary; reaching this
        // method already proves Music is focused.
        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused {
            if self.selected_is_artist() {
                return match key.code {
                    Key::Char('p') => self.artist_action(MusicTreeAction::Play),
                    Key::Char('a') => self.artist_action(MusicTreeAction::Enqueue),
                    Key::Char('s') => self.artist_action(MusicTreeAction::Shuffle),
                    // Artist roots are grouping rows, so watched-state is
                    // unavailable while the library-wide rescan remains
                    // available from every focused library row.
                    Key::Char('w') => None,
                    Key::Char('r') => Some(Msg::Shell(ShellRequest::EmbyLibraryRescan)),
                    _ => None,
                };
            }
            let item = self.selected_item();
            return match key.code {
                Key::Char('p') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryPlay { item }))
                }
                Key::Char('a') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryEnqueue { item }))
                }
                Key::Char('s') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryShuffle { item }))
                }
                Key::Char('w') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryToggleWatched { item }))
                }
                Key::Char('r') => Some(Msg::Shell(ShellRequest::EmbyLibraryRescan)),
                _ => None,
            };
        }
        match key.code {
            // Unfiltered artist Enter uses the same Hero entry as Right on an
            // expanded root in Wide geometry. The panel owns the non-Wide
            // Enter interception and opens the Library Hero overlay before
            // this owner sees the chord; the filter keeps its local expansion
            // behavior in `on_filter_key`. Once the focused pane is this
            // root's own artist Workspace, Enter belongs to the focused track
            // below.
            Key::Enter if self.browser.selected_is_artist() && !self.artist_workspace_focused() => {
                if self.inline_track_focus_enabled {
                    self.enter_artist_workspace_focus();
                }
                None
            }
            Key::Enter if self.track_focused => self.workspace_track_activation(),
            Key::Enter if self.browser.selected_is_track() => {
                let (album_target, track_id) = self.selected_tree_track()?;
                Some(Msg::Shell(ShellRequest::MusicTreeTrackActivate {
                    album_target,
                    track_id,
                }))
            }
            Key::Enter if self.track_list.rows().is_empty() => self
                .selected_item()
                .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item })),
            // Narrow geometry has no inline track pane: the chord opens (or
            // re-focuses) the Library Hero overlay instead of focusing a list
            // nothing paints.
            Key::Enter if self.inline_track_focus_enabled => {
                self.enter_track_focus();
                None
            }
            Key::Enter => self
                .selected_item()
                .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item })),
            Key::Esc | Key::Backspace if self.track_focused => {
                self.clear_track_focus();
                None
            }
            Key::Up | Key::Char('k') if self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Move(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            Key::Down | Key::Char('j') if self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Move(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            // The Library Hero overlay's pager/jump chords move the focused
            // track list, never the covered album browser (the overlay is
            // Narrow-only; Wide keeps the album-rail paging arms below).
            Key::PageUp if self.hero_overlay_open && self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Page(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            Key::PageDown if self.hero_overlay_open && self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Page(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            Key::Home if self.hero_overlay_open && self.track_focused => {
                self.track_list.select_first();
                None
            }
            Key::End if self.hero_overlay_open && self.track_focused => {
                self.track_list.select_last();
                None
            }
            Key::Char('/') => {
                if !self.inline_search.is_active() {
                    self.inline_search.open();
                    self.browser.open_filter();
                }
                Some(Msg::Shell(ShellRequest::OpenInlineSearch))
            }
            // Context menu: the focused track's own menu while the track
            // pane holds local focus, otherwise the selected album's
            // generic library context menu (mirrors the retired
            // `MusicWorkspaceComponent`'s '.' handling).
            Key::Char('.') if self.track_focused => {
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
                        (!items.is_empty()).then_some(Msg::Shell(ShellRequest::MusicRowContextMenu(
                            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                            None,
                        )))
                    }
                    Some(RowIntent::Context(target)) => {
                        self.workspace_track_item(&target).map(|track| {
                            Msg::Shell(ShellRequest::MusicRowContextMenu(
                                crate::app::types_context_menu::ContextMenuTargets::Emby(vec![
                                    track,
                                ]),
                                None,
                            ))
                        })
                    }
                    _ => None,
                }
            }
            Key::Char('.') => {
                if self.selected_is_artist() {
                    let (items, _unresolved_targets) = self.selected_artist_items()?;
                    if items.is_empty() {
                        return None;
                    }
                    Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
                        crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                        None,
                    )))
                } else {
                    self.selected_item().map(|item| {
                        Msg::Shell(ShellRequest::MusicRowContextMenu(
                            crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]),
                            None,
                        ))
                    })
                }
            }
            Key::Char('r')
                if !self.track_focused
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Some(Msg::Shell(ShellRequest::EmbyLibraryRefresh))
            }
            Key::Char('[') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: -1 }))
            }
            Key::Char(']') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: 1 }))
            }
            // Album-level navigation (unfocused track pane): the earlier
            // `self.track_focused` arms above take precedence while the
            // track pane holds local focus. The one tree owner supplies the
            // visible-node movement; a resolved album selection crosses as
            // the existing `MusicAlbumCursor` request.
            Key::Up | Key::Char('k') => self.move_album(-1, AlbumCursorKind::Move),
            Key::Down | Key::Char('j') => self.move_album(1, AlbumCursorKind::Move),
            Key::Home => {
                self.browser.select_first_visible();
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::End => {
                self.browser.select_last_visible();
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::PageUp => self.page_album(-1, AlbumCursorKind::Page),
            Key::PageDown => self.page_album(1, AlbumCursorKind::Page),
            // Left/Right are the tree's parent/child movement (task 2.4):
            // Right expands a collapsed artist root; Left collapses a focused
            // expanded root or returns a leaf to its artist parent. These fire
            // only when the track pane does not hold local focus, and only
            // after the router's fall-through — the panel switch is the Ctrl
            // chord (`panel_left`/`panel_right`), so the bare arrows always
            // reach the tree. Right on an already expanded root (the artist
            // Workspace entry) is task 6.4 and stays unhandled here.
            Key::Left if !self.track_focused => {
                if self.browser.selected_is_artist() {
                    if let Some(root) = self.browser.selected_id() {
                        if self.browser.root_is_expanded(root) {
                            self.browser.collapse_root(root);
                        }
                    }
                    None
                } else if self.browser.move_to_parent() {
                    // The parent is an artist root, so no album selection
                    // crosses: artist focus never overwrites album persistence.
                    self.album_selection_request(AlbumCursorKind::Move)
                } else {
                    None
                }
            }
            // Right first expands cached track children on an album node;
            // the shell already owns any missing album-track fetch and this
            // local operation only projects settled cache data.
            Key::Right if !self.track_focused && !self.browser.selected_is_artist() => {
                if let Some(id) = self.browser.selected_id() {
                    if !self.browser.node_is_expanded(id) {
                        self.browser.expand_node(id);
                    }
                }
                None
            }
            // Right on an artist root (task 2.4/task 6.4): a collapsed root
            // expands first; only a later Right on the already expanded root
            // enters its artist Workspace — Wide takes the inline pane's
            // cursor locally, non-Wide asks the shell to open the Library
            // Hero overlay and focus the same Workspace.
            Key::Right if !self.track_focused && self.browser.selected_is_artist() => {
                let root = self.browser.selected_id()?;
                if !self.browser.root_is_expanded(root) {
                    self.browser.expand_root(root);
                    return None;
                }
                if self.inline_track_focus_enabled {
                    self.enter_artist_workspace_focus();
                    return None;
                }
                self.artist_detail_target()
                    .map(|target| Msg::Shell(ShellRequest::MusicArtistActivate { target }))
            }
            _ => None,
        }
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        let active = self.inline_search.is_active();
        match self.on_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
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
                || (self.track_focused
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
        // Task 6.5 (design D4): the tree owner resolved the neighbour album
        // artwork window from the frame it just painted; the shell applies
        // the existing idle gate and fetches the typed targets. Source
        // pagination for this album level is unconditional and no longer
        // depends on the painted viewport.
        self.browser
            .neighbour_prefetch_targets()
            .map(|targets| Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets }))
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.set_hero_image(state)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
