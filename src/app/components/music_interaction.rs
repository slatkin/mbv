impl MusicContent {
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                let delta = index as i64 - self.context.group_cursor as i64;
                (delta != 0).then_some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta }))
            }
            LibrarySlotEvent::List(input) => {
                if self.inline_search.is_active() {
                    match input {
                        MediaListSurfaceInput::Wheel { delta, .. } => {
                            self.inline_search.move_cursor_by(delta);
                            Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                        }
                        MediaListSurfaceInput::DoubleClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            self.inline_search.selected_item().map(|item| {
                                Msg::Shell(ShellRequest::InlineSearchActivate {
                                    id: item.id,
                                    item_type: item.item_type,
                                })
                            })
                        }
                        MediaListSurfaceInput::ContextClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            self.inline_search.selected_item().map(|item| {
                                Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]), None))
                            })
                        }
                        MediaListSurfaceInput::Click(at) | MediaListSurfaceInput::ToggleClick(at) | MediaListSurfaceInput::RangeClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            None
                        }
                        _ => None,
                    }
                } else {
                    match input {
                        MediaListSurfaceInput::Wheel { at, delta } => {
                            if self.carrier.claims_current_point(at) {
                                self.carrier.delegate_operation(MediaListSurfaceInput::Wheel { at, delta }.into_operation(None).expect("resolved media-list pointer target"));
                                let target = self.carrier.selected_target()?;
                                let index = self
                                    .context
                                    .album_targets
                                    .iter()
                                    .position(|candidate| candidate == target)?;
                                Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                                    target: index,
                                    kind: AlbumCursorKind::Move,
                                }))
                            } else {
                                None
                            }
                        }
                        MediaListSurfaceInput::Click(at) | MediaListSurfaceInput::ToggleClick(at) | MediaListSurfaceInput::RangeClick(at) => {
                            let target = self.carrier.resolve_current_point(at)?.clone();
                            self.carrier.delegate_operation(input.into_operation(Some(target)).expect("resolved media-list pointer target"));
                            let _ = ();
                            Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                                target: self.selected_album_index(),
                                kind: AlbumCursorKind::Move,
                            }))
                        }
                        MediaListSurfaceInput::DoubleClick(_) => self
                            .selected_item()
                            .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item })),
                        MediaListSurfaceInput::ContextClick(at) => {
                            let target = self.carrier.resolve_current_point(at)?.clone();
                            let outcome = self.carrier.delegate_operation(input.into_operation(Some(target.clone())).expect("resolved media-list pointer target"));
                            let _ = ();
                            let items = match outcome.external_intent {
                                Some(RowIntent::ContextSelection(targets)) => targets
                                    .into_iter()
                                    .filter_map(|target| self.context.list.items.iter().find(|item| item.id == target).cloned())
                                    .collect(),
                                _ => vec![self.selected_item()?],
                            };
                            Some(Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(items), Some((at.x, at.y)))))
                        },
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
                        let album_target = self.carrier.selected_target()?;
                        let album_index = self
                            .context
                            .album_targets
                            .iter()
                            .position(|candidate| candidate == album_target)?;
                        let album = self.context.list.items.get(album_index)?;
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

