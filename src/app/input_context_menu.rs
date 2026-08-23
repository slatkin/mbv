use crate::app::{
    App, ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry, PanelFocus, TabSelection,
};

impl App {
    pub(super) fn handle_key_context_menu(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Option<bool> {
        let menu = self.context_menu.as_mut()?;

        match key.code {
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Down => {
                let selectable: Vec<usize> = menu
                    .entries
                    .iter()
                    .enumerate()
                    .filter_map(|(i, entry)| entry.action.is_some().then_some(i))
                    .collect();
                if !selectable.is_empty() {
                    let pos = selectable.iter().position(|&i| i == menu.cursor);
                    let next = match (key.code, pos) {
                        (crossterm::event::KeyCode::Up, Some(pos)) => {
                            pos.checked_sub(1).unwrap_or(selectable.len() - 1)
                        }
                        (crossterm::event::KeyCode::Down, Some(pos)) => {
                            (pos + 1) % selectable.len()
                        }
                        (crossterm::event::KeyCode::Up, None) => selectable.len() - 1,
                        (crossterm::event::KeyCode::Down, None) => 0,
                        _ => unreachable!(),
                    };
                    menu.cursor = selectable[next];
                }
            }
            crossterm::event::KeyCode::Enter => {
                let action = menu
                    .entries
                    .get(menu.cursor)
                    .and_then(|entry| entry.action.clone());
                self.context_menu = None;
                self.layout.context_menu_rect = None;
                self.force_clear = true;
                self.execute_context_action(action);
            }
            crossterm::event::KeyCode::Esc => {
                self.context_menu = None;
                self.layout.context_menu_rect = None;
                self.force_clear = true;
            }
            _ => {}
        }
        Some(false)
    }

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

    fn push_context_separator(entries: &mut Vec<ContextMenuEntry>) {
        entries.push(ContextMenuEntry {
            label: "────────",
            action: None,
        });
    }

    pub(super) fn open_context_menu(&mut self) {
        if self.any_other_modal_open() {
            return;
        }
        let mut entries: Vec<ContextMenuEntry> = vec![];

        let cw_focused =
            matches!(self.effective_panel_focus(), PanelFocus::Library) && self.tab.is_home();
        let lib_idx = self.context_menu_lib_idx();
        let in_podcast =
            lib_idx.is_some_and(|idx| self.is_podcast_library(idx)) || self.is_in_podcast_library();
        let podcast_bulk_ids = lib_idx.and_then(|lib_idx| {
            if in_podcast && self.is_feed_home_video_group_view(lib_idx) {
                Some((
                    self.podcast_mark_all_ids(lib_idx),
                    self.podcast_mark_all_unplayed_ids(lib_idx),
                ))
            } else {
                None
            }
        });
        // Exhaustive dispatch by panel and destination (design §5): a context
        // menu opens only for Home (library focus), an explicitly selected
        // Emby library, or an Emby queue item. Audiobookshelf and Feeds browse
        // rows, non-Emby queue items, and absent or stale targets produce no
        // Emby menu. `cw_focused` / `lib_idx` above drive the Emby-menu content
        // that follows; this match only resolves which target (if any) exists.
        let current_item = match (self.effective_panel_focus(), self.tab) {
            (PanelFocus::Library, TabSelection::Home) => self
                .home
                .continue_items
                .get(self.home.continue_cursor)
                .cloned(),
            (PanelFocus::Library, TabSelection::EmbyLibrary(lib_idx)) => {
                self.current_lib_item(lib_idx)
            }
            (PanelFocus::Library, TabSelection::AudiobookshelfLibrary(_))
            | (PanelFocus::Library, TabSelection::Feeds) => return,
            (PanelFocus::Queue, _) => {
                let queue = self.displayed_queue();
                queue.clone_emby_item_at(queue.queue_cursor)
            }
        };

        if let Some(ref item) = current_item {
            if item.is_folder {
                Self::push_context_action(
                    &mut entries,
                    "Play All",
                    ContextAction::PlayFolder(item.id.clone()),
                );
                Self::push_context_action(
                    &mut entries,
                    "Shuffle",
                    ContextAction::ShuffleFolder(item.id.clone()),
                );
                Self::push_context_action(
                    &mut entries,
                    "Add to Queue",
                    ContextAction::EnqueueFolder(Box::new(item.clone())),
                );
                let (played_label, unplayed_label) = if in_podcast {
                    ("Mark Played", "Mark Unplayed")
                } else {
                    ("Mark Watched", "Mark Unwatched")
                };
                if self.context_menu_play_state(item) {
                    Self::push_context_action(
                        &mut entries,
                        unplayed_label,
                        ContextAction::MarkUnplayed(item.id.clone()),
                    );
                } else {
                    Self::push_context_action(
                        &mut entries,
                        played_label,
                        ContextAction::MarkPlayed(item.id.clone()),
                    );
                }
            } else {
                Self::push_context_action(&mut entries, "Play", ContextAction::Play);
                if cw_focused
                    || lib_idx.is_some()
                    || !matches!(self.effective_panel_focus(), PanelFocus::Queue)
                {
                    Self::push_context_action(&mut entries, "Add to Queue", ContextAction::Enqueue);
                }
                // Audio items (music tracks) don't get mark-played, but podcast
                // episodes (Audio inside a Channel library) do.
                let is_music_audio =
                    (item.media_type == "Audio" || item.item_type == "Audio") && !in_podcast;
                if !is_music_audio {
                    let (played_label, unplayed_label) = if in_podcast {
                        ("Mark Played", "Mark Unplayed")
                    } else {
                        ("Mark Watched", "Mark Unwatched")
                    };
                    if self.context_menu_play_state(item) {
                        Self::push_context_action(
                            &mut entries,
                            unplayed_label,
                            ContextAction::MarkUnplayed(item.id.clone()),
                        );
                    } else {
                        Self::push_context_action(
                            &mut entries,
                            played_label,
                            ContextAction::MarkPlayed(item.id.clone()),
                        );
                    }
                }
                if cw_focused || (self.tab.is_home() && self.home.section == 0) {
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Continue Watching",
                        ContextAction::RemoveFromContinueWatching,
                    );
                }
                if !cw_focused && matches!(self.effective_panel_focus(), PanelFocus::Queue) {
                    let pos = self.displayed_queue().queue_cursor;
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Queue",
                        ContextAction::RemoveFromQueue(pos),
                    );
                }
                if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
                    Self::push_context_action(
                        &mut entries,
                        "Go to Library",
                        ContextAction::GoToLibrary(item.id.clone(), item.item_type.clone()),
                    );
                }
            }
        }

        if let Some((played_ids, unplayed_ids)) = podcast_bulk_ids {
            if !played_ids.is_empty() || !unplayed_ids.is_empty() {
                Self::push_context_separator(&mut entries);
                Self::push_context_action(
                    &mut entries,
                    "Mark All Played",
                    ContextAction::MarkItemsPlayed(played_ids),
                );
                Self::push_context_action(
                    &mut entries,
                    "Mark All Unplayed",
                    ContextAction::MarkItemsUnplayed(unplayed_ids),
                );
            }
        }

        if entries.iter().all(|entry| entry.action.is_none()) {
            return;
        }

        let anchor = match self.effective_panel_focus() {
            PanelFocus::Library => ContextMenuAnchor::SelectedItem(PanelFocus::Library),
            PanelFocus::Queue => ContextMenuAnchor::SelectedItem(PanelFocus::Queue),
        };
        self.context_menu = Some(ContextMenu {
            anchor,
            cursor: ContextMenu::first_selectable(&entries),
            entries,
        });
    }

    pub(super) fn open_context_menu_at(&mut self, x: u16, y: u16) {
        self.open_context_menu();
        if let Some(ref mut menu) = self.context_menu {
            menu.anchor = ContextMenuAnchor::Pointer { x, y };
        }
    }
}
