// Included into `music_content` via `include!` (the module's doc and
// imports live there, beside the split's other parts).

pub(in crate::app) fn track_row_label(track: &EmbyItem, index: usize) -> String {
    let number = if track.index_number > 0 {
        track.index_number
    } else {
        index as i64 + 1
    };
    format!("{number}. {}", track.name)
}

fn build_track_rows(tracks: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    tracks
        .iter()
        .enumerate()
        .map(|(index, track)| track_row(track, index))
        .collect()
}

fn track_row(track: &EmbyItem, index: usize) -> MediaListRow<String> {
    let trailing = (track.runtime_ticks > 0)
        .then(|| fmt_duration_gutter(track.runtime_ticks / TICKS_PER_SECOND))
        .map(MediaListTrailing::Gutter);
    MediaListRow::Item {
        target: track.id.clone(),
        primary: track_row_label(track, index),
        secondary: None,
        trailing,
        duration: None,
        kind: MediaKind::Media,
        // The one canonical state derivation.
        semantic_state: MediaSemanticState::from_emby(track),
    }
}

/// The focused artist's Workspace rows (task 6.3): one canonical `Heading`
/// per settled album in settled order, then that album's tracks in disc/track
/// order. Duplicate titles stay distinct because rows are keyed by the
/// tracks' own stable IDs.
fn build_artist_track_rows(
    detail: &crate::app::music_artist_detail::ArtistDetailProjection,
) -> Vec<MediaListRow<String>> {
    let mut rows = Vec::new();
    for group in &detail.track_groups {
        rows.push(MediaListRow::Heading {
            text: group.album_title.clone(),
        });
        rows.extend(
            group
                .tracks
                .iter()
                .enumerate()
                .map(|(index, track)| track_row(track, index)),
        );
    }
    rows
}

/// The snapshot identity that produced the track Workspace's current rows
/// (task 6.4): one album leaf's album snapshot or one artist root's settled
/// detail projection. The Hero facts and the Workspace rows resolve through
/// the same selection, so a stale snapshot can never paint under a new title.
#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkspaceOwner {
    Album(String),
    Artist(MusicArtistTarget),
}

/// The focused artist root's Hero content (task 6.4, design D7): settled
/// summary facts plus the selected album's projected artwork state. Artist
/// artwork is deliberately not part of this producer: the first settled
/// album supplies the default image, and a selected artist track supplies its
/// own album image through the same `music_album_artwork` source used by the
/// album Hero and neighbour prefetch.
fn artist_hero_data(
    detail: &crate::app::music_artist_detail::ArtistDetailProjection,
    mut artwork: HeroArtwork,
    image: HeroImageState,
) -> HeroContentData {
    let summary = &detail.summary;
    let mut meta_rows = vec![format!(
        "{} album{}",
        summary.album_count,
        if summary.album_count == 1 { "" } else { "s" }
    )];
    if let Some(span) = summary.year_span() {
        meta_rows.push(span);
    }
    artwork.image = image;
    HeroContentData {
        facts: HeroFacts {
            title: summary.name.clone(),
            meta_rows,
            duration_row: None,
            progress_row: None,
            links: Vec::new(),
            artwork,
        },
        overview: None,
        credits: None,
    }
}

impl MusicContent {
    /// The Workspace rows the current tree selection owns, from that
    /// selection's own snapshot only (task 6.4): an artist root reads its
    /// matching projected detail groups, an album leaf reads the pushed album
    /// snapshot when it is that leaf's. Between pushes a local move resolves
    /// to empty rows, so the prior title's tracks never paint under the new
    /// one.
    fn resolved_workspace(&self) -> (Option<WorkspaceOwner>, Vec<MediaListRow<String>>) {
        if self.selected_is_artist() {
            let owner = self.artist_detail_target().map(WorkspaceOwner::Artist);
            let rows = self
                .current_artist_detail()
                .map(build_artist_track_rows)
                .unwrap_or_default();
            return (owner, rows);
        }
        let Some(selected) = self.selected_item() else {
            return (None, Vec::new());
        };
        let tracks: &[EmbyItem] = match self.context.selected_album.as_ref() {
            Some(pushed) if pushed.id == selected.id => {
                self.context.album_tracks.as_deref().unwrap_or_default()
            }
            _ => &[],
        };
        (
            Some(WorkspaceOwner::Album(selected.id)),
            build_track_rows(tracks),
        )
    }

    /// The projected artist detail when it belongs to the tree's current
    /// artist root. The shell binds the projection to the owner-resolved
    /// target at push time; a local move onto a different root between pushes
    /// must not paint the prior root's summary, artwork, or groups.
    fn current_artist_detail(
        &self,
    ) -> Option<&crate::app::music_artist_detail::ArtistDetailProjection> {
        self.context.artist_detail.as_ref().filter(|detail| {
            self.artist_detail_target()
                .is_some_and(|target| target.same_source(&detail.target))
        })
    }

    /// Expose the current shell-projected artist detail to the shell's
    /// playback resolver without carrying queue data across the component
    /// message boundary.
    pub(in crate::app) fn artist_detail_for_target(
        &self,
        target: &MusicArtistTarget,
    ) -> Option<&crate::app::music_artist_detail::ArtistDetailProjection> {
        self.current_artist_detail()
            .filter(|detail| detail.target.same_source(target))
    }

    /// Rebuild `track_list` from the resolved Workspace and report whether it
    /// went empty-to-non-empty (the overlay/artist-entry arrival edge). A
    /// changed owner clears any stale pane focus and re-seats the cursor; a
    /// pending Wide artist-Workspace entry takes the focus once the resolved
    /// root's rows exist.
    fn reconcile_workspace_rows(&mut self) -> bool {
        let (owner, rows) = self.resolved_workspace();
        let owner_changed = self.track_rows_owner != owner;
        if owner_changed {
            self.track_focused = false;
        }
        let arrived = self.track_list.rows().is_empty() && !rows.is_empty();
        if self.track_list.rows() != rows.as_slice() {
            self.track_list.set_content(rows);
        }
        if owner_changed {
            self.track_list.select_first();
        }
        if let Some(pending) = self.pending_artist_workspace_focus.clone() {
            let owner_matches = matches!(&owner, Some(WorkspaceOwner::Artist(resolved)) if resolved.same_source(&pending));
            if owner_matches {
                // The armed root's own rows landed: take the cursor once.
                if !self.track_list.rows().is_empty() {
                    self.pending_artist_workspace_focus = None;
                    self.enter_track_focus();
                }
            } else if owner.is_some() {
                // The resolved Workspace belongs to another root or to an
                // album leaf: the selection left the armed root, so the
                // entry is void and no later push may take the cursor.
                self.pending_artist_workspace_focus = None;
            }
        }
        self.track_rows_owner = owner;
        arrived
    }

    /// Voids the armed Wide artist-Workspace entry when the tree selection no
    /// longer resolves to the root that was armed (task 6.4): the entry may
    /// only take the cursor for the root the user pressed Right on.
    fn void_artist_workspace_focus_off_root(&mut self) {
        let still_on_root = self
            .pending_artist_workspace_focus
            .as_ref()
            .is_some_and(|pending| {
                self.artist_detail_target()
                    .is_some_and(|target| target.same_source(pending))
            });
        if self.pending_artist_workspace_focus.is_some() && !still_on_root {
            self.pending_artist_workspace_focus = None;
        }
    }

    /// Refreshes the component's projected view of the shell-owned artist
    /// cache. It is retained across a local move onto an album child because
    /// the shell's next album snapshot does not itself carry the artist cache.
    fn project_tree_tracks(&mut self) {
        if self.tree_track_revision != Some(self.context.catalog_revision) {
            self.tree_tracks.clear();
            self.tree_track_revision = Some(self.context.catalog_revision);
        }
        if let Some(detail) = self.current_artist_detail() {
            let groups = detail.track_groups.clone();
            for group in groups {
                for (index, album) in self.context.list.items.iter().enumerate() {
                    if album.id == group.album_id {
                        if let Some(target) = self.context.album_targets.get(index) {
                            self.tree_tracks
                                .insert(target.clone(), group.tracks.clone());
                        }
                    }
                }
            }
            return;
        }

        // A local move onto an album may replace the artist projection with
        // the ordinary album snapshot. If its existing album-track cache is
        // now present, fold that same cache into the tree branch; this is the
        // existing fetch path, not a tree request.
        let (Some(album), Some(tracks)) = (
            self.context.selected_album.as_ref(),
            self.context.album_tracks.as_ref(),
        ) else {
            return;
        };
        if let Some(index) = self
            .context
            .list
            .items
            .iter()
            .position(|item| item.id == album.id)
        {
            if let Some(target) = self.context.album_targets.get(index) {
                self.tree_tracks.insert(target.clone(), tracks.clone());
            }
        }
    }

    pub(in crate::app) fn selected_item(&self) -> Option<EmbyItem> {
        let target = self.selected_album_target()?;
        let index = self
            .context
            .album_targets
            .iter()
            .position(|candidate| candidate == &target)?;
        self.context.list.items.get(index).cloned()
    }

    /// Resolves a set of settled album targets against the latest content
    /// snapshot. A sync race may leave one target without an item; keep the
    /// ordered items that still resolve and return the misses separately so
    /// the shell can surface feedback instead of silently dropping the action.
    fn resolve_album_targets(&self, targets: Vec<String>) -> (Vec<EmbyItem>, Vec<String>) {
        let mut items = Vec::with_capacity(targets.len());
        let mut unresolved_targets = Vec::new();
        for target in targets {
            let Some(index) = self
                .context
                .album_targets
                .iter()
                .position(|candidate| candidate == &target)
            else {
                unresolved_targets.push(target);
                continue;
            };
            let Some(item) = self.context.list.items.get(index) else {
                unresolved_targets.push(target);
                continue;
            };
            items.push(item.clone());
        }
        (items, unresolved_targets)
    }

    /// Resolves the focused artist's settled album targets against the latest
    /// content snapshot.
    fn selected_artist_items(&self) -> Option<(Vec<EmbyItem>, Vec<String>)> {
        let targets = self.selected_artist_album_targets()?;
        Some(self.resolve_album_targets(targets))
    }

    /// Resolves marked tree album leaves in settled display order. This is the
    /// multi-selection boundary: the tree owns membership and the content
    /// owner only translates stable album targets to the current snapshot.
    fn selected_tree_items(&self) -> Option<(Vec<EmbyItem>, Vec<String>)> {
        let targets = self.selected_album_targets_in_display_order();
        if targets.is_empty() {
            return None;
        }
        Some(self.resolve_album_targets(targets))
    }

    fn artist_action(&self, action: MusicTreeAction) -> Option<Msg> {
        let origin = self.selection_origin.clone()?;
        let (items, unresolved_targets) = self
            .selected_tree_items()
            .or_else(|| self.selected_artist_items())?;
        if items.is_empty() && unresolved_targets.is_empty() {
            return None;
        }
        Some(Msg::Shell(ShellRequest::MusicArtistAction {
            action,
            items,
            origin,
            unresolved_targets,
        }))
    }

    /// The focused artist root's component-resolved detail identity (design
    /// D7): settled identity, display name, leaf album targets, and the
    /// snapshot's settled revision. `None` while an album leaf is focused.
    pub(in crate::app) fn artist_detail_target(&self) -> Option<MusicArtistTarget> {
        let selected = self.browser.selected_target()?;
        let MusicTreeTarget::Artist(key) = selected else {
            return None;
        };
        let artist_name = self.browser.node(selected)?.title.clone();
        Some(MusicArtistTarget {
            artist_id: match key {
                crate::app::music_grouping::ArtistKey::Service(id) => Some(id.clone()),
                crate::app::music_grouping::ArtistKey::Fallback(_) => None,
            },
            artist_name,
            album_targets: self.selected_artist_album_targets()?,
            revision: self.context.catalog_revision,
        })
    }

    /// The Workspace row projection that owns a track target: the focused
    /// artist root's matching projected groups first (an artist root's push
    /// clears `selected_album`/`album_tracks`, so its rows have no album
    /// snapshot), otherwise the selected album's cached tracks. Every row
    /// behaviour resolves through this one owner, so an artist track row and
    /// an album track row share the same paths.
    fn workspace_track_item(&self, target: &str) -> Option<EmbyItem> {
        if let Some(detail) = self.current_artist_detail() {
            return detail
                .track_groups
                .iter()
                .flat_map(|group| group.tracks.iter())
                .find(|track| track.id == target)
                .cloned();
        }
        self.context
            .album_tracks
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|track| track.id == target)
            .cloned()
    }

    /// The album identity that owns the focused Workspace track: the selected
    /// album leaf's own ID, or the projected group the artist root's track
    /// came from (the projection already carries each group's settled
    /// `album_id`).
    fn focused_track_album_id(&self) -> Option<String> {
        let target = self.track_list.selected_target()?;
        if let Some(detail) = self.current_artist_detail() {
            return detail
                .track_groups
                .iter()
                .find(|group| group.tracks.iter().any(|track| track.id == *target))
                .map(|group| group.album_id.clone());
        }
        self.selected_item().map(|album| album.id)
    }

    /// Whether the focused track pane holds the selected artist root's own
    /// Workspace (task 6.3 projects its rows from `artist_detail`). A pane
    /// left over from an album while the tree selection has already moved
    /// onto a root is not that Workspace: there Enter keeps toggling the root
    /// (task 2.4) instead of resolving a track from a snapshot that no longer
    /// addresses the focused node.
    fn artist_workspace_focused(&self) -> bool {
        self.track_focused
            && self.selected_is_artist()
            && self.current_artist_detail().is_some()
    }

    pub(in crate::app) fn selected_track_item(&self) -> Option<EmbyItem> {
        let target = self.track_list.selected_target()?;
        self.workspace_track_item(target)
    }

    /// The neighbour album-artwork targets the shell prefetches (design D4):
    /// from the latest completed paint's visible flow, up to one album leaf
    /// behind the selected leaf and up to three ahead, in visible order,
    /// skipping artist roots and the selected leaf itself. The owner resolves
    /// the window here so the shell receives stable targets and never a
    /// cursor or enough tree state to re-resolve one.
    ///
    /// `None` when no paint completed (retained geometry is not the painted
    /// projection), when an artist root is focused (the shipped suppression),
    /// or when the window has no album leaf.
    fn neighbour_prefetch_targets(&self) -> Option<Vec<String>> {
        if !self.neighbour_prefetch_eligible() {
            return None;
        }
        let selected = self.browser.selected_target()?.clone();
        let visible = self.browser.visible_targets();
        let position = visible.iter().position(|target| *target == selected)?;
        let album_of = |target: &MusicTreeTarget| target.album_leaf_target().map(str::to_string);
        let mut targets: Vec<String> = visible[..position]
            .iter()
            .rev()
            .filter_map(album_of)
            .take(NEIGHBOUR_PREFETCH_BEHIND)
            .collect();
        targets.extend(
            visible[position + 1..]
                .iter()
                .filter_map(album_of)
                .take(NEIGHBOUR_PREFETCH_AHEAD),
        );
        (!targets.is_empty()).then_some(targets)
    }

    /// The shipped neighbour-prefetch suppression: no window without a
    /// completed paint (retained geometry is not the painted projection),
    /// with an artist root focused, or while a filter session is active.
    fn neighbour_prefetch_eligible(&self) -> bool {
        self.browser.has_completed_paint()
            && !self.selected_is_artist()
            && !self.browser.filter_active()
    }

    /// Resolves a focused Workspace track as an artist-track activation when
    /// the pane belongs to an artist root, otherwise as the ordinary album
    /// track activation. All Workspace gestures use this one fallback so the
    /// stable artist identity gate and album-item resolution cannot drift.
    fn workspace_track_activation(&self) -> Option<Msg> {
        if self.artist_workspace_focused() {
            if let (Some(target), Some(track_id)) = (
                self.artist_detail_target(),
                self.track_list.selected_target().cloned(),
            ) {
                return Some(Msg::Shell(ShellRequest::MusicArtistTrackActivate {
                    target,
                    track_id,
                }));
            }
        }
        let track = self.selected_track_item()?;
        let album_id = self.focused_track_album_id()?;
        Some(Msg::Shell(ShellRequest::MusicTrackActivate { album_id, track }))
    }

    /// Select the album whose existing artwork path supplies an artist Hero.
    /// The tree's selected track wins; otherwise the first settled artist
    /// group is the stable default. The album item lookup preserves the exact
    /// `music_album_artwork` source/cache convention used by album Heroes and
    /// neighbour prefetch.
    fn artist_hero_artwork(
        &self,
        detail: &crate::app::music_artist_detail::ArtistDetailProjection,
    ) -> HeroArtwork {
        let album_id = self
            .selected_tree_track()
            .map(|(album_id, _)| album_id)
            .or_else(|| {
                self.artist_workspace_focused()
                    .then(|| self.focused_track_album_id())
                    .flatten()
            })
            .or_else(|| {
                detail
                    .track_groups
                    .first()
                    .map(|group| group.album_id.clone())
            })
            .or_else(|| {
                detail
                    .target
                    .album_targets
                    .first()
                    .map(|target| target.split('\0').next().unwrap_or(target).to_string())
            });

        album_id
            .and_then(|album_id| {
                self.context
                    .list
                    .items
                    .iter()
                    .find(|item| item.id == album_id)
            })
            .map(music_album_artwork)
            .unwrap_or(HeroArtwork {
                shape: ArtworkShape::Square,
                source: None,
                decoration: None,
                image: HeroImageState::None,
            })
    }

    /// The Hero pane's content for the tree's current selection, or `None`
    /// when nothing hero-bearing resolves. Both arms read the one snapshot
    /// the current selection owns (task 6.4), so the Hero title and its
    /// Workspace switch atomically in Wide and the Library Hero overlay: an
    /// artist root's facts come from its matching detail projection, an album
    /// leaf's from the album arm, and an artist root without a matching
    /// projection has no honest Hero yet (the push that follows its focus
    /// supplies one).
    fn resolved_hero_data(&self) -> Option<HeroContentData> {
        if let Some(detail) = self.current_artist_detail() {
            let artwork = self.artist_hero_artwork(detail);
            return Some(artist_hero_data(detail, artwork, self.hero_image.clone()));
        }
        if self.selected_is_artist() {
            return None;
        }
        let album = self.selected_item()?;
        // Grouped Music's rows are Emby `Folder` items, so the type-based
        // `emby_artwork_policy` cannot recognise them as albums: the Music
        // owner asks for the album arm directly.
        let mut data = hero_content_music_album(&album);
        if let Some((artist, _, _)) = self.context.album_info.get(self.selected_album_index()) {
            let (title, year) = wide_album_metadata(&album, artist);
            data.facts.title = title;
            data.facts.meta_rows.clear();
            if !artist.is_empty() && artist != "Unknown Artist" {
                data.facts.meta_rows.push(artist.clone());
            }
            if year > 0 {
                data.facts.meta_rows.push(year.to_string());
            }
        }
        data.facts.artwork.image = self.hero_image.clone();
        Some(data)
    }

    pub(in crate::app) fn panel_content(&mut self) -> LibraryPanelContent<'_> {
        // The Workspace and the Hero must describe the same tree selection:
        // reconcile the rows before building either, so a local move between
        // pushes cannot paint the prior title's tracks (task 6.4).
        self.reconcile_workspace_rows();
        // Copy the selected snapshot and focus bit before borrowing either
        // list mutably for the returned slots.
        let focused = self.context.focused;
        let track_focused = self.track_focused;
        let hero = self.resolved_hero_data().map(|data| {
            HeroContent {
                facts: data.facts,
                // Music has no separate overview box when the album does not
                // provide an overview; the panel's generic producer already
                // represents that as `None`.
                overview: data.overview,
                credits: data.credits,
                workspace: Some(Workspace {
                    header: Some(WorkspaceHeader::Tracklist),
                    selector: None,
                    list: &mut self.track_list,
                    focused: focused && track_focused,
                }),
            }
        });
        let mut pills = vec!["Latest".to_string()];
        pills.extend(
            self.context
                .groups
                .iter()
                .map(|group| trunc_str(&group.name, 12).to_string()),
        );
        let mut markers = vec![self.latest_has_new_content];
        markers.resize(pills.len(), false);
        let selector = Some(SelectorRow {
            pills,
            markers,
            active: Some(if self.latest_mode || self.context.groups.is_empty() {
                0
            } else {
                self.context.group_cursor + 1
            }),
        });
        // The tree and flat Latest flow remain embedded in this same owner;
        // the Library Panel paints exactly one in the canonical list slot.
        let list = if self.latest_mode {
            ListSlot::Media(&mut self.latest_list)
        } else {
            ListSlot::Media(&mut self.browser)
        };
        LibraryPanelContent {
            selector,
            list,
            hero: (!self.latest_mode).then_some(hero).flatten(),
        }
    }
}
