use crate::app::layout::LibraryRowTarget;
use crate::app::music_grouping::{derive_album_display_name, GroupedAlbumCatalog};
use crate::app::render::components::album_art::INLINE_ALBUM_ART_ROWS;
use crate::app::render::{natural_sort_key, strip_article};
use std::collections::HashMap;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

/// Display-only artist group header carried in the display plan for
/// rendering. Not a selection or navigation target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app::render) struct ArtistGroupHeader {
    pub(in crate::app::render) first_album_id: String,
    pub(in crate::app::render) artist_label: String,
}

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
pub(in crate::app::render) enum GroupedAlbumDisplayRow {
    ArtistHeader(ArtistGroupHeader),
    ArtistGroupSpacer,
    AlbumDetailRule,
    AlbumWrappedContinuation,
    Album(usize),
    AlbumInlineDetailStart(usize),
    /// Action-hint row shown directly under the selected album's title when
    /// it is *not* expanded into full track-selection mode (`AlbumDetailStart`
    /// covers the hint once expanded) outside the music-group view.
    AlbumActionHint,
    AlbumDetailStart(usize),
    AlbumDetailContinuation,
    AlbumLoading,
}

/// Display-plan build context for grouped album views.
pub(in crate::app::render) struct HeaderFocusCtx {
    /// True when the music-group (pill-selector) view is active, enabling
    /// the selected-group block frame and two-column layout.
    pub(in crate::app::render) in_music_group_view: bool,
    pub(in crate::app::render) expand_selected: bool,
}

pub(in crate::app::render) struct GroupedAlbumDisplayPlan {
    pub(in crate::app::render) order: Vec<usize>,
    pub(in crate::app::render) rows: Vec<GroupedAlbumDisplayRow>,
    pub(in crate::app::render) display_cursor: usize,
    /// Absolute (unscrolled) indices into `rows` of the selected block's
    /// framing `AlbumDetailRule` rows — `(top_rule_idx, bottom_rule_idx)`.
    pub(in crate::app::render) selected_block_bounds: Option<(usize, usize)>,
    /// Absolute indices into `rows` of the track detail block — `(start_idx, end_idx)`.
    pub(in crate::app::render) track_detail_bounds: Option<(usize, usize)>,
}

impl GroupedAlbumDisplayRow {
    pub(in crate::app::render) fn row_target(&self) -> Option<LibraryRowTarget> {
        match self {
            Self::Album(idx) | Self::AlbumInlineDetailStart(idx) => {
                Some(LibraryRowTarget::Album(*idx))
            }
            _ => None,
        }
    }
}

pub(in crate::app::render) struct GroupedAlbumDisplayPlanCtx<'a> {
    pub(in crate::app::render) images_enabled: bool,
    pub(in crate::app::render) playing_track_id: Option<String>,
    pub(in crate::app::render) album_tracks: &'a HashMap<String, Vec<mbv_core::api::EmbyItem>>,
}

/// Resolves the display artist for an album item in the grouped music views
/// synchronously, given the album-artist cache. Mirrors
/// `App::resolve_group_album_artist` without borrowing `App`.
pub(in crate::app::render) fn resolve_group_album_artist(
    album_artist_cache: &HashMap<String, String>,
    item: &mbv_core::api::EmbyItem,
) -> String {
    crate::app::music_grouping::derive_album_artist(
        item,
        album_artist_cache.get(&item.id).map(String::as_str),
    )
}

/// Builds the `(artist, year, album_name)` display info for every album,
/// consuming the settled catalog when available (no artist derivation) and
/// falling back to a synchronous best-effort chain otherwise.
pub(in crate::app::render) fn group_album_info(
    album_artist_cache: &HashMap<String, String>,
    albums: &[mbv_core::api::EmbyItem],
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
                let artist = resolve_group_album_artist(album_artist_cache, item);
                let (year, name) = derive_album_display_name(item);
                (artist, year, name)
            })
            .collect(),
    }
}

pub(in crate::app::render) fn build_grouped_album_display_plan_with_ctx(
    albums: &[mbv_core::api::EmbyItem],
    album_info: &[(String, String, String)],
    order: &[usize],
    cursor: usize,
    _fetch_missing_tracks: bool,
    header_focus: HeaderFocusCtx,
    wrap_widths: Option<(u16, u16)>,
    hero_handles_detail: bool,
    ctx: GroupedAlbumDisplayPlanCtx<'_>,
) -> GroupedAlbumDisplayPlan {
    let HeaderFocusCtx {
        in_music_group_view,
        expand_selected,
    } = header_focus;
    let inline_art_rows_after_album = if ctx.images_enabled {
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
    let playing_track_id = ctx.playing_track_id.as_deref();
    let selected_detail_rows = |tracks: &[mbv_core::api::EmbyItem], show_hint: bool| {
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
                let play_width = if playing_track_id == Some(track.id.as_str()) {
                    crate::app::render::LIST_PLAY_ICON.width() + 1
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
        let group_header = ArtistGroupHeader {
            first_album_id: albums[first_idx].id.clone(),
            artist_label: artist,
        };
        let group_contains_cursor = order[group_start..group_end].contains(&cursor);
        let selected_group = in_music_group_view && group_contains_cursor;

        if selected_group {
            let group_indices = order[group_start..group_end].to_vec();
            rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
            let top_idx = rows.len();
            rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
            rows.push(GroupedAlbumDisplayRow::ArtistHeader(group_header));
            rows.push(GroupedAlbumDisplayRow::AlbumActionHint);
            let hint = if expand_selected {
                "^P: Play | ^A: Enqueue | ^S: Shuffle | BACK: Exit"
            } else {
                "^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks"
            };
            if !hero_handles_detail {
                rows.extend(std::iter::repeat_n(
                    GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                    selected_hint_lines(hint).saturating_sub(1),
                ));
            }

            for &idx in &group_indices {
                if !hero_handles_detail && idx == cursor {
                    rows.push(GroupedAlbumDisplayRow::AlbumInlineDetailStart(idx));
                    match ctx.album_tracks.get(&albums[idx].id) {
                        Some(tracks) if !tracks.is_empty() => {
                            let detail_rows = selected_detail_rows(tracks, false);
                            rows.extend(
                                std::iter::repeat_with(|| {
                                    GroupedAlbumDisplayRow::AlbumDetailContinuation
                                })
                                .take(detail_rows),
                            );
                        }
                        Some(_) => {}
                        None => {
                            rows.push(GroupedAlbumDisplayRow::AlbumLoading);
                        }
                    }
                } else {
                    rows.push(GroupedAlbumDisplayRow::Album(idx));
                    rows.extend(std::iter::repeat_n(
                        GroupedAlbumDisplayRow::AlbumWrappedContinuation,
                        selected_title_lines(idx).saturating_sub(1),
                    ));
                    if expand_selected && idx == cursor {
                        match ctx.album_tracks.get(&albums[idx].id) {
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
            }

            let art_top = top_idx + 1;
            let art_rows = if ctx.images_enabled {
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
            rows.push(GroupedAlbumDisplayRow::ArtistHeader(group_header));
            for &idx in &order[group_start..group_end] {
                if idx == cursor && !in_music_group_view && !expand_selected {
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
                        std::iter::repeat_with(|| GroupedAlbumDisplayRow::AlbumDetailContinuation)
                            .take(inline_art_rows_after_album.saturating_sub(1)),
                    );
                    let bottom_idx = rows.len();
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                    selected_block_bounds = Some((top_idx, bottom_idx));
                } else if idx == cursor && !in_music_group_view {
                    match ctx.album_tracks.get(&albums[idx].id) {
                        Some(tracks) if !tracks.is_empty() => {
                            let detail_rows =
                                selected_detail_rows(tracks, true).max(inline_art_rows_after_album);
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

    let find_display_cursor = |rows: &[GroupedAlbumDisplayRow]| -> usize {
        rows.iter()
            .position(|row| row.row_target() == Some(LibraryRowTarget::Album(cursor)))
            .unwrap_or(0)
    };
    let display_cursor = find_display_cursor(&rows);

    // When the hero panel handles the detail rendering, suppress the
    // inline detail rows and clear the bounds that reference them.
    if hero_handles_detail {
        rows.retain(|row| {
            !matches!(
                row,
                GroupedAlbumDisplayRow::AlbumDetailStart(_)
                    | GroupedAlbumDisplayRow::AlbumInlineDetailStart(_)
                    | GroupedAlbumDisplayRow::AlbumDetailContinuation
                    | GroupedAlbumDisplayRow::AlbumDetailRule
                    | GroupedAlbumDisplayRow::AlbumLoading
                    | GroupedAlbumDisplayRow::AlbumActionHint
            )
        });
        return GroupedAlbumDisplayPlan {
            order: order.to_vec(),
            display_cursor: find_display_cursor(&rows),
            rows,
            selected_block_bounds: None,
            track_detail_bounds: None,
        };
    }

    GroupedAlbumDisplayPlan {
        order: order.to_vec(),
        rows,
        display_cursor,
        selected_block_bounds,
        track_detail_bounds,
    }
}
