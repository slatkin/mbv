// The module's documentation and shared imports live in the parent module.
use super::workspace::track_row_label;
use super::{MediaSemanticState, MusicContent, MusicTreeTarget};
use crate::app::components::list::tree_browser::{TreeMarkPolicy, TreeNode};
use std::collections::HashMap;

// The destination's stable-target translation layer (design D6): Music
// resolves its own domain facts — the selected album/track identity, the
// focused artist root's settled scope, the ordered mark scope, the settled
// node projection, and the neighbour prefetch window — from the shared
// browser's stable-target surfaces, never from its internals.

impl MusicContent {
    /// Resolves the selected tree track to stable identities only. The shell
    /// owns the cached Emby items and resolves the playback queue.
    pub(super) fn selected_tree_track(&self) -> Option<(String, String)> {
        let (album_target, track_target) = self.selected_track_identity()?;
        self.tree_tracks
            .get(&album_target)?
            .iter()
            .find(|track| track.id == track_target)?;
        Some((album_target, track_target))
    }

    /// The selected node's album identity: an album leaf's own target, or the
    /// owning album of a selected cached track. An artist root resolves to no
    /// album (task 2.2: artist focus never writes album persistence).
    pub(in crate::app) fn selected_album_target(&self) -> Option<String> {
        match self.browser.selected_target()? {
            MusicTreeTarget::Album(target) => Some(target.clone()),
            MusicTreeTarget::Track { album, .. } => Some(album.clone()),
            MusicTreeTarget::Artist(_) => None,
        }
    }

    /// The selected cached track's stable `(album target, track target)`
    /// identity, if the tree is on a track item rather than an artist or
    /// album row.
    pub(in crate::app) fn selected_track_identity(&self) -> Option<(String, String)> {
        match self.browser.selected_target()? {
            MusicTreeTarget::Track { album, track } => Some((album.clone(), track.clone())),
            _ => None,
        }
    }

    /// Whether the selected node is an artist root (task 2.2: an artist focus
    /// resolves to no album and never writes album persistence).
    pub(in crate::app) fn selected_is_artist(&self) -> bool {
        matches!(
            self.browser.selected_target(),
            Some(MusicTreeTarget::Artist(_))
        )
    }

    /// Whether the selected node is a cached track row.
    pub(super) fn selected_is_track(&self) -> bool {
        matches!(
            self.browser.selected_target(),
            Some(MusicTreeTarget::Track { .. })
        )
    }

    /// Whether an album leaf is in the shared owner's current visible flow.
    /// Without an active filter every projected leaf is visible; the filter
    /// session is the visibility predicate.
    fn album_target_visible(&self, target: &MusicTreeTarget) -> bool {
        if !self.browser.filter_active() {
            return true;
        }
        self.browser
            .visible_targets()
            .iter()
            .any(|visible| visible == target)
    }

    /// Album targets marked in insertion order. Hidden marks are masked while
    /// a filter is active but remain in the shared mark set for dismissal.
    pub(in crate::app) fn selected_album_targets(&self) -> Vec<String> {
        self.browser
            .marked_targets()
            .iter()
            .filter_map(|target| target.album_leaf_target().map(str::to_string))
            .filter(|album| self.album_target_visible(&MusicTreeTarget::Album(album.clone())))
            .collect()
    }

    /// Album targets marked in the current settled tree display order. This
    /// is the order boundary-crossing actions use, independent of click
    /// order. The walk reads the shared owner's root/child structure rather
    /// than the expanded flow, because expansion never participates in the
    /// action scope: a collapsed root contributes the same marked albums as
    /// an expanded one. The shared ordered-mark set supplies membership only;
    /// it is never the ordering source.
    pub(in crate::app) fn selected_album_targets_in_display_order(&self) -> Vec<String> {
        let mut targets = Vec::new();
        for root in self.browser.roots() {
            let Some(children) = self.browser.children_of(root) else {
                continue;
            };
            for child in children {
                let Some(album) = child.album_leaf_target() else {
                    continue;
                };
                if self.album_target_visible(child)
                    && self
                        .browser
                        .marked_targets()
                        .iter()
                        .any(|marked| marked.album_leaf_target() == Some(album))
                {
                    targets.push(album.to_string());
                }
            }
        }
        targets
    }

    /// The focused artist root's settled album targets in settled order. The
    /// walk reads the owner's child list rather than the expanded flow: a
    /// collapsed root has the same action scope as an expanded root. When the
    /// filter session is active, the current visible flow is the visibility
    /// predicate, so only matching leaves cross the component boundary. A
    /// non-artist selection is not an artist action.
    pub(super) fn selected_artist_album_targets(&self) -> Option<Vec<String>> {
        let selected = self.browser.selected_target()?.clone();
        if !matches!(selected, MusicTreeTarget::Artist(_)) {
            return None;
        }
        let children = self.browser.children_of(&selected)?;
        Some(
            children
                .iter()
                .filter(|child| self.album_target_visible(child))
                .filter_map(|child| child.album_leaf_target().map(str::to_string))
                .collect(),
        )
    }

    /// Resolves an artist root's currently visible album descendants in tree
    /// order for context-hit membership checks. A target the owner does not
    /// hold is an explicit empty result.
    pub(super) fn artist_album_targets(&self, root: &MusicTreeTarget) -> Vec<String> {
        self.browser
            .children_of(root)
            .into_iter()
            .flatten()
            .filter(|child| self.album_target_visible(child))
            .filter_map(|child| child.album_leaf_target().map(str::to_string))
            .collect()
    }

    /// The settled Grouped Music projection the shared owner reconciles
    /// (design D6): artist roots grouped by the settled `ArtistKey` in
    /// first-occurrence order, each with its album leaves in settled order
    /// and the album's cached track children. Stored played/unplayed facts
    /// are deliberately ignored for music rows; a positive position still
    /// retains `Active`.
    pub(super) fn tree_projection(&self) -> Vec<TreeNode<MusicTreeTarget>> {
        let mut nodes: Vec<TreeNode<MusicTreeTarget>> = Vec::new();
        let mut root_of_key: HashMap<
            crate::app::state::music_grouping::ArtistKey,
            MusicTreeTarget,
        > = HashMap::new();
        for &index in &self.context.album_order {
            let Some((artist, year, name)) = self.context.album_info.get(index) else {
                continue;
            };
            let Some(artist_key) = self.context.album_artist_keys.get(index) else {
                continue;
            };
            let Some(album_target) = self.context.album_targets.get(index) else {
                continue;
            };
            let artist_target = if let Some(existing) = root_of_key.get(artist_key) {
                existing.clone()
            } else {
                let target = MusicTreeTarget::Artist(artist_key.clone());
                root_of_key.insert(artist_key.clone(), target.clone());
                nodes.push(
                    TreeNode::new(
                        target.clone(),
                        None,
                        artist.clone(),
                        artist.clone(),
                        MediaSemanticState::Ordinary,
                        TreeMarkPolicy::Aggregate,
                    )
                    .with_title_role(
                        crate::app::components::list::tree_browser::TreeTitleRole::Heading,
                    )
                    .with_expandable(true),
                );
                target
            };
            // The tree's album projection is music by owner context even
            // when Emby represents its rows as `Folder`. Ignore stored played
            // state while retaining a positive playback position as the live
            // `Active` distinction.
            let semantic_state =
                self.context
                    .list
                    .items
                    .get(index)
                    .map_or(MediaSemanticState::Ordinary, |item| {
                        MediaSemanticState::from_progress(
                            false,
                            item.playback_position_ticks,
                            item.runtime_ticks,
                        )
                    });
            let album_node = TreeNode::new(
                MusicTreeTarget::Album(album_target.clone()),
                Some(artist_target),
                name.clone(),
                format!("{name} {}", year.as_str()),
                semantic_state,
                TreeMarkPolicy::Direct,
            )
            .with_title_role(crate::app::components::list::tree_browser::TreeTitleRole::Secondary)
            .with_expandable(
                self.tree_tracks
                    .get(album_target)
                    .is_some_and(|tracks| !tracks.is_empty()),
            );
            let album_node = if year.is_empty() {
                album_node
            } else {
                album_node.with_trailing(year.clone())
            };
            nodes.push(album_node);
            for (track_index, track) in self
                .tree_tracks
                .get(album_target)
                .into_iter()
                .flatten()
                .enumerate()
            {
                nodes.push(TreeNode::new(
                    MusicTreeTarget::Track {
                        album: album_target.clone(),
                        track: track.id.clone(),
                    },
                    Some(MusicTreeTarget::Album(album_target.clone())),
                    track_row_label(track, track_index),
                    track.name.clone(),
                    // Track rows are grouping leaves: playback emphasis and
                    // mark membership belong to their album.
                    MediaSemanticState::Ordinary,
                    TreeMarkPolicy::Excluded,
                ));
            }
        }
        nodes
    }
}
