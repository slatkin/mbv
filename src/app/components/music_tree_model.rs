// Included into `music_tree` via `include!` (the module's doc and
// imports live there, beside the split's other parts).

/// One settled Grouped Music album row the browser projects: the stable
/// already keys albums by, plus its settled artist identity and the display
/// text the row paints.
#[derive(Clone)]
pub(in crate::app) struct MusicTreeEntry {
    pub(in crate::app) artist: String,
    pub(in crate::app) artist_key: ArtistKey,
    pub(in crate::app) title: String,
    pub(in crate::app) year: Option<String>,
    pub(in crate::app) target: String,
    /// The settled item's playback-live semantic state. Stored played/unplayed
    /// facts never enter the tree; only `Active`/`NowPlaying` distinctions are
    /// retained for the label renderer.
    pub(in crate::app) semantic_state: MediaSemanticState,
}

/// A cached track projected below an album leaf. The browser needs the
/// stable Workspace target, painted label, and searchable title; the owning
/// Music component retains the full `EmbyItem` to resolve activation through
/// the existing playback arm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct MusicTreeTrack {
    pub(in crate::app) target: String,
    pub(in crate::app) title: String,
    pub(in crate::app) search_title: String,
}

enum MusicNode {
    Artist {
        /// The node's own settled identity (node-to-domain translation, D2).
        key: ArtistKey,
        name: String,
    },
    Album {
        artist: String,
        title: String,
        year: Option<String>,
        target: String,
        semantic_state: MediaSemanticState,
    },
    Track {
        album_target: String,
        target: String,
        title: String,
        search_title: String,
    },
}

/// The destination-local node arena behind the crate's `TreeModel` (design
/// D2). Node ids are arena indexes into a monotonic, append-only `Vec`:
/// interned by semantic key, stable across ordinary settled-catalog
/// replacement, and never reused for a different key during the owner's
/// lifetime. Entries removed by a replacement leave the root/child
/// projection (tombstoned) while their interned mapping is retained; the
/// arena resets only when the retained Music destination changes identity
/// (`reset`).
pub(in crate::app) struct MusicTreeModel {
    nodes: Vec<MusicNode>,
    /// Stable target per arena id, aligned with `nodes`. Node ids are
    /// append-only, so a target can be borrowed by the seam
    /// (`Cursored::selected_target`) without rebuilding one per call.
    targets: Vec<MusicTreeTarget>,
    intern: HashMap<MusicNodeKey, usize>,
    roots: Vec<usize>,
    /// Per node id: its artist root's settled child leaves. Album nodes and
    /// tombstoned artist roots carry an empty child list.
    children: Vec<Vec<usize>>,
    /// Per node: its top-level artist group's settled order (the group zebra
    /// phase). Descendant albums and tracks share their root's phase;
    /// `usize::MAX` marks a tombstone.
    root_position: Vec<usize>,
    revision: TreeRevision,
}

impl MusicTreeModel {
    pub(in crate::app) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            targets: Vec::new(),
            intern: HashMap::new(),
            roots: Vec::new(),
            children: Vec::new(),
            root_position: Vec::new(),
            revision: TreeRevision::INITIAL,
        }
    }

    /// A model over one settled entry set, for callers that do not keep an
    /// incremental owner.
    #[cfg(test)]
    pub(in crate::app) fn from_entries(entries: &[MusicTreeEntry]) -> Self {
        let mut model = Self::new();
        model.reconcile(entries);
        model
    }

    /// Reconciles the arena with one settled catalog (design D2/D3): intern
    /// artist roots and album leaves in settled order, then atomically
    /// replace the root/child projection, bumping the model revision only
    /// when the settled content actually changed. Settled entries sharing
    /// one `ArtistKey` form one artist root, with roots in first-occurrence
    /// order and leaves in settled order.
    #[cfg(test)]
    pub(in crate::app) fn reconcile(&mut self, entries: &[MusicTreeEntry]) {
        self.reconcile_with_tracks(entries, &HashMap::new());
    }

    /// Reconciles the settled album projection plus the already cached track
    /// children. The map is keyed by the stable album target, so duplicate
    /// album IDs remain distinct tree branches.
    pub(in crate::app) fn reconcile_with_tracks(
        &mut self,
        entries: &[MusicTreeEntry],
        tracks_by_album: &HashMap<String, Vec<MusicTreeTrack>>,
    ) {
        let mut next_roots = Vec::new();
        let mut next_children: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut display_changed = false;
        let mut root_of_key: HashMap<ArtistKey, usize> = HashMap::new();

        for entry in entries {
            let root = match root_of_key.get(&entry.artist_key) {
                Some(&root) => root,
                None => {
                    let root =
                        self.intern_artist(&entry.artist_key, &entry.artist, &mut display_changed);
                    root_of_key.insert(entry.artist_key.clone(), root);
                    next_roots.push(root);
                    root
                }
            };
            let leaf = self.intern_album(
                &entry.artist,
                &entry.target,
                &entry.title,
                entry.year.as_deref(),
                &entry.semantic_state,
                &mut display_changed,
            );
            next_children.entry(root).or_default().push(leaf);
            let track_ids = tracks_by_album
                .get(&entry.target)
                .into_iter()
                .flatten()
                .map(|track| self.intern_track(&entry.target, track, &mut display_changed))
                .collect();
            next_children.insert(leaf, track_ids);
        }

        let mut children = vec![Vec::new(); self.nodes.len()];
        for (root, leaves) in next_children {
            children[root] = leaves;
        }
        let mut root_position = vec![usize::MAX; self.nodes.len()];
        for (position, &root) in next_roots.iter().enumerate() {
            root_position[root] = position;
            for &leaf in &children[root] {
                root_position[leaf] = position;
                for &track in &children[leaf] {
                    root_position[track] = position;
                }
            }
        }

        let changed = display_changed || self.roots != next_roots || self.children != children;
        self.roots = next_roots;
        self.children = children;
        self.root_position = root_position;
        if changed {
            self.revision.advance();
        }
    }

    /// Clears the arena for a new destination identity (design D2): the
    /// intern space restarts, so no stale mapping survives a destination
    /// change.
    #[cfg(test)]
    pub(in crate::app) fn reset(&mut self) {
        self.nodes.clear();
        self.targets.clear();
        self.intern.clear();
        self.roots.clear();
        self.children.clear();
        self.root_position.clear();
        self.revision = TreeRevision::INITIAL;
    }

    /// Interns (or refreshes the display name of) one artist root.
    fn intern_artist(&mut self, key: &ArtistKey, name: &str, display_changed: &mut bool) -> usize {
        let node_key = MusicNodeKey::Artist(key.clone());
        if let Some(id) = self.intern.get(&node_key).copied() {
            if let Some(MusicNode::Artist { name: existing, .. }) = self.nodes.get_mut(id) {
                if existing != name {
                    *existing = name.to_string();
                    *display_changed = true;
                }
            }
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(MusicNode::Artist {
            key: key.clone(),
            name: name.to_string(),
        });
        self.targets.push(MusicTreeTarget::Artist(key.clone()));
        self.intern.insert(node_key, id);
        id
    }

    /// Interns (or refreshes the display data of) one album leaf.
    fn intern_track(
        &mut self,
        album_target: &str,
        track: &MusicTreeTrack,
        display_changed: &mut bool,
    ) -> usize {
        let node_key = MusicNodeKey::Track {
            album: album_target.to_string(),
            track: track.target.clone(),
        };
        if let Some(id) = self.intern.get(&node_key).copied() {
            if let Some(MusicNode::Track {
                title,
                search_title,
                ..
            }) = self.nodes.get_mut(id)
            {
                if title != &track.title || search_title != &track.search_title {
                    *title = track.title.clone();
                    *search_title = track.search_title.clone();
                    *display_changed = true;
                }
            }
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(MusicNode::Track {
            album_target: album_target.to_string(),
            target: track.target.clone(),
            title: track.title.clone(),
            search_title: track.search_title.clone(),
        });
        self.targets.push(MusicTreeTarget::Track {
            album: album_target.to_string(),
            track: track.target.clone(),
        });
        self.intern.insert(node_key, id);
        id
    }

    fn intern_album(
        &mut self,
        artist: &str,
        target: &str,
        title: &str,
        year: Option<&str>,
        semantic_state: &MediaSemanticState,
        display_changed: &mut bool,
    ) -> usize {
        // Music rows never track played/unplayed. Normalize at the tree-model
        // boundary as a second line of defence for test/builders that provide
        // a raw `Played` state; playback-live states remain unchanged.
        let semantic_state = match semantic_state {
            MediaSemanticState::Played => MediaSemanticState::Ordinary,
            state => state.clone(),
        };
        let node_key = MusicNodeKey::Album(target.to_string());
        if let Some(id) = self.intern.get(&node_key).copied() {
            if let Some(MusicNode::Album {
                artist: existing_artist,
                title: existing_title,
                year: existing_year,
                semantic_state: existing_state,
                ..
            }) = self.nodes.get_mut(id)
            {
                if existing_artist != artist
                    || existing_title != title
                    || existing_year.as_deref() != year
                    || existing_state != &semantic_state
                {
                    *existing_artist = artist.to_string();
                    *existing_title = title.to_string();
                    *existing_year = year.map(str::to_string);
                    *existing_state = semantic_state;
                    *display_changed = true;
                }
            }
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(MusicNode::Album {
            artist: artist.to_string(),
            title: title.to_string(),
            year: year.map(str::to_string),
            target: target.to_string(),
            semantic_state: semantic_state.clone(),
        });
        self.targets
            .push(MusicTreeTarget::Album(target.to_string()));
        self.intern.insert(node_key, id);
        id
    }

    /// The interned arena id for a stable target, when the arena holds it.
    /// This is the target→node half of the boundary map; the arena index it
    /// returns never leaves this module.
    fn id_of(&self, target: &MusicTreeTarget) -> Option<usize> {
        self.intern.get(&MusicNodeKey::from(target)).copied()
    }

    /// The stable target of an interned node id. This is the node→target half
    /// of the boundary map; an unknown (tombstoned or out-of-range) id has no
    /// target.
    fn target_of_node(&self, id: usize) -> Option<MusicTreeTarget> {
        self.target_ref_of_node(id).cloned()
    }

    /// Borrow the stable target of an interned node id for the shared seam.
    fn target_ref_of_node(&self, id: usize) -> Option<&MusicTreeTarget> {
        self.targets.get(id)
    }

    /// Whether the node is an artist root.
    fn is_artist(&self, id: usize) -> bool {
        matches!(self.nodes.get(id), Some(MusicNode::Artist { .. }))
    }

    /// The artist root's settled identity (node-to-domain translation, D2);
    /// album leaves have none.
    fn artist_key_of(&self, id: usize) -> Option<&ArtistKey> {
        match self.nodes.get(id) {
            Some(MusicNode::Artist { key, .. }) => Some(key),
            _ => None,
        }
    }

    /// The artist root's settled display name; album leaves have none.
    fn artist_name_of(&self, id: usize) -> Option<&str> {
        match self.nodes.get(id) {
            Some(MusicNode::Artist { name, .. }) => Some(name),
            _ => None,
        }
    }

    fn title_of(&self, id: usize) -> &str {
        match &self.nodes[id] {
            MusicNode::Artist { name, .. } => name,
            MusicNode::Album { title, .. } | MusicNode::Track { title, .. } => title,
        }
    }

    fn year_of(&self, id: usize) -> Option<&str> {
        match &self.nodes[id] {
            MusicNode::Album { year, .. } => year.as_deref().filter(|year| !year.is_empty()),
            MusicNode::Artist { .. } | MusicNode::Track { .. } => None,
        }
    }

    fn target_of(&self, id: usize) -> Option<&str> {
        match self.nodes.get(id) {
            Some(MusicNode::Album { target, .. }) => Some(target),
            _ => None,
        }
    }

    /// Resolves an album identity for an album or one of its track children.
    fn album_target_of(&self, id: usize) -> Option<&str> {
        match self.nodes.get(id) {
            Some(MusicNode::Album { target, .. }) => Some(target),
            Some(MusicNode::Track { album_target, .. }) => Some(album_target),
            _ => None,
        }
    }

    /// Resolves the stable `(album target, track target)` identity of a track
    /// node. Track rows never use projection indexes as identities.
    fn track_identity_of(&self, id: usize) -> Option<(&str, &str)> {
        match self.nodes.get(id) {
            Some(MusicNode::Track {
                album_target,
                target,
                ..
            }) => Some((album_target, target)),
            _ => None,
        }
    }

    /// The node's own searchable text for the inline filter. Every level
    /// matches on its own identity alone — an artist root on its name, an
    /// album leaf on its title and year, a cached track on its searchable
    /// title — so a match never drags a sibling or a deeper row in: an album
    /// no longer
    /// matches through its artist's name, and a track no longer matches
    /// through its album's.
    fn search_text_of(&self, id: usize) -> Option<String> {
        match self.nodes.get(id) {
            Some(MusicNode::Artist { name, .. }) => Some(name.clone()),
            Some(MusicNode::Album { title, year, .. }) => {
                Some(format!("{} {}", title, year.as_deref().unwrap_or_default()))
            }
            Some(MusicNode::Track { search_title, .. }) => Some(search_title.clone()),
            None => None,
        }
    }

    /// The album leaf's playback-live semantic state; artist roots are
    /// ordinary grouping rows and carry none, and a cached track reports its
    /// album's through the label renderer's own parent lookup. Played/unplayed
    /// is normalized away before a state reaches the arena.
    fn semantic_state_of(&self, id: usize) -> Option<&MediaSemanticState> {
        match self.nodes.get(id) {
            Some(MusicNode::Album { semantic_state, .. }) => Some(semantic_state),
            _ => None,
        }
    }

    /// The node's top-level group zebra phase. The header and every visible
    /// descendant deliberately share one band, so expanding a group does not
    /// introduce row-based colour changes.
    fn is_striped(&self, id: usize) -> bool {
        self.root_position
            .get(id)
            .is_some_and(|position| *position != usize::MAX && position.is_multiple_of(2))
    }
}

impl TreeModel for MusicTreeModel {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = usize> + '_ {
        self.roots.iter().copied()
    }

    fn children(&self, id: usize) -> TreeChildren<'_, usize> {
        match &self.nodes[id] {
            MusicNode::Artist { .. } => TreeChildren::Loaded(&self.children[id]),
            MusicNode::Album { .. } if self.children[id].is_empty() => TreeChildren::Leaf,
            MusicNode::Album { .. } => TreeChildren::Loaded(&self.children[id]),
            MusicNode::Track { .. } => TreeChildren::Leaf,
        }
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn size_hint(&self) -> usize {
        self.nodes.len()
    }
}

/// The destination-local matching projection used by the tree owner. The
/// eventual fuzzy filter (task 5.1) supplies matching album/root ids through
/// this same seam; expansion never participates in the action scope.
#[derive(Clone, Debug, Default)]
struct MusicTreeFilter {
    matching: HashSet<usize>,
}

impl TreeFilter<MusicTreeModel> for MusicTreeFilter {
    fn is_match(&self, model: &MusicTreeModel, id: usize) -> bool {
        let _ = model;
        // A filtered tree shows each match's own row and the ancestors needed
        // to reach it, and nothing below the match's level: a matched album no
        // longer drags its whole track list into the projection, so a track row
        // appears only while the query is off.
        self.matching.contains(&id)
    }
}

/// Width of the plain-space indentation + separator prefix
/// `tree_label_line` paints before a row's name. Roots have no prefix; nested
/// rows have one three-column space span per level and one separator before
/// the title. The state glyph is intentionally empty.
fn glyph_prefix_width(level: usize) -> usize {
    if level == 0 {
        0
    } else {
        // The renderer trims two columns from album prefixes and four from
        // track prefixes; keep the title budget in step with that composition.
        let trimmed = if level == 2 { 4 } else { 2 };
        3 * level + 1 - trimmed
    }
}
