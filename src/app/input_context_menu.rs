use crate::app::{App, ContextAction, ContextMenu, ContextMenuEntry, PanelFocus};

impl App {
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
        let mut entries: Vec<ContextMenuEntry> = vec![];

        let cw_focused = matches!(self.panel_focus, PanelFocus::Library) && self.tab.is_home();
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
        let artist_header_context = lib_idx
            .and_then(|lib_idx| self.selected_artist_header_album_items(lib_idx))
            .map(|(selection, _)| selection);

        let current_item = if artist_header_context.is_some() {
            None
        } else if cw_focused {
            self.home
                .continue_items
                .get(self.home.continue_cursor)
                .cloned()
        } else if lib_idx.is_some() {
            self.current_lib_item()
        } else if matches!(self.panel_focus, PanelFocus::Queue) {
            let queue = self.displayed_queue();
            queue.clone_emby_item_at(queue.queue_cursor)
        } else {
            None
        };

        if let Some(selection) = artist_header_context {
            Self::push_context_action(
                &mut entries,
                "Play All",
                ContextAction::PlayArtistHeader(selection.clone()),
            );
            Self::push_context_action(
                &mut entries,
                "Shuffle",
                ContextAction::ShuffleArtistHeader(selection.clone()),
            );
            Self::push_context_action(
                &mut entries,
                "Add to Queue",
                ContextAction::EnqueueArtistHeader(selection),
            );
        } else if let Some(ref item) = current_item {
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
                if cw_focused || lib_idx.is_some() || !matches!(self.panel_focus, PanelFocus::Queue)
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
                if !cw_focused && matches!(self.panel_focus, PanelFocus::Queue) {
                    let pos = self.displayed_queue().queue_cursor;
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Queue",
                        ContextAction::RemoveFromQueue(pos),
                    );
                }
                if matches!(self.panel_focus, PanelFocus::Queue) {
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

        let (x, y) = self.context_menu_spawn_point();
        self.context_menu = Some(ContextMenu {
            x,
            y,
            cursor: ContextMenu::first_selectable(&entries),
            entries,
        });
    }

    pub(super) fn open_context_menu_at(&mut self, x: u16, y: u16) {
        self.open_context_menu();
        if let Some(ref mut menu) = self.context_menu {
            menu.x = x;
            menu.y = y;
        }
    }

    fn context_menu_spawn_point(&self) -> (u16, u16) {
        match self.panel_focus {
            PanelFocus::Library => {
                let area = self.layout.main.left_area;
                if area.width > 0 {
                    let y = self.layout.main.cursor_screen_y.unwrap_or(area.y);
                    let x = area.x + 2;
                    // Avoid inline image overlap (detail/episode poster).
                    if let Some(img) = self.layout.main.inline_image_rect {
                        if y >= img.y && y < img.y + img.height {
                            let below = img.y + img.height;
                            if below < area.y + area.height {
                                return (x, below);
                            }
                        }
                    }
                    return (x, y);
                }
            }
            PanelFocus::Queue => {
                let area = self.layout.main.queue_area;
                if area.width > 0 {
                    let y = self.layout.main.queue_cursor_screen_y.unwrap_or(area.y);
                    return (area.x + 2, y);
                }
            }
        }
        (4, 4)
    }
}
