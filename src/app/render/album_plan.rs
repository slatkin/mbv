use super::album_art::INLINE_ALBUM_ART_ROWS;
use super::{natural_sort_key, strip_article};
use crate::app::layout::LibraryRowTarget;
use crate::app::music_grouping::{derive_album_display_name, GroupedAlbumCatalog};
use crate::app::{App, ArtistHeaderSelection};
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

pub(super) const SELECTED_ALBUM_WINDOW: usize = 12;

/// Sorted album display order for a set of `(artist, year, name)` info
/// triples: indices ordered by the artist's natural sort key (articles
/// stripped). Mirrors the catalog builder's sort so the fallback path
/// (no settled catalog yet) matches the settled ordering exactly.
pub(crate) fn sorted_group_album_order(album_info: &[(String, String, String)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..album_info.len()).collect();
    order.sort_by_key(|&i| natural_sort_key(strip_article(&album_info[i].0)));
    order
}

#[derive(Clone)]
pub(super) enum GroupedAlbumDisplayRow {
    ArtistHeader(ArtistHeaderSelection),
    ArtistGroupSpacer,
    AlbumDetailRule,
    AlbumWrappedContinuation,
    Album(usize),
    /// Action-hint row shown directly under the selected album's title when
    /// it is *not* expanded into full track-selection mode (`AlbumDetailStart`
    /// covers the hint once expanded) outside the music-group view.
    AlbumActionHint,
    /// Action-hint row shown directly under a selected artist header in a
    /// music-group selection block.
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
    pub(super) selected_group_indices: Option<Vec<usize>>,
    /// Absolute (unscrolled) indices into `rows` of the selected block's
    /// framing `AlbumDetailRule` rows — `(top_rule_idx, bottom_rule_idx)`.
    pub(super) selected_block_bounds: Option<(usize, usize)>,
    /// Absolute indices into `rows` of the track detail block — `(start_idx, end_idx)`.
    pub(super) track_detail_bounds: Option<(usize, usize)>,
}

impl GroupedAlbumDisplayRow {
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
    /// Builds the `(artist, year, album_name)` display info for every album,
    /// consuming the settled catalog when available (no artist derivation) and
    /// falling back to a synchronous best-effort chain otherwise.
    pub(super) fn group_album_info(
        &self,
        albums: &[mbv_core::api::MediaItem],
        catalog: Option<&GroupedAlbumCatalog>,
    ) -> Vec<(String, String, String)> {
        match catalog {
            Some(cat) => (0..albums.len())
                .map(|i| {
                    let pos = cat.index_to_entry.get(&i).copied().unwrap_or(0);
                    let entry = &cat.entries[pos];
                    (entry.artist.clone(), entry.year.clone(), entry.name.clone())
                })
                .collect(),
            None => albums
                .iter()
                .map(|item| {
                    let artist = self.resolve_group_album_artist(item);
                    let (year, name) = derive_album_display_name(item);
                    (artist, year, name)
                })
                .collect(),
        }
    }

    pub(super) fn build_grouped_album_display_plan(
        &mut self,
        albums: &[mbv_core::api::MediaItem],
        album_info: &[(String, String, String)],
        order: &[usize],
        cursor: usize,
        fetch_missing_tracks: bool,
        selectable_headers: bool,
        selected_artist_header: Option<&ArtistHeaderSelection>,
        expand_selected: bool,
        wrap_widths: Option<(u16, u16)>,
    ) -> GroupedAlbumDisplayPlan {
        let header_selected = selectable_headers && selected_artist_header.is_some();
        let inline_art_rows_after_album = if self.images_enabled() {
            INLINE_ALBUM_ART_ROWS.saturating_sub(1) as usize
        } else {
            0
        };
        let wrapped_lines = |text: &str, width: u16| wrap(text, width.max(1) as usize).len().max(1);
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
                            .saturating_sub(2)
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
                        full_width.saturating_sub(artwork_width).saturating_sub(2),
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
        let selected_detail_rows = |tracks: &[mbv_core::api::MediaItem], show_hint: bool| {
            let Some((full_width, artwork_width)) = wrap_widths else {
                return if show_hint {
                    2 + tracks.len()
                } else {
                    tracks.len()
                };
            };
            let table_width = full_width.saturating_sub(artwork_width).saturating_sub(4);
            let show_length = table_width > 40;
            let title_col_width =
                (table_width as usize).saturating_sub(4 + if show_length { 8 } else { 0 });
            let hint_overhead = if show_hint {
                let hint_width = table_width.saturating_sub(2).max(1) as usize;
                let hint_lines = wrap(
                    "^P: Play | ^A: Enqueue | ^S: Shuffle | BACK: Exit",
                    hint_width,
                )
                .len()
                .max(1);
                hint_lines + 1
            } else {
                0
            };
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
                        super::LIST_PLAY_ICON.width() + 1
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
            hint_overhead + track_lines
        };
        let mut rows: Vec<GroupedAlbumDisplayRow> = Vec::new();
        let mut has_artist_group = false;
        let mut selected_block_bounds: Option<(usize, usize)> = None;
        let mut track_detail_bounds: Option<(usize, usize)> = None;
        let mut selected_group_indices = None;
        let mut group_start = 0;
        while group_start < order.len() {
            let artist = album_info[order[group_start]].0.clone();
            let mut group_end = group_start + 1;
            while group_end < order.len() && album_info[order[group_end]].0 == artist {
                group_end += 1;
            }
            if has_artist_group {
                rows.push(GroupedAlbumDisplayRow::ArtistGroupSpacer);
            }
            let first_idx = order[group_start];
            let header_selection = ArtistHeaderSelection {
                first_album_id: albums[first_idx].id.clone(),
                artist_label: artist,
            };
            let group_contains_cursor = order[group_start..group_end].contains(&cursor);
            let selected_group = selectable_headers
                && ((header_selected && selected_artist_header == Some(&header_selection))
                    || (!header_selected && group_contains_cursor));

            if selected_group {
                let group_indices = order[group_start..group_end].to_vec();
                let window_start = if group_indices.len() > SELECTED_ALBUM_WINDOW {
                    let cursor_pos = group_indices
                        .iter()
                        .position(|&idx| idx == cursor)
                        .unwrap_or(0);
                    cursor_pos
                        .saturating_sub(SELECTED_ALBUM_WINDOW.saturating_sub(1))
                        .min(group_indices.len() - SELECTED_ALBUM_WINDOW)
                } else {
                    0
                };
                let window_end = (window_start + SELECTED_ALBUM_WINDOW).min(group_indices.len());
                let visible_group_indices = &group_indices[window_start..window_end];
                selected_group_indices = Some(group_indices.clone());
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                let top_idx = rows.len();
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                rows.push(GroupedAlbumDisplayRow::ArtistHeader(header_selection));
                if header_selected {
                    rows.push(GroupedAlbumDisplayRow::ArtistActionHint);
                    rows.extend(std::iter::repeat_n(
                        GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                        selected_hint_lines("^P: Play | ^A: Enqueue | ^S: Shuffle")
                            .saturating_sub(1),
                    ));
                } else {
                    rows.push(GroupedAlbumDisplayRow::AlbumActionHint);
                    let hint = if expand_selected {
                        "^P: Play | ^A: Enqueue | ^S: Shuffle | BACK: Exit"
                    } else {
                        "^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks"
                    };
                    rows.extend(std::iter::repeat_n(
                        GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                        selected_hint_lines(hint).saturating_sub(1),
                    ));
                }

                rows.push(GroupedAlbumDisplayRow::ArtistGroupSpacer);

                for &idx in visible_group_indices {
                    rows.push(GroupedAlbumDisplayRow::Album(idx));
                    rows.extend(std::iter::repeat_n(
                        GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                        selected_title_lines(idx).saturating_sub(1),
                    ));
                    if !header_selected && expand_selected && idx == cursor {
                        match self.album_tracks_cache.get(&albums[idx].id) {
                            Some(tracks) if !tracks.is_empty() => {
                                let detail_rows = selected_detail_rows(tracks, false);
                                let track_start = rows.len();
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailContinuation);
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailStart(idx));
                                rows.extend(
                                    std::iter::repeat_with(|| {
                                        GroupedAlbumDisplayRow::AlbumDetailContinuation
                                    })
                                    .take(detail_rows.saturating_sub(1)),
                                );
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailContinuation);
                                let track_end = rows.len();
                                track_detail_bounds = Some((track_start, track_end));
                            }
                            Some(_) => {}
                            None => {
                                if fetch_missing_tracks {
                                    self.fetch_album_tracks(albums[idx].id.clone());
                                }
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
                            }
                        }
                    }
                }

                let art_top = top_idx + 1;
                let art_rows = if self.images_enabled() {
                    INLINE_ALBUM_ART_ROWS as usize
                } else {
                    0
                };
                let art_bottom = art_top + art_rows;
                rows.extend(
                    std::iter::repeat_with(|| GroupedAlbumDisplayRow::AlbumDetailContinuation)
                        .take(art_bottom.saturating_sub(rows.len())),
                );
                let bottom_idx = rows.len();
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                selected_block_bounds = Some((top_idx, bottom_idx));
            } else {
                rows.push(GroupedAlbumDisplayRow::ArtistHeader(header_selection));
                for &idx in &order[group_start..group_end] {
                    if idx == cursor && !selectable_headers && !expand_selected {
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                        let top_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
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
                            std::iter::repeat_with(|| {
                                GroupedAlbumDisplayRow::AlbumDetailContinuation
                            })
                            .take(inline_art_rows_after_album.saturating_sub(1)),
                        );
                        let bottom_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                        selected_block_bounds = Some((top_idx, bottom_idx));
                    } else if idx == cursor && !selectable_headers {
                        match self.album_tracks_cache.get(&albums[idx].id) {
                            Some(tracks) if !tracks.is_empty() => {
                                let detail_rows = selected_detail_rows(tracks, true)
                                    .max(inline_art_rows_after_album);
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                                let top_idx = rows.len();
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
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
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                                selected_block_bounds = Some((top_idx, bottom_idx));
                            }
                            Some(_) => rows.push(GroupedAlbumDisplayRow::Album(idx)),
                            None => {
                                if fetch_missing_tracks {
                                    self.fetch_album_tracks(albums[idx].id.clone());
                                }
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                                let top_idx = rows.len();
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
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
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                                selected_block_bounds = Some((top_idx, bottom_idx));
                            }
                        }
                    } else {
                        rows.push(GroupedAlbumDisplayRow::Album(idx));
                    }
                }
            }
            has_artist_group = true;
            group_start = group_end;
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
            order: order.to_vec(),
            rows,
            display_cursor,
            selected_artist_header_valid,
            selected_group_indices,
            selected_block_bounds,
            track_detail_bounds,
        }
    }
}
