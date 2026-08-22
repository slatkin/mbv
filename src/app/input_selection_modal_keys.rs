use crate::app::types_selection_modal::SelectionModalSource;
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(super) fn handle_key_selection_modal(&mut self, key: KeyEvent) -> Option<bool> {
        let modal = self.selection_modal.as_ref()?;
        // `[`/`]` filter cycling (the podcast played/unplayed filter, design.md
        // decision 4): only meaningful for a modal that has a filter row, the
        // one bracket convention every other filter/bucket cycle already uses
        // (`cycle_audiobookshelf_filter`, `switch_music_group`).
        if modal.filter.is_some() {
            match key.code {
                KeyCode::Char('[') => {
                    match modal.source {
                        SelectionModalSource::Podcast { .. } => {
                            self.cycle_podcast_selection_modal_filter(-1)
                        }
                        SelectionModalSource::Series { .. } => {
                            self.cycle_series_selection_modal_season(-1)
                        }
                        _ => {}
                    }
                    return Some(false);
                }
                KeyCode::Char(']') => {
                    match modal.source {
                        SelectionModalSource::Podcast { .. } => {
                            self.cycle_podcast_selection_modal_filter(1)
                        }
                        SelectionModalSource::Series { .. } => {
                            self.cycle_series_selection_modal_season(1)
                        }
                        _ => {}
                    }
                    return Some(false);
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Up => self.move_selection_modal_cursor(-1),
            KeyCode::Down => self.move_selection_modal_cursor(1),
            KeyCode::Enter => self.activate_selection_modal_item(),
            KeyCode::Esc | KeyCode::Backspace => self.close_selection_modal(),
            _ => {}
        }
        Some(false)
    }
}
