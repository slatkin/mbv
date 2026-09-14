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
                        RowLocalInput::Wheel { delta, .. } => {
                            self.inline_search.move_cursor_by(delta);
                            Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                        }
                        RowLocalInput::DoubleClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            self.inline_search.selected_item().map(|item| {
                                Msg::Shell(ShellRequest::InlineSearchActivate {
                                    id: item.id,
                                    item_type: item.item_type,
                                })
                            })
                        }
                        RowLocalInput::ContextClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            self.inline_search.selected_item().map(|item| {
                                Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]), None))
                            })
                        }
                        RowLocalInput::Click(at) | RowLocalInput::ToggleClick(at) | RowLocalInput::RangeClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            None
                        }
                        _ => None,
                    }
                } else {
                    match input {
                        RowLocalInput::Wheel { at, delta } => {
                            if self.carrier.claims_current_point(at) {
                                self.carrier
                                    .delegate(RowLocalInput::Wheel { at, delta }, None);
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
                        RowLocalInput::Click(at) | RowLocalInput::ToggleClick(at) | RowLocalInput::RangeClick(at) => {
                            let target = self.carrier.resolve_current_point(at)?.clone();
                            self.carrier.delegate(input, Some(target));
                            if let Some(count) = self.carrier.selection_changed_msg() {
                                return Some(Msg::Shell(ShellRequest::SelectionChanged(count)));
                            }
                            Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                                target: self.selected_album_index(),
                                kind: AlbumCursorKind::Move,
                            }))
                        }
                        RowLocalInput::DoubleClick(_) => self
                            .selected_item()
                            .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item })),
                        RowLocalInput::ContextClick(at) => self.selected_item().map(|item| {
                            Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]), Some((at.x, at.y))))
                        }),
                        _ => None,
                    }
                }
            }
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::ControlPicked(_) => {
                None
            }
            LibrarySlotEvent::HeroPane(input) => match input {
                // The track owner is local to this workspace; the shell
                // never recomputes a wheel step. Track-pane focus is not a
                // selection and does not move here (mirrors the retired
                // `MusicWorkspaceComponent`'s wheel handling).
                RowLocalInput::Wheel { at, delta } => {
                    if self.track_list.claims_current_point(at) {
                        self.track_list
                            .delegate(RowLocalInput::Wheel { at, delta }, None);
                    }
                    None
                }
                RowLocalInput::Click(at)
                | RowLocalInput::ToggleClick(at)
                | RowLocalInput::RangeClick(at)
                | RowLocalInput::DoubleClick(at) => {
                    let target = self.track_list.resolve_current_point(at)?.clone();
                    self.track_focused = true;
                    self.track_list.delegate(input, Some(target));
                    (matches!(input, RowLocalInput::DoubleClick(_))).then(|| {
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
                RowLocalInput::ContextClick(at) => self
                    .track_list
                    .resolve_current_point(at)
                    .and_then(|target| {
                        self.context
                            .album_tracks
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .find(|track| track.id == *target)
                            .cloned()
                            .map(|track| {
                                Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(vec![track]), Some((at.x, at.y))))
                            })
                    }),
                _ => None,
            },
        }
    }
}

