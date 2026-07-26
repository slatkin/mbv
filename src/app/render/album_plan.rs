use super::album_art::INLINE_ALBUM_ART_ROWS;
use super::{natural_sort_key, parse_album_folder_name, strip_article};
use crate::app::layout::LibraryRowTarget;
use crate::app::{App, ArtistHeaderSelection};
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

#[derive(Clone)]
pub(super) enum GroupedAlbumDisplayRow {
    ArtistHeader(ArtistHeaderSelection),
    ArtistGroupSpacer,
    AlbumDetailRule,
    AlbumArtist(usize),
    AlbumWrappedContinuation,
    Album(usize),
    /// Action-hint row shown directly under the selected album's title when
    /// it is *not* expanded into full track-selection mode (`AlbumDetailStart`
    /// covers the hint once expanded).
    AlbumActionHint,
    /// Action-hint row shown directly under a selected artist header.
    ArtistActionHint,
    AlbumDetailStart(usize),
    AlbumDetailContinuation,
    AlbumLoading,
}

pub(super) struct GroupedAlbumDisplayPlan {
    pub(super) order: Vec<usize>,
    pub(super) rows: Vec<GroupedAlbumDisplayRow>,
    pub(super) display_cursor: usize,
    pub(super) selected_artist_header_valid: bool,
    /// Absolute (unscrolled) indices into `rows` of the selected album's
    /// framing `AlbumDetailRule` rows — `(top_rule_idx, bottom_rule_idx)`.
    /// `None` when the selected album has no colored-block framing (header
    /// is the actual focus, or the track cache resolved to an empty vec).
    pub(super) selected_block_bounds: Option<(usize, usize)>,
}

impl GroupedAlbumDisplayRow {
    pub(super) fn album_index(&self) -> Option<usize> {
        match self {
            Self::Album(idx) => Some(*idx),
            _ => None,
        }
    }

    pub(super) fn row_target(&self, selectable_headers: bool) -> Option<LibraryRowTarget> {
        match self {
            Self::Album(idx) => Some(LibraryRowTarget::Album(*idx)),
            Self::ArtistHeader(selection) if selectable_headers => {
                Some(LibraryRowTarget::ArtistHeader(selection.clone()))
            }
            _ => None,
        }
    }
}

impl App {
    pub(super) fn album_artist_label(&self, item: &mbv_core::api::MediaItem) -> String {
        self.album_artist_cache
            .get(&item.id)
            .filter(|artist| !artist.is_empty())
            .cloned()
            .unwrap_or_else(|| item.artist.clone())
    }

    pub(super) fn build_grouped_album_display_plan(
        &mut self,
        albums: &[mbv_core::api::MediaItem],
        cursor: usize,
        fetch_missing_tracks: bool,
        selectable_headers: bool,
        selected_artist_header: Option<&ArtistHeaderSelection>,
        expand_selected: bool,
        wrap_widths: Option<(u16, u16)>,
    ) -> GroupedAlbumDisplayPlan {
        let mut album_info: Vec<(String, String, String)> = Vec::with_capacity(albums.len());
        for item in albums {
            let artist = self.resolve_group_album_artist(item);
            let (year_str, album_name) = if !item.artist.is_empty() {
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else {
                    String::new()
                };
                (year_str, item.display_name())
            } else if let Some((_, year, album)) = parse_album_folder_name(&item.name) {
                let year_str = if year > 0 {
                    year.to_string()
                } else {
                    String::new()
                };
                (year_str, album)
            } else {
                (String::new(), item.display_name())
            };
            album_info.push((artist, year_str, album_name));
        }

        let mut order: Vec<usize> = (0..album_info.len()).collect();
        order.sort_by_key(|&i| natural_sort_key(strip_article(&album_info[i].0)));

        // When an artist header itself is the focused row, no album beneath
        // it should still render as "selected" -- otherwise the album under
        // the cursor (which the header focus was entered from) keeps showing
        // its selected styling/hint/expansion alongside the header.
        let header_selected = selectable_headers && selected_artist_header.is_some();

        let inline_art_rows_after_album = if self.images_enabled() {
            INLINE_ALBUM_ART_ROWS.saturating_sub(1) as usize
        } else {
            0
        };
        let album_artist_labels: Vec<String> = albums
            .iter()
            .map(|item| self.album_artist_label(item))
            .collect();
        let wrapped_lines = |text: &str, width: u16| wrap(text, width.max(1) as usize).len().max(1);
        let selected_artist_lines = |idx: usize| {
            wrap_widths
                .map(|(full_width, _)| {
                    wrapped_lines(&album_artist_labels[idx], full_width.saturating_sub(1))
                })
                .unwrap_or(1)
        };
        let selected_title_lines = |idx: usize| {
            wrap_widths
                .map(|(full_width, artwork_width)| {
                    let suffix = if album_info[idx].1.is_empty() {
                        String::new()
                    } else {
                        format!(" • {}", album_info[idx].1)
                    };
                    let suffix_width = suffix.chars().count() as u16;
                    wrapped_lines(
                        &album_info[idx].2,
                        full_width
                            .saturating_sub(artwork_width)
                            .saturating_sub(1)
                            .saturating_sub(suffix_width),
                    )
                })
                .unwrap_or(1)
        };
        let selected_hint_lines = |text: &str| {
            wrap_widths
                .map(|(full_width, artwork_width)| {
                    wrapped_lines(
                        text,
                        full_width.saturating_sub(artwork_width).saturating_sub(1),
                    )
                })
                .unwrap_or(1)
        };
        let playing_track_id = {
            let playback = self.effective_playback_state();
            playback.active.then(|| {
                self.playback_queue()
                    .items
                    .get(playback.active_idx)
                    .map(|item| item.id.clone())
            })
        }
        .flatten();
        let use_nerd_fonts = self.use_nerd_fonts;
        let selected_detail_rows = |tracks: &[mbv_core::api::MediaItem]| {
            let Some((full_width, artwork_width)) = wrap_widths else {
                return 2 + tracks.len();
            };
            let table_width = full_width.saturating_sub(artwork_width);
            let show_length = table_width > 40;
            let title_col_width =
                (table_width as usize).saturating_sub(2 + if show_length { 8 } else { 0 });
            let hint_width = table_width.saturating_sub(1).max(1) as usize;
            let hint_lines = wrap(
                "^P: Play | ^A: Enqueue | ^S: Shuffle | BACK: Exit",
                hint_width,
            )
            .len()
            .max(1);
            let track_lines = tracks
                .iter()
                .enumerate()
                .map(|(i, track)| {
                    let track_num = if track.index_number > 0 {
                        format!("{}. ", track.index_number)
                    } else {
                        format!("{}. ", i + 1)
                    };
                    let play_width = if playing_track_id.as_deref() == Some(track.id.as_str()) {
                        super::play_icon(use_nerd_fonts).width() + 1
                    } else {
                        0
                    };
                    wrap(
                        &track.name,
                        title_col_width
                            .saturating_sub(track_num.chars().count() + play_width)
                            .max(1),
                    )
                    .len()
                    .max(1)
                })
                .sum::<usize>();
            hint_lines + 1 + track_lines
        };

        let mut rows: Vec<GroupedAlbumDisplayRow> = Vec::new();
        let mut last_artist = String::new();
        let mut has_artist_group = false;
        let mut selected_block_bounds: Option<(usize, usize)> = None;
        for &idx in &order {
            let artist = &album_info[idx].0;
            if artist != &last_artist {
                if has_artist_group {
                    rows.push(GroupedAlbumDisplayRow::ArtistGroupSpacer);
                }
                let header_selection = ArtistHeaderSelection {
                    first_album_id: albums[idx].id.clone(),
                    artist_label: artist.clone(),
                };
                let this_header_selected =
                    header_selected && selected_artist_header == Some(&header_selection);
                if this_header_selected {
                    // Wrap the selected artist header in the same colored
                    // block frame as a selected album (see the `!expand_selected`
                    // album branch below): border space, bg padding, the
                    // header row itself, an action-hint row, filler rows so
                    // the block is tall enough for the collage, bg padding,
                    // border space.
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                    let top_idx = rows.len();
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                    rows.push(GroupedAlbumDisplayRow::ArtistHeader(header_selection));
                    rows.push(GroupedAlbumDisplayRow::ArtistActionHint);
                    rows.extend(
                        std::iter::repeat_with(|| GroupedAlbumDisplayRow::AlbumDetailContinuation)
                            .take(inline_art_rows_after_album.saturating_sub(1)),
                    );
                    let bottom_idx = rows.len();
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                    selected_block_bounds = Some((top_idx, bottom_idx));
                } else {
                    rows.push(GroupedAlbumDisplayRow::ArtistHeader(header_selection));
                }
                last_artist = artist.clone();
                has_artist_group = true;
            }
            if idx == cursor && header_selected {
                rows.push(GroupedAlbumDisplayRow::Album(idx));
            } else if idx == cursor && !expand_selected {
                // Hint-only state (album selected, tracks not yet shown): wrap in block frame
                // Insert extra detail rule rows for borders (one before, one after the colored block)
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                let top_idx = rows.len();
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                rows.push(GroupedAlbumDisplayRow::AlbumArtist(idx));
                rows.extend(std::iter::repeat_n(
                    GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                    selected_artist_lines(idx).saturating_sub(1),
                ));
                rows.push(GroupedAlbumDisplayRow::Album(idx));
                rows.extend(std::iter::repeat_n(
                    GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                    selected_title_lines(idx).saturating_sub(1),
                ));
                rows.push(GroupedAlbumDisplayRow::AlbumActionHint);
                rows.extend(std::iter::repeat_n(
                    GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                    selected_hint_lines(
                        "^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks",
                    )
                    .saturating_sub(1),
                ));
                rows.extend(
                    std::iter::repeat_with(|| GroupedAlbumDisplayRow::AlbumDetailContinuation)
                        .take(inline_art_rows_after_album.saturating_sub(1)),
                );
                let bottom_idx = rows.len();
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                selected_block_bounds = Some((top_idx, bottom_idx));
            } else if idx == cursor {
                match self.album_tracks_cache.get(&albums[idx].id) {
                    Some(tracks) if !tracks.is_empty() => {
                        let detail_rows =
                            selected_detail_rows(tracks).max(inline_art_rows_after_album);
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                        let top_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                        rows.push(GroupedAlbumDisplayRow::AlbumArtist(idx));
                        rows.extend(std::iter::repeat_n(
                            GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                            selected_artist_lines(idx).saturating_sub(1),
                        ));
                        rows.push(GroupedAlbumDisplayRow::Album(idx));
                        rows.extend(std::iter::repeat_n(
                            GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                            selected_title_lines(idx).saturating_sub(1),
                        ));
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailStart(idx));
                        rows.extend(
                            std::iter::repeat_with(|| {
                                GroupedAlbumDisplayRow::AlbumDetailContinuation
                            })
                            .take(detail_rows.saturating_sub(1)),
                        );
                        let bottom_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                        selected_block_bounds = Some((top_idx, bottom_idx));
                    }
                    Some(_) => rows.push(GroupedAlbumDisplayRow::Album(idx)),
                    None => {
                        if fetch_missing_tracks {
                            self.fetch_album_tracks(albums[idx].id.clone());
                        }
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                        let top_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                        rows.push(GroupedAlbumDisplayRow::AlbumArtist(idx));
                        rows.extend(std::iter::repeat_n(
                            GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                            selected_artist_lines(idx).saturating_sub(1),
                        ));
                        rows.push(GroupedAlbumDisplayRow::Album(idx));
                        rows.extend(std::iter::repeat_n(
                            GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                            selected_title_lines(idx).saturating_sub(1),
                        ));
                        rows.push(GroupedAlbumDisplayRow::AlbumLoading);
                        rows.extend(std::iter::repeat_n(
                            GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                            selected_hint_lines("Loading…").saturating_sub(1),
                        ));
                        rows.extend(
                            std::iter::repeat_with(|| {
                                GroupedAlbumDisplayRow::AlbumDetailContinuation
                            })
                            .take(inline_art_rows_after_album.saturating_sub(1)),
                        );
                        let bottom_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                        selected_block_bounds = Some((top_idx, bottom_idx));
                    }
                }
            } else {
                rows.push(GroupedAlbumDisplayRow::Album(idx));
            }
        }

        let display_cursor = rows
            .iter()
            .position(|row| {
                selectable_headers
                    && matches!(
                        (row, selected_artist_header),
                        (
                            GroupedAlbumDisplayRow::ArtistHeader(selection),
                            Some(selected)
                        ) if selection == selected
                    )
            })
            .or_else(|| {
                rows.iter()
                    .position(|row| matches!(row, GroupedAlbumDisplayRow::Album(i) if *i == cursor))
            })
            .unwrap_or(0);
        let selected_artist_header_valid = selected_artist_header.is_some_and(|selected| {
            selectable_headers
                && rows.iter().any(|row| {
                    matches!(row, GroupedAlbumDisplayRow::ArtistHeader(selection) if selection == selected)
                })
        });

        GroupedAlbumDisplayPlan {
            order,
            rows,
            display_cursor,
            selected_artist_header_valid,
            selected_block_bounds,
        }
    }
}
