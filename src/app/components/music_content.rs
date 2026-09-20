//! Grouped Music's embedded Library panel content owner (task 9.1).
//!
//! `MusicContent` owns the shell-projected album/track snapshot, the shared
//! album and track list controls, local track focus, and the Inline Search
//! session.  It is a plain [`LibraryContentOwner`]; the legacy
//! `MusicWorkspaceComponent` temporarily borrows this owner for its existing
//! painters until the later Music panel slices move painting and registration.

use mbv_core::api::EmbyItem;
use std::collections::HashMap;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::inline_search::{InlineSearch, InlineSearchHost};
use super::library_panel::content::{
    ArtworkShape, HeroArtwork, HeroContent, HeroFacts, HeroImageState, LibraryPanelContent,
    ListSlot, SelectorRow, Workspace,
};
use super::library_panel::hero::{hero_content_music_album, music_album_artwork};
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaSemanticState, RowIntent, SelectionOrigin,
};
use super::msg::{AlbumCursorKind, Msg, MusicArtistTarget, MusicTreeAction, ShellRequest};
use super::msg::{LeafKeyResult, TerminalObserverEvent};
use super::music_tree::{MusicTreeBrowser, MusicTreeEntry, MusicTreeModel, MusicTreeTrack};
use crate::app::render::MusicWideRenderCtx;
use crate::app::ui_util::trunc_str;

/// Strips the `Artist (Year) ` folder-name prefix from an album's display
/// name, returning the bare title and resolved release year. Rehomed from the
/// deleted `wide_album_metadata` (task 9.2) so the Wide hero keeps the
/// established artist/year/title presentation through the shared panel
/// without a second painter. `artist` is the resolved display artist the
/// content projection already carries (`group_album_info`'s
/// `album_artist_cache` fallback chain).
pub(in crate::app) fn wide_album_metadata(album: &EmbyItem, artist: &str) -> (String, u32) {
    let display_name = album.display_name();
    if let Some((parsed_artist, parsed_year, title)) =
        crate::app::render::parse_album_folder_name(&display_name)
    {
        let year_matches = album.production_year == 0 || album.production_year == parsed_year;
        if parsed_artist == artist && year_matches {
            return (title, album.production_year.max(parsed_year));
        }
    }

    let prefix = if album.production_year > 0 {
        format!("{artist} ({}) ", album.production_year)
    } else {
        format!("{artist} ")
    };
    let title = display_name
        .strip_prefix(&prefix)
        .unwrap_or(&display_name)
        .to_string();
    (title, album.production_year)
}

fn build_track_rows(tracks: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    tracks
        .iter()
        .enumerate()
        .map(|(index, track)| track_row(track, index))
        .collect()
}

fn track_row(track: &EmbyItem, index: usize) -> MediaListRow<String> {
    let number = if track.index_number > 0 {
        track.index_number
    } else {
        index as i64 + 1
    };
    // Library lists carry no time column (only the Queue list and the
    // sessions modal show one).
    MediaListRow::Item {
        target: track.id.clone(),
        primary: format!("{number}. {}", track.name),
        secondary: None,
        trailing: None,
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
            links: Vec::new(),
            artwork,
        },
        overview: None,
        credits: None,
    }
}

/// The plain Music content owner. Its tree browser owns the Grouped Music
/// album selection, expansion, viewport, and marks (one owner across Wide,
/// Narrow, and Mini); the track `MediaList` carrier retains its Workspace
/// cursor/scroll/selection locally. Shell pushes replace only the content
/// snapshot and never mirror those interaction values.
pub struct MusicContent {
    pub(in crate::app) context: MusicWideRenderCtx,
    /// The Grouped Music browser: the destination-local shallow tree (task
    /// 2.3). It is the only browser owner and painter — the parallel flat album
    /// carrier and its row projection are gone.
    pub(in crate::app) browser: MusicTreeBrowser,
    pub(in crate::app) track_list: MediaListCarrier<String>,
    pub(in crate::app) track_focused: bool,
    /// Whether this frame's geometry hosts the inline track list (the Wide
    /// pane). Pushed each sync pass beside the track-focus clear; narrow
    /// selects the Library Hero overlay instead.
    pub(in crate::app) inline_track_focus_enabled: bool,
    /// The snapshot identity that produced the Workspace rows currently in
    /// `track_list` (task 6.4). A different title never paints those rows.
    track_rows_owner: Option<WorkspaceOwner>,
    /// A Wide artist-Workspace entry armed before the artist's track rows
    /// arrived: Right on an expanded root takes the pane's focus as soon as
    /// the resolved root's rows land (the overlay path re-focuses on arrival
    /// through its own open-transition bit). The entry carries the root the
    /// user pressed Right on: it fires only for that root's own rows and dies
    /// when the tree selection leaves the root or the geometry stops hosting
    /// the inline pane, so a later push for another root can never take the
    /// cursor under a selection the user never entered.
    pending_artist_workspace_focus: Option<MusicArtistTarget>,
    /// The artist identity last carried to the shell on a typed
    /// artist-track request (design D7). Moving onto a different root emits;
    /// returning to the last reported one relies on the shell's projection
    /// push, which re-derives the same component-resolved target.
    last_artist_request: Option<crate::app::music_grouping::ArtistKey>,
    pub(in crate::app) inline_search: InlineSearch,
    /// Stable identity of the tree/list that produced a direct artist action.
    /// The Library panel supplies it on activation. It stays absent until that
    /// projection so an unprojected component cannot claim Queue as its origin;
    /// task 4.3 will consume the carried identity for status/bulk-action wiring.
    selection_origin: Option<SelectionOrigin>,
    hero_image: HeroImageState,
    /// Whether the Library Hero overlay is open over this owner (pushed by
    /// the panel). The overlay takes the Workspace's keyboard focus once, on
    /// its open transition; ordinary pushes never focus or re-focus the
    /// track pane, and an explicit shell focus clear always wins.
    hero_overlay_open: bool,
    /// Full cached track items projected for the tree. The browser receives
    /// only stable targets/labels; this retained shell projection resolves a
    /// selected tree track through the existing playback request arm even
    /// after the shell changes its selected-album snapshot.
    tree_tracks: HashMap<String, Vec<EmbyItem>>,
    tree_track_revision: Option<u64>,
}

impl MusicContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            context: MusicWideRenderCtx::new(
                crate::app::render::LibraryListRenderCtx::from_items(Vec::new(), 0),
                None,
                String::new(),
                Vec::new(),
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
            ),
            browser: MusicTreeBrowser::new(MusicTreeModel::new()),
            track_list: MediaListCarrier::new(),
            track_focused: false,
            inline_track_focus_enabled: false,
            track_rows_owner: None,
            pending_artist_workspace_focus: None,
            last_artist_request: None,
            inline_search: InlineSearch::new(),
            // There is no honest library identity before the panel's first
            // active-owner projection. Artist actions wait for that projection
            // instead of claiming the Queue origin by default.
            selection_origin: None,
            hero_image: HeroImageState::None,
            hero_overlay_open: false,
            tree_tracks: HashMap::new(),
            tree_track_revision: None,
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: MusicWideRenderCtx) {
        // Content projection never carries framework focus; preserve the
        // component-owned value across the shell snapshot swap.
        let focused = self.context.focused;
        self.context = context;
        self.context.focused = focused;

        self.project_tree_tracks();
        let tracks_by_album = self
            .tree_tracks
            .iter()
            .map(|(album, tracks)| {
                (
                    album.clone(),
                    tracks
                        .iter()
                        .map(|track| MusicTreeTrack {
                            target: track.id.clone(),
                            title: track.name.clone(),
                        })
                        .collect(),
                )
            })
            .collect();
        self.browser.set_track_items(tracks_by_album);
        let album_rows = self.tree_entries();
        self.browser.reconcile(&album_rows);
        // A fresh owner (or a destination whose tree has no selection yet)
        // adopts the shell's projected album position once. Later pushes never
        // re-point the tree — the tree owns selection, and an explicit shell
        // re-anchor request (`re_anchor`) adopts a navigated position.
        if !self.browser.has_selection() {
            let adopted = self
                .context
                .selected_album
                .as_ref()
                .and_then(|album| {
                    self.context
                        .list
                        .items
                        .iter()
                        .position(|item| item.id == album.id)
                })
                .and_then(|position| self.context.album_targets.get(position).cloned());
            if let Some(target) = adopted {
                self.browser.select_album_target(&target);
            }
        }
        // The Workspace rows and the Hero facts resolve from the same tree
        // selection (`resolved_workspace`/`resolved_hero_data`), so a
        // snapshot for another album or artist never paints under this title.
        let rows_arrived = self.reconcile_workspace_rows();
        // The overlay's Workspace can outrun the track fetch: the open
        // transition cannot take the focus while the rows are still empty, so
        // it is taken the moment they arrive. Only the empty-to-non-empty edge
        // fires, so an explicit Esc (or any later push) never re-seizes the
        // focus the user left behind.
        if rows_arrived && self.hero_overlay_open {
            self.enter_track_focus();
        }
    }

    /// The Workspace rows the current tree selection owns, from that
    /// selection's own snapshot only (task 6.4): an artist root reads its
    /// matching projected detail groups, an album leaf reads the pushed album
    /// snapshot when it is that leaf's. Between pushes a local move resolves
    /// to empty rows, so the prior title's tracks never paint under the new
    /// one.
    fn resolved_workspace(&self) -> (Option<WorkspaceOwner>, Vec<MediaListRow<String>>) {
        if self.browser.selected_is_artist() {
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

    /// Resolves a selected tree track to the full cached item used by the
    /// existing `MusicTrackActivate` arm. The tree contributes only stable
    /// album/track identity; playback resolution remains in the shell.
    fn selected_tree_track(&self) -> Option<(String, EmbyItem)> {
        let (album_target, track_target) = self.browser.selected_track_identity()?;
        let track = self
            .tree_tracks
            .get(album_target)?
            .iter()
            .find(|track| track.id == track_target)?
            .clone();
        let album_id = if track.album_id.is_empty() {
            album_target
                .split('\0')
                .next()
                .unwrap_or(album_target)
                .to_string()
        } else {
            track.album_id.clone()
        };
        Some((album_id, track))
    }

    /// The settled album projection the tree owner reconciles: one entry per
    /// album in settled order, carrying the stable album target, the settled
    /// display text, the album's stable artist identity (design D2/D3), and
    /// playback-live state. Stored played/unplayed facts are deliberately
    /// ignored for music rows; a positive position still retains `Active`.
    fn tree_entries(&self) -> Vec<MusicTreeEntry> {
        self.context
            .album_order
            .iter()
            .filter_map(|&index| {
                let (artist, year, name) = self.context.album_info.get(index)?;
                let artist_key = self.context.album_artist_keys.get(index)?.clone();
                let target = self.context.album_targets.get(index)?;
                // The tree's album projection is music by owner context even
                // when Emby represents its rows as `Folder`. Ignore stored
                // played state while retaining a positive playback position
                // as the live `Active` distinction.
                let semantic_state = self
                    .context
                    .list
                    .items
                    .get(index)
                    .map(|item| {
                        MediaSemanticState::from_progress(
                            false,
                            item.playback_position_ticks,
                            item.runtime_ticks,
                        )
                    })
                    .unwrap_or(MediaSemanticState::Ordinary);
                Some(MusicTreeEntry {
                    artist: artist.clone(),
                    artist_key,
                    title: name.clone(),
                    year: (!year.is_empty()).then(|| year.clone()),
                    target: target.clone(),
                    semantic_state,
                })
            })
            .collect()
    }

    pub(in crate::app) fn selected_item(&self) -> Option<EmbyItem> {
        let target = self.browser.selected_album_target()?;
        let index = self
            .context
            .album_targets
            .iter()
            .position(|candidate| candidate == target)?;
        self.context.list.items.get(index).cloned()
    }

    /// Resolves the focused artist's settled album targets against the latest
    /// content snapshot. A sync race may leave one target without an item; keep
    /// the ordered items that still resolve and return the misses separately so
    /// the shell can surface feedback instead of silently dropping the action.
    fn selected_artist_items(&self) -> Option<(Vec<EmbyItem>, Vec<String>)> {
        let targets = self.browser.selected_artist_album_targets()?;
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
        Some((items, unresolved_targets))
    }

    /// Resolves marked tree album leaves in settled display order. This is the
    /// multi-selection boundary: the tree owns membership and the content
    /// owner only translates stable album targets to the current snapshot.
    fn selected_tree_items(&self) -> Option<(Vec<EmbyItem>, Vec<String>)> {
        let targets = self.browser.selected_album_targets_in_display_order();
        if targets.is_empty() {
            return None;
        }
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
        Some((items, unresolved_targets))
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

    /// Whether the tree's focused node is an artist root (task 2.2: an artist
    /// focus resolves to no album and never writes album persistence).
    pub(in crate::app) fn selected_is_artist(&self) -> bool {
        self.browser.selected_is_artist()
    }

    /// The focused artist root's component-resolved detail identity (design
    /// D7): settled identity, display name, leaf album targets, and the
    /// snapshot's settled revision. `None` while an album leaf is focused.
    pub(in crate::app) fn artist_detail_target(&self) -> Option<MusicArtistTarget> {
        let key = self.browser.selected_artist_key()?.clone();
        Some(MusicArtistTarget {
            artist_id: match key {
                crate::app::music_grouping::ArtistKey::Service(id) => Some(id),
                crate::app::music_grouping::ArtistKey::Fallback(_) => None,
            },
            artist_name: self.browser.selected_artist_name()?.to_string(),
            album_targets: self.browser.selected_artist_album_targets()?,
            revision: self.context.catalog_revision,
        })
    }

    fn selected_album_index(&self) -> usize {
        self.browser
            .selected_album_target()
            .and_then(|target| {
                self.context
                    .album_targets
                    .iter()
                    .position(|candidate| candidate == target)
            })
            .unwrap_or(0)
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.context.focused = focused;
    }

    /// The album-selection persistence request after a local tree move (design
    /// D3 step 5): emitted only when the resolved selected album changed, and
    /// never when an artist root receives focus — artist focus does not
    /// overwrite album persistence with an artist target.
    fn album_selection_request(&mut self, kind: AlbumCursorKind) -> Option<Msg> {
        // Every local tree movement resolves here after the move: a selection
        // that left the armed root voids its Wide Workspace entry.
        self.void_artist_workspace_focus_off_root();
        if let Some(target) = self.browser.take_album_selection_change() {
            self.last_artist_request = None;
            let index = self
                .context
                .album_targets
                .iter()
                .position(|candidate| *candidate == target)?;
            return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                target: index,
                kind,
            }));
        }
        // Artist roots have no album persistence target (task 2.2). Their
        // focus crosses as the typed artist-track request (design D7) so the
        // resolved identity — not a recomputed cursor — drives the shell's
        // detail fetches; the shell dedupes repeat identities by cache key.
        let target = self.artist_detail_target()?;
        let identity = match &target.artist_id {
            Some(id) => crate::app::music_grouping::ArtistKey::Service(id.clone()),
            None => crate::app::music_grouping::ArtistKey::Fallback(target.artist_name.clone()),
        };
        if self.last_artist_request.as_ref() == Some(&identity) {
            return None;
        }
        self.last_artist_request = Some(identity);
        Some(Msg::Shell(ShellRequest::MusicArtistTracks { target }))
    }

    /// Moves the tree owner's selection by `delta` visible rows and reports the
    /// resolved album-selection change.
    fn move_album(&mut self, delta: i64, kind: AlbumCursorKind) -> Option<Msg> {
        self.browser.move_selection(delta);
        self.album_selection_request(kind)
    }

    /// Moves the tree owner's selection one viewport page and reports the
    /// resolved album-selection change.
    fn page_album(&mut self, delta: i64, kind: AlbumCursorKind) -> Option<Msg> {
        self.browser.page_selection(delta);
        self.album_selection_request(kind)
    }

    pub(in crate::app) fn re_anchor(&mut self, cursor: usize, scroll: usize) {
        let cursor = cursor.min(self.context.album_targets.len().saturating_sub(1));
        if let Some(target) = self.context.album_targets.get(cursor).cloned() {
            // The persisted offset is a flat-flow row (artist row + leaves),
            // not a tree projection row: the tree owner translates it so an
            // interleaved artist root can never anchor the viewport to the
            // wrong album.
            self.browser.anchor_album_target(&target, scroll);
        }
        // A re-anchor is a discrete navigation transition: a cursor that no
        // longer rests on the armed root voids its Wide Workspace entry.
        self.void_artist_workspace_focus_off_root();
    }

    pub(in crate::app) fn set_inline_track_focus_enabled(&mut self, enabled: bool) {
        // The shell pushes this frame's breakpoint: only the Wide pane hosts
        // the inline track list, so the Enter chord must not silently focus a
        // narrow track pane nothing paints.
        self.inline_track_focus_enabled = enabled;
        if !enabled {
            // The armed Wide entry belongs to the inline pane: a geometry
            // that no longer hosts it voids the entry, so no later push or
            // draw can take the focus on a narrow pane nothing paints.
            self.pending_artist_workspace_focus = None;
        }
        // An ordinary Narrow push clears the pre-overlay surface's track
        // focus (design D5), but must not wipe the open Library Hero
        // overlay's Workspace focus. Explicit shell clears are separate
        // one-shot requests (`MusicTrackFocusRequest::Clear` ->
        // `clear_track_focus`) or the dismiss path; they always win and now
        // stay won, because no push re-seizes the focus.
        if !enabled && !self.hero_overlay_open {
            self.track_focused = false;
        }
    }
    pub(in crate::app) fn enter_track_focus(&mut self) {
        if !self.track_list.rows().is_empty() {
            self.track_focused = true;
            self.track_list.select_first();
        }
    }
    /// Right on an expanded artist root in Wide geometry (task 6.4): take the
    /// inline artist-track Workspace's cursor. When the resolved root's rows
    /// have not arrived yet, arm the entry so the next content push takes it —
    /// the component-local twin of the shell's album Enter re-arm.
    fn enter_artist_workspace_focus(&mut self) {
        self.enter_track_focus();
        if !self.track_focused {
            // Arm with the focused root's resolved identity (design D7): the
            // entry may only take the cursor when the landing rows belong to
            // this same root.
            self.pending_artist_workspace_focus = self.artist_detail_target();
        }
    }
    pub(in crate::app) fn clear_track_focus(&mut self) {
        self.track_focused = false;
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn album_cursor(&self) -> usize {
        self.selected_album_index()
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn album_scroll(&self) -> usize {
        self.browser.offset()
    }
    pub(in crate::app) fn track_focused(&self) -> bool {
        self.track_focused
    }
    /// Whether a Wide artist-Workspace entry is currently armed (task 6.4
    /// tests: the armed entry is invisible state, so its void points assert
    /// through this accessor).
    #[cfg(test)]
    pub(in crate::app) fn pending_artist_workspace_focus_for_test(&self) -> bool {
        self.pending_artist_workspace_focus.is_some()
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn album_flow_targets(&self) -> Vec<Option<String>> {
        // The tree's visible-node row flow: one entry per projected node, an
        // album target for leaves and `None` for artist roots.
        self.browser.projected_targets()
    }

    /// Expands every artist root (test fixture for the tree's settled visible
    /// album order).
    #[cfg(test)]
    pub(in crate::app) fn expand_all_tree_roots(&mut self) {
        self.browser.expand_all_roots();
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn track_selected_row(&self) -> Option<usize> {
        let target = self.track_list.selected_target()?;
        if let Some(detail) = self.current_artist_detail() {
            return detail
                .track_groups
                .iter()
                .flat_map(|group| group.tracks.iter())
                .position(|track| track.id == *target);
        }
        self.context
            .album_tracks
            .as_deref()?
            .iter()
            .position(|track| track.id == *target)
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
            && self.browser.selected_is_artist()
            && self.current_artist_detail().is_some()
    }

    pub(in crate::app) fn selected_track_item(&self) -> Option<EmbyItem> {
        let target = self.track_list.selected_target()?;
        self.workspace_track_item(target)
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
        if self.browser.selected_is_artist() {
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

    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        self.resolved_hero_data()
    }

    pub(in crate::app) fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    fn on_filter_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.selected_is_artist() {
                return match key.code {
                    Key::Char('p') => self.artist_action(MusicTreeAction::Play),
                    Key::Char('a') => self.artist_action(MusicTreeAction::Enqueue),
                    Key::Char('s') => self.artist_action(MusicTreeAction::Shuffle),
                    _ => None,
                };
            }
            let item = self.selected_item();
            return match key.code {
                Key::Char('p') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryPlay { item }))
                }
                Key::Char('a') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryEnqueue { item }))
                }
                Key::Char('s') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryShuffle { item }))
                }
                _ => None,
            };
        }
        match key.code {
            Key::Esc => {
                self.inline_search.close();
                self.browser.close_filter();
                None
            }
            Key::Backspace => {
                let action = self.inline_search.handle_key(key);
                if matches!(
                    action,
                    Some(super::inline_search::InlineSearchAction::Dismiss)
                ) {
                    self.inline_search.close();
                    self.browser.close_filter();
                } else if self.inline_search.query().is_empty() {
                    self.browser.apply_filter_query("");
                }
                None
            }
            Key::Char(_) => {
                let _ = self.inline_search.handle_key(key);
                None
            }
            Key::Up => self.move_album(-1, AlbumCursorKind::Move),
            Key::Down => self.move_album(1, AlbumCursorKind::Move),
            Key::PageUp => self.page_album(-1, AlbumCursorKind::Page),
            Key::PageDown => self.page_album(1, AlbumCursorKind::Page),
            Key::Home => {
                self.browser.select_first_visible();
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::End => {
                self.browser.select_last_visible();
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::Left if self.browser.selected_is_artist() => {
                if let Some(root) = self.browser.selected_id() {
                    if self.browser.root_is_expanded(root) {
                        self.browser.collapse_root(root);
                    }
                }
                None
            }
            Key::Left => {
                self.browser.move_to_parent();
                self.album_selection_request(AlbumCursorKind::Move)
            }
            Key::Right if self.browser.selected_is_artist() => {
                let root = self.browser.selected_id()?;
                if !self.browser.root_is_expanded(root) {
                    self.browser.expand_root(root);
                    None
                } else {
                    self.artist_detail_target()
                        .map(|target| Msg::Shell(ShellRequest::MusicArtistActivate { target }))
                }
            }
            Key::Right => {
                if let Some(id) = self.browser.selected_id() {
                    self.browser.expand_node(id);
                }
                None
            }
            Key::Enter if self.browser.selected_is_artist() => {
                if let Some(root) = self.browser.selected_id() {
                    self.browser.toggle_root(root);
                }
                None
            }
            Key::Enter if self.browser.selected_is_track() => {
                let (album_id, track) = self.selected_tree_track()?;
                Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                    album_id,
                    track,
                }))
            }
            Key::Enter => {
                let target = self.browser.selected_album_target()?.to_string();
                let item = self.selected_item()?;
                self.inline_search.close();
                self.browser.close_filter();
                self.browser.select_album_target(&target);
                if self.track_list.rows().is_empty() {
                    Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item }))
                } else if self.inline_track_focus_enabled {
                    self.enter_track_focus();
                    None
                } else {
                    Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item }))
                }
            }
            _ => None,
        }
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
                    header: Some("TRACKLIST"),
                    selector: None,
                    list: &mut self.track_list,
                    focused: focused && track_focused,
                }),
            }
        });
        self.browser
            .set_search_bar(self.inline_search.query(), false);
        let selector = (!self.context.groups.is_empty()).then(|| SelectorRow {
            pills: self
                .context
                .groups
                .iter()
                .map(|group| trunc_str(&group.name, 12).to_string())
                .collect(),
            active: Some(self.context.group_cursor),
        });
        // Grouped Music keeps the tree as the browser owner while the shared
        // Inline Search control supplies only the query editor/debounce and
        // the panel's one-row search-bar projection.
        let list = ListSlot::Media(&mut self.browser);
        LibraryPanelContent {
            selector,
            list,
            hero,
        }
    }
}
include!("music_interaction.rs");
impl Default for MusicContent {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineSearchHost for MusicContent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }
    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }

    fn uses_local_filter(&self) -> bool {
        true
    }

    fn open_inline_search(&mut self) {
        if !self.inline_search.is_active() {
            self.inline_search.open();
            self.browser.open_filter();
        }
    }

    fn close_inline_search(&mut self) {
        if self.inline_search.is_active() {
            self.inline_search.close();
            self.browser.close_filter();
        }
    }

    fn inline_search_debounced(&mut self) {
        let query = self.inline_search.query().to_string();
        self.browser.apply_filter_query(&query);
    }
}

impl LibraryContentOwner for MusicContent {
    fn clear_selection(&mut self) {
        // The tree's multi-selection is task 4.2 scope; the destination switch
        // clears the browser's stored marks so no stale selection mark
        // survives into a new destination.
        self.browser.clear_marks();
    }

    fn hero_overlay_target_available(&mut self) -> bool {
        // Both hero-bearing tree rows can own the overlay before their Hero
        // snapshot materializes: an album leaf and an artist root.
        self.selected_item().is_some() || self.browser.selected_is_artist()
    }

    fn hero_overlay_enter_available(&mut self) -> bool {
        // Enter is the album leaf's overlay entry; an artist root's Enter
        // toggles its expansion (its overlay entry is Right on the already
        // expanded root).
        self.selected_item().is_some()
    }

    fn inline_search_session(&mut self) -> Option<&mut dyn InlineSearchHost> {
        Some(self)
    }

    fn inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        Some(self)
    }

    fn set_selection_origin(
        &mut self,
        origin: crate::app::components::media_list::SelectionOrigin,
    ) {
        // The panel's active-owner projection records the stable identity
        // used by both the status pill and bulk-action clear routing. Direct
        // tree actions carry the same identity without exposing membership.
        self.selection_origin = Some(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
        // The tree keeps membership locally; expose only the same read-only
        // count/origin projection used by every canonical list. Music rows
        // never inspect played/unplayed state here.
        self.selection_origin.clone().map(|origin| {
            crate::app::components::media_list::SelectionSummary {
                count: self.browser.selected_album_targets().len(),
                origin,
            }
        })
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.panel_content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.on_slot_event(event)
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.inline_search.is_active() {
            // The production Grouped Music session never populates the flat
            // carrier. Keep the legacy host hook usable for focused harnesses
            // that explicitly seed that carrier while exercising unrelated
            // activation plumbing; the panel still always paints the tree.
            if self.browser.filter_active()
                && !self.inline_search.has_pool_entries()
                && self.inline_search.results_len() == 0
            {
                return self.on_filter_key(key);
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                if let Some(item) = self.inline_search.selected_item() {
                    let request = match key.code {
                        Key::Char('p') => Some(ShellRequest::EmbyLibraryPlay { item }),
                        Key::Char('a') => Some(ShellRequest::EmbyLibraryEnqueue { item }),
                        Key::Char('s') => Some(ShellRequest::EmbyLibraryShuffle { item }),
                        _ => None,
                    };
                    if let Some(request) = request {
                        self.inline_search.close();
                        self.browser.close_filter();
                        return Some(Msg::Shell(request));
                    }
                }
            }
            return match self.inline_search.handle_key(key) {
                Some(super::inline_search::InlineSearchAction::Activate { id, item_type }) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                        id,
                        item_type,
                    }))
                }
                Some(super::inline_search::InlineSearchAction::Dismiss) => {
                    self.inline_search.close();
                    self.browser.close_filter();
                    None
                }
                Some(super::inline_search::InlineSearchAction::QueryStarted) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchQueryStarted))
                }
                None => None,
            };
        }
        // The LibraryPanel is the framework focus boundary; reaching this
        // method already proves Music is focused.
        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused {
            if self.selected_is_artist() {
                return match key.code {
                    Key::Char('p') => self.artist_action(MusicTreeAction::Play),
                    Key::Char('a') => self.artist_action(MusicTreeAction::Enqueue),
                    Key::Char('s') => self.artist_action(MusicTreeAction::Shuffle),
                    // Artist roots are grouping rows, so watched-state is
                    // unavailable while the library-wide rescan remains
                    // available from every focused library row.
                    Key::Char('w') => None,
                    Key::Char('r') => Some(Msg::Shell(ShellRequest::EmbyLibraryRescan)),
                    _ => None,
                };
            }
            let item = self.selected_item();
            return match key.code {
                Key::Char('p') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryPlay { item }))
                }
                Key::Char('a') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryEnqueue { item }))
                }
                Key::Char('s') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryShuffle { item }))
                }
                Key::Char('w') => {
                    item.map(|item| Msg::Shell(ShellRequest::EmbyLibraryToggleWatched { item }))
                }
                Key::Char('r') => Some(Msg::Shell(ShellRequest::EmbyLibraryRescan)),
                _ => None,
            };
        }
        match key.code {
            // An artist root is a grouping row, not an album: Enter toggles its
            // persistent expansion (task 2.4) while the tree rail owns the
            // focus. Once the focused pane is this root's own artist Workspace
            // (task 6.3), Enter belongs to the focused track below; a pane left
            // over from an album is not that Workspace and must not swallow the
            // chord through the track arm's track/album resolution.
            Key::Enter if self.browser.selected_is_artist() && !self.artist_workspace_focused() => {
                if let Some(root) = self.browser.selected_id() {
                    self.browser.toggle_root(root);
                }
                None
            }
            Key::Enter if self.track_focused => {
                let track = self.selected_track_item()?;
                let album_id = self.focused_track_album_id()?;
                Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                    album_id,
                    track,
                }))
            }
            Key::Enter if self.browser.selected_is_track() => {
                let (album_id, track) = self.selected_tree_track()?;
                Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                    album_id,
                    track,
                }))
            }
            Key::Enter if self.track_list.rows().is_empty() => self
                .selected_item()
                .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item })),
            // Narrow geometry has no inline track pane: the chord opens (or
            // re-focuses) the Library Hero overlay instead of focusing a list
            // nothing paints.
            Key::Enter if self.inline_track_focus_enabled => {
                self.enter_track_focus();
                None
            }
            Key::Enter => self
                .selected_item()
                .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item })),
            Key::Esc | Key::Backspace if self.track_focused => {
                self.clear_track_focus();
                None
            }
            Key::Up | Key::Char('k') if self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Move(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            Key::Down | Key::Char('j') if self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Move(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            // The Library Hero overlay's pager/jump chords move the focused
            // track list, never the covered album browser (the overlay is
            // Narrow-only; Wide keeps the album-rail paging arms below).
            Key::PageUp if self.hero_overlay_open && self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Page(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            Key::PageDown if self.hero_overlay_open && self.track_focused => {
                self.track_list.delegate_operation(
                    MediaListSurfaceInput::Page(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            Key::Home if self.hero_overlay_open && self.track_focused => {
                self.track_list.select_first();
                None
            }
            Key::End if self.hero_overlay_open && self.track_focused => {
                self.track_list.select_last();
                None
            }
            Key::Char('/') => {
                if !self.inline_search.is_active() {
                    self.inline_search.open();
                    self.browser.open_filter();
                }
                Some(Msg::Shell(ShellRequest::OpenInlineSearch))
            }
            // Context menu: the focused track's own menu while the track
            // pane holds local focus, otherwise the selected album's
            // generic library context menu (mirrors the retired
            // `MusicWorkspaceComponent`'s '.' handling).
            Key::Char('.') if self.track_focused => {
                match self
                    .track_list
                    .delegate_operation(
                        MediaListSurfaceInput::Context
                            .into_operation(None)
                            .expect("resolved media-list pointer target"),
                    )
                    .external_intent
                {
                    Some(RowIntent::ContextSelection(targets)) => {
                        let items: Vec<EmbyItem> = targets
                            .into_iter()
                            .filter_map(|target| self.workspace_track_item(&target))
                            .collect();
                        (!items.is_empty()).then_some(Msg::Shell(ShellRequest::RowContextMenu(
                            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                            None,
                        )))
                    }
                    Some(RowIntent::Context(target)) => {
                        self.workspace_track_item(&target).map(|track| {
                            Msg::Shell(ShellRequest::RowContextMenu(
                                crate::app::types_context_menu::ContextMenuTargets::Emby(vec![
                                    track,
                                ]),
                                None,
                            ))
                        })
                    }
                    _ => None,
                }
            }
            Key::Char('.') => {
                if self.selected_is_artist() {
                    let (items, _unresolved_targets) = self.selected_artist_items()?;
                    if items.is_empty() {
                        return None;
                    }
                    Some(Msg::Shell(ShellRequest::RowContextMenu(
                        crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                        None,
                    )))
                } else {
                    self.selected_item().map(|item| {
                        Msg::Shell(ShellRequest::RowContextMenu(
                            crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]),
                            None,
                        ))
                    })
                }
            }
            Key::Char('r')
                if !self.track_focused
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Some(Msg::Shell(ShellRequest::EmbyLibraryRefresh))
            }
            Key::Char('[') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: -1 }))
            }
            Key::Char(']') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: 1 }))
            }
            // Album-level navigation (unfocused track pane): the earlier
            // `self.track_focused` arms above take precedence while the
            // track pane holds local focus. The one tree owner supplies the
            // visible-node movement; a resolved album selection crosses as
            // the existing `MusicAlbumCursor` request.
            Key::Up | Key::Char('k') => self.move_album(-1, AlbumCursorKind::Move),
            Key::Down | Key::Char('j') => self.move_album(1, AlbumCursorKind::Move),
            Key::Home => {
                self.browser.select_first_visible();
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::End => {
                self.browser.select_last_visible();
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::PageUp => self.page_album(-1, AlbumCursorKind::Page),
            Key::PageDown => self.page_album(1, AlbumCursorKind::Page),
            // Left/Right are the tree's parent/child movement (task 2.4):
            // Right expands a collapsed artist root; Left collapses a focused
            // expanded root or returns a leaf to its artist parent. These fire
            // only when the track pane does not hold local focus, and only
            // after the router's fall-through — in the Both layout the central
            // `panel_left` precedence still claims plain Left before the tree
            // ever sees it. Right on an already expanded root (the artist
            // Workspace entry) is task 6.4 and stays unhandled here.
            Key::Left if !self.track_focused => {
                if self.browser.selected_is_artist() {
                    if let Some(root) = self.browser.selected_id() {
                        if self.browser.root_is_expanded(root) {
                            self.browser.collapse_root(root);
                        }
                    }
                    None
                } else if self.browser.move_to_parent() {
                    // The parent is an artist root, so no album selection
                    // crosses: artist focus never overwrites album persistence.
                    self.album_selection_request(AlbumCursorKind::Move)
                } else {
                    None
                }
            }
            // Right first expands cached track children on an album node;
            // the shell already owns any missing album-track fetch and this
            // local operation only projects settled cache data.
            Key::Right if !self.track_focused && !self.browser.selected_is_artist() => {
                if let Some(id) = self.browser.selected_id() {
                    if !self.browser.node_is_expanded(id) {
                        self.browser.expand_node(id);
                    }
                }
                None
            }
            // Right on an artist root (task 2.4/task 6.4): a collapsed root
            // expands first; only a later Right on the already expanded root
            // enters its artist Workspace — Wide takes the inline pane's
            // cursor locally, non-Wide asks the shell to open the Library
            // Hero overlay and focus the same Workspace.
            Key::Right if !self.track_focused && self.browser.selected_is_artist() => {
                let root = self.browser.selected_id()?;
                if !self.browser.root_is_expanded(root) {
                    self.browser.expand_root(root);
                    return None;
                }
                if self.inline_track_focus_enabled {
                    self.enter_artist_workspace_focus();
                    return None;
                }
                self.artist_detail_target()
                    .map(|target| Msg::Shell(ShellRequest::MusicArtistActivate { target }))
            }
            _ => None,
        }
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        let active = self.inline_search.is_active();
        match self.on_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if (active
                && matches!(
                    key.code,
                    Key::Esc
                        | Key::Enter
                        | Key::Backspace
                        | Key::Up
                        | Key::Down
                        | Key::Left
                        | Key::Right
                        | Key::Char(_)
                ))
                || (self.track_focused
                    && matches!(
                        key.code,
                        Key::Enter
                            | Key::Esc
                            | Key::Backspace
                            | Key::Up
                            | Key::Down
                            | Key::Char('k' | 'j' | '.')
                    )) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    fn inline_search_active(&self) -> bool {
        self.inline_search.is_active()
    }

    fn focus_hero_workspace(&mut self) -> bool {
        self.enter_track_focus();
        true
    }

    fn clear_hero_workspace_focus(&mut self) {
        self.clear_track_focus();
    }

    fn set_hero_overlay_open(&mut self, open: bool) {
        // The overlay takes the Workspace focus exactly once, on its open
        // transition (bit false->true) -- never again on the sync pass's
        // bit re-assert or an ordinary push, so shell and local focus
        // clears stay authoritative while the overlay is open.
        if open && !self.hero_overlay_open {
            self.enter_track_focus();
        }
        self.hero_overlay_open = open;
    }

    fn post_paint_message(&mut self) -> Option<Msg> {
        // Task 6.5 (design D4): the tree owner resolved the neighbour album
        // artwork window from the frame it just painted; the shell applies
        // the existing idle gate and fetches the typed targets.
        let targets = self.browser.neighbour_prefetch_targets()?;
        Some(Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets }))
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.set_hero_image(state)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
#[path = "music_content_tests.rs"]
mod tests;
