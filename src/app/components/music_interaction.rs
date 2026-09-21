impl MusicContent {
    /// Whether the destination's own tree filter owns pointer/keyboard input.
    /// Production Grouped Music filtering paints the tree and leaves the flat
    /// Inline Search carrier empty, so input must resolve through the
    /// current-frame tree geometry rather than the compatibility carrier path
    /// (design D5).
    fn local_filter_owns_input(&self) -> bool {
        self.browser.filter_active()
            && !self.inline_search.has_pool_entries()
            && self.inline_search.results_len() == 0
    }

    fn pointer_album_selection_request(&mut self, kind: AlbumCursorKind) -> Option<Msg> {
        self.album_selection_request(kind)
            .or(Some(Msg::Shell(ShellRequest::LibraryPanelFocus)))
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                let delta = index as i64 - self.context.group_cursor as i64;
                (delta != 0).then_some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta }))
            }
            LibrarySlotEvent::List(input) => {
                let filtered_tree = self.local_filter_owns_input();
                if self.inline_search.is_active() && !filtered_tree {
                    // Inline Search pointer handling (design.md D4): the
                    // panel-normalized input delegates to the session's
                    // embedded carrier like every other list — a click
                    // selects, a double-click activates, a right-click
                    // resolves the row's ordinary item-based context-menu
                    // intent, and a wheel over the painted rows is claimed.
                    let search = &mut self.inline_search;
                    match input {
                        MediaListSurfaceInput::Wheel { at, delta } => {
                            if !search.results_mut().claims_current_point(at) {
                                return None;
                            }
                            search.results_mut().delegate_operation(
                                MediaListSurfaceInput::Wheel { at, delta }
                                    .into_operation(None)
                                    .expect("wheel converts without a target"),
                            );
                            Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                        }
                        // When the destination's own filter owns pointer
                        // input, the branch below resolves the current-frame
                        // filtered tree geometry and focuses Library through
                        // `pointer_album_selection_request`. This flat Inline
                        // Search carrier path resolves only wheel,
                        // double-click and context-click, so a plain or
                        // modified click must not mutate it: the framework
                        // delivers after mutation, and discarding the message
                        // would leave a losing selection change behind.
                        MediaListSurfaceInput::Click(_)
                        | MediaListSurfaceInput::ToggleClick(_)
                        | MediaListSurfaceInput::RangeClick(_) => None,
                        MediaListSurfaceInput::DoubleClick(at) => {
                            let target = search.results_mut().resolve_current_point(at)?.clone();
                            let outcome = search.results_mut().delegate_operation(
                                MediaListSurfaceInput::DoubleClick(at)
                                    .into_operation(Some(target))
                                    .expect("resolved media-list pointer target"),
                            );
                            // The delegated transition's resolved intent is
                            // the authority for which row the gesture
                            // activated.
                            match outcome.external_intent {
                                Some(RowIntent::Activate(target)) => {
                                    search.item_for_target(&target).map(|item| {
                                        Msg::Shell(ShellRequest::InlineSearchActivate {
                                            id: item.id,
                                            item_type: item.item_type,
                                        })
                                    })
                                }
                                // A double-click never resolves a context
                                // intent, and no row resolved when the intent
                                // is `None`.
                                Some(RowIntent::Context(_))
                                | Some(RowIntent::ContextSelection(_))
                                | None => None,
                            }
                        }
                        MediaListSurfaceInput::ContextClick(at) => {
                            let target = search.results_mut().resolve_current_point(at)?.clone();
                            let outcome = search.results_mut().delegate_operation(
                                MediaListSurfaceInput::ContextClick(at)
                                    .into_operation(Some(target))
                                    .expect("resolved media-list pointer target"),
                            );
                            // The delegated transition's resolved intent is
                            // the authority for which row the gesture
                            // contextualized.
                            match outcome.external_intent {
                                Some(RowIntent::Context(target)) => {
                                    search.item_for_target(&target).map(|item| {
                                        Msg::Shell(ShellRequest::MusicRowContextMenu(
                                            crate::app::types_context_menu::ContextMenuTargets::Emby(
                                                vec![item],
                                            ),
                                            None,
                                        ))
                                    })
                                }
                                // A context click never resolves an activate
                                // intent, a search session has no Visual-mode
                                // multi-selection so a `ContextSelection`
                                // cannot arise (D4 non-goal), and no row
                                // resolved when the intent is `None`.
                                Some(RowIntent::Activate(_))
                                | Some(RowIntent::ContextSelection(_))
                                | None => None,
                            }
                        }
                        _ => None,
                    }
                } else {
                    match input {
                        MediaListSurfaceInput::Wheel { at, delta } => {
                            if !self.browser.claims_point(at) {
                                return None;
                            }
                            self.browser.move_selection(delta);
                            self.pointer_album_selection_request(AlbumCursorKind::Move)
                        }
                        MediaListSurfaceInput::Click(at)
                        | MediaListSurfaceInput::RangeClick(at) => {
                            if !self.browser.claims_point(at) {
                                return None;
                            }
                            let (_id, index) = self.browser.hit_node(at)?;
                            self.browser.clear_marks();
                            self.browser.select_index(index);
                            self.pointer_album_selection_request(AlbumCursorKind::Move)
                        }
                        MediaListSurfaceInput::ToggleClick(at) => {
                            // Resolve the latest painted row before changing
                            // either focus or membership. Artist roots toggle
                            // their visible album descendants; they never
                            // become effect or Queue targets.
                            self.browser.toggle_mark_at(at)?;
                            self.pointer_album_selection_request(AlbumCursorKind::Move)
                        }
                        MediaListSurfaceInput::DoubleClick(at) => {
                            if !self.browser.claims_point(at) {
                                return None;
                            }
                            let (id, index) = self.browser.hit_node(at)?;
                            if self.browser.model_is_track(id) {
                                // Resolve the stable identity from the resolved
                                // node before any local mutation, so a track
                                // gesture never changes the selection and then
                                // returns no message.
                                let (album_target, track_id) =
                                    self.browser.track_identity_of(id)?;
                                let album_target = album_target.to_string();
                                let track_id = track_id.to_string();
                                self.browser.select_index(index);
                                return Some(Msg::Shell(
                                    ShellRequest::MusicTreeTrackActivate {
                                        album_target,
                                        track_id,
                                    },
                                ));
                            }
                            self.browser.select_index(index);
                            if self.browser.model_is_artist(id) {
                                self.browser.toggle_root(id);
                                return Some(Msg::Shell(ShellRequest::LibraryPanelFocus));
                            }
                            if self.browser.node_has_children(id) {
                                self.browser.toggle_node(id);
                            }
                            Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
                        }
                        MediaListSurfaceInput::ContextClick(at) => {
                            if !self.browser.claims_point(at) {
                                return None;
                            }
                            let (id, index) = self.browser.hit_node(at)?;
                            let marked = self.browser.selected_album_targets_in_display_order();
                            let clicked_marked = if let Some(target) = self.browser.target_of(id) {
                                marked.iter().any(|selected| selected == target)
                            } else if self.browser.model_is_artist(id) {
                                self.browser
                                    .artist_album_targets(id)
                                    .into_iter()
                                    .any(|target| marked.iter().any(|selected| selected == &target))
                            } else {
                                false
                            };
                            self.browser.select_index(index);
                            if !marked.is_empty() && clicked_marked {
                                let (items, unresolved_targets) = self.selected_tree_items()?;
                                if items.is_empty() && unresolved_targets.is_empty() {
                                    return None;
                                }
                                return Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
                                    crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                                    Some((at.x, at.y)),
                                )));
                            }

                            // A context click outside the marked set has the
                            // canonical list semantics: clear only this tree
                            // selection and open the ordinary single/root menu.
                            self.browser.clear_marks();
                            if self.browser.selected_is_artist() {
                                let (items, _unresolved_targets) = self.selected_artist_items()?;
                                if items.is_empty() {
                                    return None;
                                }
                                Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
                                    crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                                    Some((at.x, at.y)),
                                )))
                            } else {
                                let item = self.selected_item()?;
                                Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
                                    crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]),
                                    Some((at.x, at.y)),
                                )))
                            }
                        }
                        _ => None,
                    }
                }
            }
            LibrarySlotEvent::WorkspaceSelectorPicked(_) => None,
            LibrarySlotEvent::HeroActivate => {
                if self.track_focused {
                    if let Some(message) = self.artist_track_activation() {
                        Some(message)
                    } else {
                        let track = self.selected_track_item()?;
                        let album_id = self.focused_track_album_id()?;
                        Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                            album_id,
                            track,
                        }))
                    }
                } else {
                    self.selected_item()
                        .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item }))
                }
            }
            LibrarySlotEvent::HeroPane(input) => match input {
                // The track owner is local to this workspace; the shell
                // never recomputes a wheel step. Track-pane focus is not a
                // selection and does not move here (mirrors the retired
                // `MusicWorkspaceComponent`'s wheel handling).
                MediaListSurfaceInput::Wheel { at, delta } => {
                    if self.track_list.claims_current_point(at) {
                        self.track_list.delegate_operation(MediaListSurfaceInput::Wheel { at, delta }.into_operation(None).expect("resolved media-list pointer target"));
                    }
                    None
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at)
                | MediaListSurfaceInput::DoubleClick(at) => {
                    let target = self.track_list.resolve_current_point(at)?.clone();
                    self.track_focused = true;
                    self.track_list.delegate_operation(input.into_operation(Some(target)).expect("resolved media-list pointer target"));
                    (matches!(input, MediaListSurfaceInput::DoubleClick(_))).then(|| {
                        if let Some(message) = self.artist_track_activation() {
                            Some(message)
                        } else {
                            let track_target = self.track_list.selected_target()?;
                            let track = self.workspace_track_item(track_target)?;
                            let album_id = self.focused_track_album_id()?;
                            Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                                album_id,
                                track,
                            }))
                        }
                    })?
                }
                MediaListSurfaceInput::ContextClick(at) => {
                    let target = self.track_list.resolve_current_point(at)?.clone();
                    let item = self.workspace_track_item(&target)?;
                    let outcome = self.track_list.delegate_operation(input.into_operation(Some(target)).expect("resolved media-list pointer target"));
                    let _ = ();
                    let items = match outcome.external_intent {
                        Some(RowIntent::ContextSelection(targets)) => targets
                            .into_iter()
                            .filter_map(|target| self.workspace_track_item(&target))
                            .collect(),
                        _ => vec![item],
                    };
                    Some(Msg::Shell(ShellRequest::MusicRowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(items), Some((at.x, at.y)))))
                },
                _ => None,
            },
        }
    }
}

