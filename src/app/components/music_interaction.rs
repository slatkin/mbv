impl MusicContent {
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                let delta = index as i64 - self.context.group_cursor as i64;
                (delta != 0).then_some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta }))
            }
            LibrarySlotEvent::List(input) => {
                if self.inline_search.is_active() {
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
                        MediaListSurfaceInput::Click(at)
                        | MediaListSurfaceInput::ToggleClick(at)
                        | MediaListSurfaceInput::RangeClick(at) => {
                            let target = search.results_mut().resolve_current_point(at)?.clone();
                            // Design D4 non-goal: a search session has no
                            // multi-selection UI, so a modifier click selects
                            // exactly the clicked row and never toggles or
                            // extends a range into `multi_selection`.
                            search
                                .results_mut()
                                .delegate_operation(MediaListOperation::Select(target));
                            None
                        }
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
                                        Msg::Shell(ShellRequest::RowContextMenu(
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
                            self.album_selection_request(AlbumCursorKind::Move)
                        }
                        MediaListSurfaceInput::Click(at)
                        | MediaListSurfaceInput::ToggleClick(at)
                        | MediaListSurfaceInput::RangeClick(at) => {
                            if !self.browser.claims_point(at) {
                                return None;
                            }
                            let (_id, index) = self.browser.hit_node(at)?;
                            self.browser.select_index(index);
                            self.album_selection_request(AlbumCursorKind::Move)
                        }
                        MediaListSurfaceInput::DoubleClick(at) => {
                            if !self.browser.claims_point(at) {
                                return None;
                            }
                            let (_id, index) = self.browser.hit_node(at)?;
                            self.browser.select_index(index);
                            self.selected_item()
                                .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item }))
                        }
                        MediaListSurfaceInput::ContextClick(at) => {
                            if !self.browser.claims_point(at) {
                                return None;
                            }
                            let (_id, index) = self.browser.hit_node(at)?;
                            self.browser.select_index(index);
                            if self.browser.selected_is_artist() {
                                let items = self.selected_artist_items()?;
                                if items.is_empty() {
                                    return None;
                                }
                                Some(Msg::Shell(ShellRequest::RowContextMenu(
                                    crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                                    Some((at.x, at.y)),
                                )))
                            } else {
                                let item = self.selected_item()?;
                                Some(Msg::Shell(ShellRequest::RowContextMenu(
                                    crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]),
                                    Some((at.x, at.y)),
                                )))
                            }
                        }
                        _ => None,
                    }
                }
            }
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::ControlPicked(_) => {
                None
            }
            LibrarySlotEvent::HeroActivate => {
                if self.track_focused {
                    let track = self.selected_track_item()?;
                    let album = self.selected_item()?;
                    Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                        album_id: album.id,
                        track,
                    }))
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
                        let track_target = self.track_list.selected_target()?;
                        let track = self
                            .context
                            .album_tracks
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .find(|track| &track.id == track_target)
                            .cloned()?;
                        let album = self.selected_item()?;
                        Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                            album_id: album.id.clone(),
                            track,
                        }))
                    })?
                }
                MediaListSurfaceInput::ContextClick(at) => {
                    let target = self.track_list.resolve_current_point(at)?.clone();
                    let item = self.context.album_tracks.as_deref().unwrap_or_default().iter().find(|track| track.id == target)?.clone();
                    let outcome = self.track_list.delegate_operation(input.into_operation(Some(target)).expect("resolved media-list pointer target"));
                    let _ = ();
                    let items = match outcome.external_intent {
                        Some(RowIntent::ContextSelection(targets)) => targets
                            .into_iter()
                            .filter_map(|target| self.context.album_tracks.as_deref().unwrap_or_default().iter().find(|track| track.id == target).cloned())
                            .collect(),
                        _ => vec![item],
                    };
                    Some(Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(items), Some((at.x, at.y)))))
                },
                _ => None,
            },
        }
    }
}

