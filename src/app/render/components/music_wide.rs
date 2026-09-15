//! Grouped Music's wide Wide hero component.

use crate::app::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState,
};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::App;
use mbv_core::api::EmbyItem;
use std::collections::HashMap;

#[derive(Clone)]
pub(in crate::app) struct MusicWideRenderCtx {
    pub(in crate::app) list: LibraryListRenderCtx,
    pub(in crate::app) album_targets: Vec<String>,
    pub(in crate::app) selected_album: Option<EmbyItem>,
    pub(in crate::app) groups: Vec<EmbyItem>,
    pub(in crate::app) group_cursor: usize,
    pub(in crate::app) album_info: Vec<(String, String, String)>,
    pub(in crate::app) album_order: Vec<usize>,
    pub(in crate::app) focused: bool,
    pub(in crate::app) album_tracks: Option<Vec<EmbyItem>>,
}

impl MusicWideRenderCtx {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::app) fn new(
        list: LibraryListRenderCtx,
        selected_album: Option<EmbyItem>,
        _album_artist: String,
        groups: Vec<EmbyItem>,
        group_cursor: usize,
        album_info: Vec<(String, String, String)>,
        album_order: Vec<usize>,
        album_tracks: Option<Vec<EmbyItem>>,
    ) -> Self {
        let mut counts = HashMap::<&str, usize>::new();
        for album in &list.items {
            *counts.entry(&album.id).or_default() += 1;
        }
        let album_targets = list
            .items
            .iter()
            .enumerate()
            .map(|(index, album)| {
                if counts[album.id.as_str()] > 1 {
                    format!("{}\u{0}{index}", album.id)
                } else {
                    album.id.clone()
                }
            })
            .collect();
        Self {
            list,
            album_targets,
            selected_album,
            groups,
            group_cursor,
            album_info,
            album_order,
            // Framework focus is owned by `MusicWorkspaceComponent` and applied
            // from `Attribute::Focus`; content projection never sets it.
            focused: false,
            album_tracks,
        }
    }
}

impl MusicWideRenderCtx {
    /// Canonical row projection shared by the wide `WideMediaList` and the
    /// one-column fixed-row browser: one `Heading` per artist group, a `Spacer`
    /// between groups, and one selectable `Item` per album keyed by its stable
    /// id. Grouped Music album rows carry no played/active state (parity with
    /// the wide rail and the legacy painter).
    pub(in crate::app) fn grouped_rows(&self) -> Vec<MediaListRow<String>> {
        grouped_album_rows_with_targets(&self.album_info, &self.album_order, &self.album_targets)
    }
}

fn grouped_album_rows_with_targets(
    album_info: &[(String, String, String)],
    order: &[usize],
    targets: &[String],
) -> Vec<MediaListRow<String>> {
    let mut rows = Vec::new();
    let mut start = 0;
    while start < order.len() {
        let artist = album_info[order[start]].0.clone();
        let mut end = start + 1;
        while end < order.len() && album_info[order[end]].0 == artist {
            end += 1;
        }
        if start > 0 {
            rows.push(MediaListRow::Spacer);
        }
        rows.push(MediaListRow::Heading { text: artist });
        for &idx in &order[start..end] {
            let (_, year, name) = &album_info[idx];
            rows.push(MediaListRow::Item {
                target: targets[idx].clone(),
                primary: name.clone(),
                secondary: None,
                trailing: (!year.is_empty()).then(|| MediaListTrailing::Year(year.clone())),
                duration: None,
                kind: MediaKind::Collection,
                semantic_state: MediaSemanticState::Ordinary,
            });
        }
        start = end;
    }
    rows
}

impl App {
    pub(in crate::app) fn wide_music_render_ctx(
        &self,
        lib_idx: usize,
        cursor_scroll: Option<(usize, usize)>,
    ) -> MusicWideRenderCtx {
        let list = self.library_list_render_ctx(
            lib_idx,
            cursor_scroll.map_or(0, |v| v.0),
            cursor_scroll.map_or(0, |v| v.1),
        );
        let lib = &self.libs[lib_idx];
        let level = lib.nav_stack.last();
        let selected_cursor = cursor_scroll.map_or(0, |(cursor, _)| cursor);
        let selected_album = level
            .and_then(|level| level.items.get(selected_cursor))
            .cloned();
        let (groups, group_cursor) = if lib.nav_stack.len() >= 2 {
            let group = &lib.nav_stack[lib.nav_stack.len() - 2];
            (group.items.clone(), group.resting().cursor())
        } else {
            (Vec::new(), 0)
        };
        let albums = level.map(|level| level.items.clone()).unwrap_or_default();
        let catalog = level
            .and_then(|level| level.music_grouping.as_ref())
            .and_then(|state| state.settled.clone());
        let album_info = crate::app::render::screens::album_plan::group_album_info(
            &self.album_artist_cache,
            &albums,
            catalog.as_ref(),
        );
        let album_order = catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .entries
                    .iter()
                    .map(|entry| entry.album_index)
                    .filter(|&index| index < albums.len())
                    .collect()
            })
            .unwrap_or_else(|| crate::app::render::sorted_group_album_order(&album_info));
        let album_tracks = selected_album
            .as_ref()
            .and_then(|album| self.album_tracks_cache.get(&album.id).cloned());

        MusicWideRenderCtx::new(
            list,
            selected_album,
            String::new(),
            groups,
            group_cursor,
            album_info,
            album_order,
            album_tracks,
        )
    }
}
