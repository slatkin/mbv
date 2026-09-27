//! Grouped Music's wide Wide hero component.

use crate::components::list_rows::LibraryListRenderCtx;
use mbv_emby_model::EmbyItem;
use mbv_ui_model::music_grouping::ArtistKey;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct MusicWideRenderCtx {
    pub list: LibraryListRenderCtx,
    pub album_targets: Vec<String>,
    pub selected_album: Option<EmbyItem>,
    pub groups: Vec<EmbyItem>,
    pub group_cursor: usize,
    pub album_info: Vec<(String, String, String)>,
    /// Stable artist identity per album (parallel to `album_info`, design
    /// D2): the settled catalog's resolved `ArtistItems` identity or its
    /// deterministic fallback key. The Grouped Music tree owner groups the
    /// settled albums into artist roots by this key.
    pub album_artist_keys: Vec<ArtistKey>,
    pub album_order: Vec<usize>,
    pub focused: bool,
    pub album_tracks: Option<Vec<EmbyItem>>,
    /// Settled source revision carried with the tree snapshot (design D7):
    /// artist detail requests bind to it so a completion from a replaced
    /// catalog can never become the new snapshot's Workspace.
    pub catalog_revision: u64,
    /// The focused artist's projected summary and grouped track Workspace
    /// (tasks 6.2/6.3), when the tree owner is on an artist root.
    pub artist_detail: Option<mbv_ui_model::music_artist_detail::ArtistDetailProjection>,
}

impl MusicWideRenderCtx {
    #[must_use]
    pub fn new(
        list: LibraryListRenderCtx,
        selected_album: Option<EmbyItem>,
        _album_artist: String,
        groups: Vec<EmbyItem>,
        group_cursor: usize,
        album_info: Vec<(String, String, String)>,
        album_artist_keys: Vec<ArtistKey>,
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
            album_artist_keys,
            album_order,
            // Framework focus is owned by `MusicWorkspaceComponent` and applied
            // from `Attribute::Focus`; content projection never sets it.
            focused: false,
            album_tracks,
            catalog_revision: 0,
            artist_detail: None,
        }
    }

    #[must_use]
    pub fn with_catalog_revision(mut self, revision: u64) -> Self {
        self.catalog_revision = revision;
        self
    }
}
