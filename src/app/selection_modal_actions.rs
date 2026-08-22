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
        self.selection_modal = Some(SelectionModal {
            source,
            title,
            state,
            cursor,
            filter,
        });
    }

    pub(crate) fn close_selection_modal(&mut self) {
        self.selection_modal = None;
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
    ) {
        let Some(modal) = self.selection_modal.as_mut() else {
            return;
        };
        if modal.source != source {
            return;
        }
        let selected_id = modal
            .state
            .rows()
            .get(modal.cursor)
            .and_then(SelectionModalRow::item_id)
            .map(str::to_owned);
        let state = state.normalize();
        let cursor = selected_id
            .as_deref()
            .and_then(|id| {
                state
                    .rows()
                    .iter()
                    .position(|row| row.item_id() == Some(id))
            })
            .or_else(|| state.rows().iter().position(|row| row.item_id().is_some()))
            .unwrap_or(0);
        modal.state = state;
        modal.cursor = cursor;
    }

    pub(crate) fn move_selection_modal_cursor(&mut self, delta: i64) {
        let Some(modal) = self.selection_modal.as_mut() else {
            return;
        };
        let item_positions: Vec<usize> = modal
            .state
            .rows()
            .iter()
            .enumerate()
            .filter_map(|(i, row)| matches!(row, SelectionModalRow::Item(_)).then_some(i))
            .collect();
        let Some(pos) = item_positions.iter().position(|&i| i == modal.cursor) else {
            return;
        };
        let next = (pos as i64 + delta).clamp(0, item_positions.len() as i64 - 1) as usize;
        modal.cursor = item_positions[next];
    }

    pub(crate) fn activate_selection_modal_item(&mut self) {
        let Some(modal) = self.selection_modal.as_ref() else {
            return;
        };
        let source = modal.source.clone();
        let item_id = match modal.state.rows().get(modal.cursor) {
            Some(SelectionModalRow::Item(item)) => Some(item.id.clone()),
            _ => None,
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types_selection_modal::SelectionModalItem;

    fn item(id: &str) -> SelectionModalRow {
        SelectionModalRow::Item(SelectionModalItem {
            name: id.into(),
            meta: String::new(),
            id: id.into(),
        })
    }

    fn source() -> SelectionModalSource {
        SelectionModalSource::Album {
            album_id: "album-1".into(),
        }
    }

    #[test]
    fn matching_refresh_preserves_cursor_by_stable_item_id() {
        let mut app = crate::app::tests::make_app_stub();
        app.open_selection_modal(
            source(),
            "Tracks".into(),
            SelectionModalListState::ready(vec![item("a"), item("b")]),
            None,
        );
        app.move_selection_modal_cursor(1);

        app.refresh_selection_modal(
            source(),
            SelectionModalListState::ready(vec![item("x"), item("b"), item("c")]),
        );

        let modal = app.selection_modal.as_ref().unwrap();
        assert_eq!(modal.cursor, 1);
        assert_eq!(modal.state.rows()[modal.cursor].item_id(), Some("b"));
    }

    #[test]
    fn nonmatching_refresh_does_not_mutate_open_modal() {
        let mut app = crate::app::tests::make_app_stub();
        app.open_selection_modal(
            source(),
            "Tracks".into(),
            SelectionModalListState::ready(vec![item("a")]),
            None,
        );

        app.refresh_selection_modal(
            SelectionModalSource::Series {
                series_id: "series-1".into(),
            },
            SelectionModalListState::Empty,
        );

        let modal = app.selection_modal.as_ref().unwrap();
        assert_eq!(modal.cursor, 0);
        assert_eq!(modal.state.rows()[0].item_id(), Some("a"));
    }

    #[test]
    fn matching_refresh_replaces_loading_ready_and_empty_in_place() {
        let mut app = crate::app::tests::make_app_stub();
        app.open_selection_modal(
            source(),
            "Tracks".into(),
            SelectionModalListState::Loading,
            None,
        );

        for state in [
            SelectionModalListState::ready(vec![item("a")]),
            SelectionModalListState::Empty,
        ] {
            app.refresh_selection_modal(source(), state);
            assert!(app.selection_modal.is_some());
        }

        assert!(matches!(
            app.selection_modal.as_ref().unwrap().state,
            SelectionModalListState::Empty
        ));
    }

    #[test]
    fn album_modal_rows_include_track_number_in_name() {
        let mut track = crate::app::tests::make_item("Track Name", "Audio");
        track.index_number = 1;

        let state = album_modal_state(&[track]);

        assert!(matches!(
            &state.rows()[0],
            SelectionModalRow::Item(item) if item.name == " 1. Track Name"
        ));
    }

    #[test]
    fn album_modal_activation_replaces_queue_with_album_tracks() {
        let mut app = crate::app::tests::make_app_stub();
        crate::app::tests::install_test_emby(&mut app, crate::config::Config::default());
        let mut first = crate::app::tests::make_item("First", "Audio");
        first.id = "track-1".into();
        first.media_type = "Audio".into();
        let mut second = crate::app::tests::make_item("Second", "Audio");
        second.id = "track-2".into();
        second.media_type = "Audio".into();
        app.album_tracks_cache
            .insert("album-1".into(), vec![first.clone(), second.clone()]);
        app.open_selection_modal(
            source(),
            "Tracks".into(),
            SelectionModalListState::ready(vec![SelectionModalRow::Item(SelectionModalItem {
                name: second.name.clone(),
                meta: String::new(),
                id: second.id.clone(),
            })]),
            None,
        );

        app.activate_selection_modal_item();

        assert!(app.selection_modal.is_none());
        assert_eq!(app.player_tab.total_queue_len(), 2);
        assert_eq!(app.player_tab.queue_cursor, 1);
        assert!(matches!(
            app.queue_source,
            crate::config::QueueSource::Album
        ));
    }
}
