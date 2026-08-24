use super::types_selection_modal::SelectionModalItem;
use super::{
    App, SelectionModal, SelectionModalFilter, SelectionModalListState, SelectionModalRow,
    SelectionModalSource,
};
use crate::app::ui_util::fmt_duration_mmss;
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};

pub(crate) fn album_modal_state(tracks: &[EmbyItem]) -> SelectionModalListState {
    let rows = tracks
        .iter()
        .map(|track| {
            let title = album_track_title(track);
            let name = if track.index_number > 0 {
                format!("{:>2}. {title}", track.index_number)
            } else {
                title
            };
            SelectionModalRow::Item(SelectionModalItem {
                name,
                meta: if track.runtime_ticks > 0 {
                    fmt_duration_mmss(track.runtime_ticks / TICKS_PER_SECOND)
                } else {
                    String::new()
                },
                id: track.id.clone(),
            })
        })
        .collect();
    SelectionModalListState::ready(rows)
}

pub(crate) fn album_track_title(item: &EmbyItem) -> String {
    let raw_name = if item.name.trim().is_empty() {
        item.file_name()
    } else {
        item.name.trim()
    };
    let file_name = raw_name.rsplit(['/', '\\']).next().unwrap_or(raw_name);
    let (stem, from_filename) = file_name
        .rsplit_once('.')
        .filter(|(_, extension)| {
            [
                "aac", "aif", "aiff", "alac", "ape", "flac", "m4a", "mka", "mp3", "oga", "ogg",
                "opus", "wav", "wma", "wv",
            ]
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
        })
        .map(|(stem, _)| (stem, true))
        .unwrap_or((file_name, false));
    let digits_end = stem
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .last()
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or(0);
    if digits_end == 0 {
        return stem.to_string();
    }
    if !from_filename
        && item.index_number > 0
        && stem[..digits_end].parse::<i64>().ok() != Some(item.index_number)
    {
        return stem.to_string();
    }
    let after_number = &stem[digits_end..];
    let trimmed = after_number.trim_start();
    let has_separator = trimmed.len() != after_number.len()
        || matches!(trimmed.chars().next(), Some('-' | '.' | '_'));
    if !has_separator {
        return stem.to_string();
    }
    let title = trimmed.trim_start_matches(['-', '.', '_']).trim();
    if title.is_empty() {
        stem.to_string()
    } else {
        title.to_string()
    }
}

impl App {
    pub(crate) fn open_selection_modal(
        &mut self,
        source: SelectionModalSource,
        title: String,
        state: SelectionModalListState,
        filter: Option<SelectionModalFilter>,
    ) {
        let state = state.normalize();
        let cursor = state
            .rows()
            .iter()
            .position(|row| matches!(row, SelectionModalRow::Item(_)))
            .unwrap_or(0);
        self.pending_overlay = Some(super::types_overlay::OverlayRequest::SelectionModal(
            SelectionModal {
                source,
                title,
                state,
                cursor,
                filter,
            },
        ));
    }

    pub(crate) fn close_selection_modal(&mut self) {
        self.pending_overlay = Some(super::types_overlay::OverlayRequest::DismissSelectionModal);
    }

    /// Opens the Album constituent-list modal (design.md decision 3/task
    /// 3.3): a flat scrollable list of track `Item` rows, no headers (unlike
    /// Series' season-grouped modal -- tracks aren't hierarchical). Ensures
    /// the track list is fetched, mirroring `open_series_selection_modal`;
    /// if it hasn't landed in `album_tracks_cache` yet, opens with a loading
    /// placeholder instead of track rows.
    pub(crate) fn open_album_selection_modal(&mut self, album: &EmbyItem) {
        self.fetch_album_tracks(album.id.clone());
        let state = match self.album_tracks_cache.get(&album.id) {
            Some(tracks) => album_modal_state(tracks),
            None => SelectionModalListState::Loading,
        };
        self.open_selection_modal(
            SelectionModalSource::Album {
                album_id: album.id.clone(),
            },
            album.display_name(),
            state,
            None,
        );
    }

    /// Replaces the live list only when the provider completion belongs to the
    /// open modal. The selected item's provider ID, rather than its row
    /// position, is the cursor anchor across reordered refreshes.
    pub(crate) fn refresh_selection_modal(
        &mut self,
        source: SelectionModalSource,
        state: SelectionModalListState,
        filter: Option<SelectionModalFilter>,
    ) {
        self.pending_overlay = Some(
            super::types_overlay::OverlayRequest::RefreshSelectionModal {
                source,
                state,
                filter,
            },
        );
    }

    pub(crate) fn activate_selection_modal_item(
        &mut self,
        source: SelectionModalSource,
        item_id: Option<String>,
    ) {
        if let SelectionModalSource::Series { series_id } = source {
            let episode_id = item_id;
            let episode = episode_id.and_then(|id| {
                self.series_detail_cache.get(&series_id).and_then(|detail| {
                    detail
                        .episodes
                        .values()
                        .flatten()
                        .find(|ep| ep.id == id)
                        .cloned()
                })
            });
            self.close_selection_modal();
            if let Some(episode) = episode {
                self.play_item(episode);
            }
            return;
        }
        if matches!(source, SelectionModalSource::Podcast { .. }) {
            // Podcast activation is intentionally read-only: the selection
            // modal is for browsing downloaded episodes, not for starting
            // playback or changing the queue (podcast-library spec).
            self.close_selection_modal();
            return;
        }
        if let SelectionModalSource::Album { album_id } = source {
            let track_id = item_id;
            let track = track_id.and_then(|id| {
                self.album_tracks_cache
                    .get(&album_id)
                    .and_then(|tracks| tracks.iter().find(|track| track.id == id).cloned())
            });
            self.close_selection_modal();
            if let Some(track) = track {
                self.play_album_track(&album_id, &track);
            }
            return;
        }
        if let SelectionModalSource::Book { book_id } = source {
            let chapter_id = item_id
                .as_deref()
                .and_then(|id| id.strip_prefix("chapter:"))
                .and_then(|id| id.parse::<usize>().ok());
            let audio_file_id = item_id
                .as_deref()
                .and_then(|id| id.strip_prefix("audio-file:"));
            let chapter_index = self.audiobookshelf_book_browse.iter().find_map(|state| {
                let (chapters, files) = state.detail_cache.get(&book_id)?;
                chapter_id
                    .and_then(|id| chapters.iter().position(|chapter| chapter.id == id))
                    .or_else(|| {
                        audio_file_id.and_then(|id| files.iter().position(|file| file.ino == id))
                    })
            });
            self.close_selection_modal();
            if let Some(chapter_index) = chapter_index {
                if let Some(index) = self.tab.audiobookshelf_index() {
                    if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
                        state.chapter_selection = Some(chapter_index);
                    }
                }
                self.activate_audiobookshelf_book_row();
            }
            return;
        }
        self.close_selection_modal();
    }
}
