//! Grouped Music's embedded Library panel content owner (task 9.1).
//!
//! `MusicContent` owns the shell-projected album/track snapshot, the shared
//! album and track list controls, local track focus, and the Inline Search
//! session.  It is a plain [`LibraryContentOwner`]; the legacy
//! `MusicWorkspaceComponent` temporarily borrows this owner for its existing
//! painters until the later Music panel slices move painting and registration.

use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use mbv_core::config::{EmbySelectorKey, LibraryItemIdentity, SelectorIdentity};
use std::collections::HashMap;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use self::workspace::WorkspaceOwner;
use super::inline_search::{InlineSearch, InlineSearchHost};
use super::library_panel::content::{
    ArtworkShape, HeroArtwork, HeroContent, HeroFacts, HeroImageState, LibraryPanelContent,
    ListSlot, SelectorRow, Workspace, WorkspaceHeader,
};
use super::library_panel::hero::{hero_content_music_album, music_album_artwork};
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaListSurfaceInput, MediaListTrailing,
    MediaSemanticState, RowIntent, SelectionOrigin,
};
use super::msg::{AlbumCursorKind, Msg, MusicArtistTarget, MusicTreeAction, ShellRequest};
use super::msg::{LeafKeyResult, TerminalObserverEvent};
use super::music_tree_target::MusicTreeTarget;
use crate::app::components::list::tree_browser::{
    TreeBrowser, TreeConsumed, TreeMarkPolicy, TreeNode, TreeOperation,
};
use crate::app::render::MusicWideRenderCtx;
use crate::app::ui_util::{fmt_duration_gutter, trunc_str};

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
/// The neighbour album-artwork window (design D4): the shell prefetches up
/// to one visible album leaf behind the selected leaf and up to three ahead.
const NEIGHBOUR_PREFETCH_BEHIND: usize = 1;
const NEIGHBOUR_PREFETCH_AHEAD: usize = 3;

pub struct MusicContent {
    pub(in crate::app) context: MusicWideRenderCtx,
    /// The Grouped Music browser: the shared embedded tree owner over
    /// `MusicTreeTarget` rows. It is the only browser owner and painter — the
    /// destination projects settled snapshots into nodes and translates the
    /// owner's stable-target transitions (design D6).
    pub(in crate::app) browser: TreeBrowser<MusicTreeTarget>,
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
    last_artist_request: Option<crate::app::state::music_grouping::ArtistKey>,
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
    /// The last album target reported for the shell's destination-position
    /// persistence (design D3 step 5). An artist-root focus resolves to no
    /// album and never clears or overwrites it. This is a destination
    /// translation of the shared owner's selected stable target.
    last_reported_album: Option<String>,
    /// Whether this owner already adopted the shell's projected album position
    /// (design D3 step 4). Later pushes never re-point the tree — the tree
    /// owns selection, and an explicit shell re-anchor request (`re_anchor`)
    /// adopts a navigated position.
    tree_adopted: bool,
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
            browser: TreeBrowser::new(),
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
            last_reported_album: None,
            tree_adopted: false,
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: MusicWideRenderCtx) {
        // Content projection never carries framework focus; preserve the
        // component-owned value across the shell snapshot swap.
        let focused = self.context.focused;
        self.context = context;
        self.context.focused = focused;

        self.project_tree_tracks();
        let projection = self.tree_projection();
        let reconciled = self.browser.reconcile(projection);
        // A rejected projection is the typed D2 result that leaves the
        // previous tree unchanged; the projection is generated from settled
        // snapshots, so a rejection is a destination bug. Surface it loudly
        // in debug builds instead of freezing the tree behind a swallowed
        // error.
        debug_assert!(
            reconciled.is_ok(),
            "music tree projection rejected: {:?}",
            reconciled.as_ref().err()
        );
        // A fresh owner adopts the shell's projected album position once
        // (design D3 step 4). The first push can be a loading/empty snapshot
        // with no target to adopt, so the latch is set only when adoption
        // actually resolved one; later pushes retry until it lands, then
        // never re-point the tree — the tree owns selection, and an explicit
        // shell re-anchor request (`re_anchor`) adopts a navigated position.
        if !self.tree_adopted && reconciled.is_ok() {
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
                let transition = self.browser.apply(TreeOperation::AnchorSelection {
                    target: MusicTreeTarget::Album(target),
                    flow_offset: 0,
                });
                self.tree_adopted = transition.disposition == TreeConsumed::Consumed;
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
}

impl MusicContent {
    fn selected_album_index(&self) -> usize {
        self.selected_album_target()
            .and_then(|target| {
                self.context
                    .album_targets
                    .iter()
                    .position(|candidate| candidate == &target)
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
        if let Some(target) = self.take_album_selection_change() {
            self.last_artist_request = None;
            let index = self
                .context
                .album_targets
                .iter()
                .position(|candidate| *candidate == target)?;
            return Some(Msg::Shell(Box::new(ShellRequest::MusicAlbumCursor {
                target: index,
                kind,
            })));
        }
        // Artist roots have no album persistence target (task 2.2). Their
        // focus crosses as the typed artist-track request (design D7) so the
        // resolved identity — not a recomputed cursor — drives the shell's
        // detail fetches; the shell dedupes repeat identities by cache key.
        let target = self.artist_detail_target()?;
        let identity = match &target.artist_id {
            Some(id) => crate::app::state::music_grouping::ArtistKey::Service(id.clone()),
            None => {
                crate::app::state::music_grouping::ArtistKey::Fallback(target.artist_name.clone())
            }
        };
        if self.last_artist_request.as_ref() == Some(&identity) {
            return None;
        }
        self.last_artist_request = Some(identity);
        Some(Msg::Shell(Box::new(ShellRequest::MusicArtistTracks {
            target,
        })))
    }

    /// The album-selection persistence translation for the shell (design D3
    /// step 5): `Some(target)` only when the resolved selected album differs
    /// from the last one reported, and `None` while an artist root is focused
    /// — an artist focus never overwrites (or clears) the retained album
    /// identity.
    fn take_album_selection_change(&mut self) -> Option<String> {
        let resolved = self.selected_album_target()?;
        if self.last_reported_album.as_deref() != Some(resolved.as_str()) {
            self.last_reported_album = Some(resolved.clone());
            return Some(resolved);
        }
        None
    }

    /// Moves the tree owner's selection by `delta` visible rows and reports the
    /// resolved album-selection change.
    fn move_album(&mut self, delta: i64, kind: AlbumCursorKind) -> Option<Msg> {
        self.browser.apply(TreeOperation::Move(delta));
        self.album_selection_request(kind)
    }

    /// Moves the tree owner's selection one viewport page and reports the
    /// resolved album-selection change.
    fn page_album(&mut self, delta: i64, kind: AlbumCursorKind) -> Option<Msg> {
        self.browser.apply(TreeOperation::Page(delta));
        self.album_selection_request(kind)
    }

    pub(in crate::app) fn re_anchor(&mut self, cursor: usize, scroll: usize) {
        let cursor = cursor.min(self.context.album_targets.len().saturating_sub(1));
        if let Some(target) = self.context.album_targets.get(cursor).cloned() {
            // The persisted offset is a settled-flow row (artist row +
            // leaves), not a tree projection row: the tree owner translates
            // it so an interleaved artist root can never anchor the viewport
            // to the wrong album.
            self.browser.apply(TreeOperation::AnchorSelection {
                target: MusicTreeTarget::Album(target),
                flow_offset: scroll,
            });
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
        // A tree move can leave the previous album Workspace focused until the
        // next panel projection. Reconcile now so an artist root never adopts
        // that stale carrier as its own Workspace.
        self.reconcile_workspace_rows();
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
    #[cfg(test)]
    pub(in crate::app) fn album_cursor(&self) -> usize {
        self.selected_album_index()
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
    #[cfg(test)]
    pub(in crate::app) fn album_flow_targets(&self) -> Vec<Option<String>> {
        // The tree's visible-node row flow: one entry per projected node, an
        // album target for leaves and `None` for artist roots and tracks.
        self.browser
            .visible_targets()
            .iter()
            .map(|target| target.album_leaf_target().map(str::to_string))
            .collect()
    }

    /// Expands every projected node (test fixture for the tree's settled
    /// visible order, tracks included).
    #[cfg(test)]
    pub(in crate::app) fn expand_all_tree_roots(&mut self) {
        let targets: Vec<MusicTreeTarget> = self.browser.visible_targets();
        for target in targets {
            if !self.browser.is_expanded(&target) {
                self.browser
                    .apply(TreeOperation::ToggleExpansionTarget(target));
            }
        }
    }

    #[cfg(test)]
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
    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        self.resolved_hero_data()
    }

    pub(in crate::app) fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    fn on_filter_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            self.on_filter_control_key(key)
        } else {
            self.on_filter_unmodified_key(key)
        }
    }

    fn on_filter_control_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.selected_is_artist() {
            return match key.code {
                Key::Char('p') => self.artist_action(MusicTreeAction::Play),
                Key::Char('a') => self.artist_action(MusicTreeAction::Enqueue),
                Key::Char('s') => self.artist_action(MusicTreeAction::Shuffle),
                _ => None,
            };
        }
        let item = self.selected_item();
        match key.code {
            Key::Char('p') => {
                item.map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryPlay { item })))
            }
            Key::Char('a') => {
                item.map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryEnqueue { item })))
            }
            Key::Char('s') => {
                item.map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryShuffle { item })))
            }
            _ => None,
        }
    }

    fn on_filter_unmodified_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Esc | Key::Backspace | Key::Char(_) => self.on_filter_edit_key(key),
            Key::Up
            | Key::Down
            | Key::PageUp
            | Key::PageDown
            | Key::Home
            | Key::End
            | Key::Left
            | Key::Right => self.on_filter_navigation_key(key),
            Key::Enter => self.on_filter_enter_key(),
            _ => None,
        }
    }

    fn on_filter_edit_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Esc => {
                self.inline_search.close();
                self.browser.apply(TreeOperation::ClearFilter);
                None
            }
            Key::Backspace => {
                let action = self.inline_search.handle_key(key);
                if matches!(
                    action,
                    Some(super::inline_search::InlineSearchAction::Dismiss)
                ) {
                    self.inline_search.close();
                    self.browser.apply(TreeOperation::ClearFilter);
                } else if self.inline_search.query().is_empty() {
                    self.browser.apply(TreeOperation::EditFilter(String::new()));
                }
                None
            }
            Key::Char(_) => {
                let _ = self.inline_search.handle_key(key);
                None
            }
            _ => None,
        }
    }

    fn on_filter_navigation_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Up => self.move_album(-1, AlbumCursorKind::Move),
            Key::Down => self.move_album(1, AlbumCursorKind::Move),
            Key::PageUp => self.page_album(-1, AlbumCursorKind::Page),
            Key::PageDown => self.page_album(1, AlbumCursorKind::Page),
            Key::Home => {
                self.browser.apply(TreeOperation::First);
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::End => {
                self.browser.apply(TreeOperation::Last);
                self.album_selection_request(AlbumCursorKind::Jump)
            }
            Key::Left if self.selected_is_artist() => {
                if let Some(root) = self.browser.selected_target().cloned() {
                    if self.browser.is_expanded(&root) {
                        self.browser
                            .apply(TreeOperation::ToggleExpansionTarget(root));
                    }
                }
                None
            }
            Key::Left => {
                self.browser.apply(TreeOperation::Parent);
                self.album_selection_request(AlbumCursorKind::Move)
            }
            Key::Right => {
                let intent = self.browser.apply(TreeOperation::Right).external_intent;
                match intent {
                    Some(
                        crate::app::components::list::tree_browser::TreeExternalIntent::Activate(_),
                    ) if self.selected_is_artist() => self.artist_detail_target().map(|target| {
                        Msg::Shell(Box::new(ShellRequest::MusicArtistActivate { target }))
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn on_filter_enter_key(&mut self) -> Option<Msg> {
        if self.selected_is_artist() {
            if let Some(root) = self.browser.selected_target().cloned() {
                self.browser
                    .apply(TreeOperation::ToggleExpansionTarget(root));
            }
            return None;
        }
        if self.selected_is_track() {
            let (album_target, track_id) = self.selected_tree_track()?;
            return Some(Msg::Shell(Box::new(ShellRequest::MusicTreeTrackActivate {
                album_target,
                track_id,
            })));
        }

        // Capture the activated target before the dismissal restores the
        // pre-filter anchor, then re-select it: the shared `ClearFilter`
        // restores `filter_anchor`, so without this the tree (and with it
        // Hero/Workspace/persistence) would revert to the row the filter
        // started on. `AnchorSelection` rather than `Select` because the
        // filter can reach a leaf under a collapsed artist root: `Select`
        // only addresses a visible row, so the re-selection would come back
        // `Unhandled` and the tree would revert. `AnchorSelection` reveals
        // the ancestor path (legacy `select_album_target` parity) and re-arms
        // the viewport visibility rule for the next view.
        let target = self.browser.selected_target().cloned();
        let item = self.selected_item()?;
        self.inline_search.close();
        self.browser.apply(TreeOperation::ClearFilter);
        if let Some(target) = target {
            self.browser.apply(TreeOperation::AnchorSelection {
                target,
                flow_offset: 0,
            });
        }
        if self.track_list.rows().is_empty() {
            Some(Msg::Shell(Box::new(ShellRequest::MusicAlbumActivate {
                item,
            })))
        } else if self.inline_track_focus_enabled {
            self.enter_track_focus();
            None
        } else {
            Some(Msg::Shell(Box::new(ShellRequest::MusicAlbumActivate {
                item,
            })))
        }
    }
}

mod interaction;
mod owner;
mod tree_target;
mod workspace;

impl Default for MusicContent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
