//! Grouped Music's embedded Library panel content owner (task 9.1).
//!
//! `MusicContent` owns the shell-projected album/track snapshot, the shared
//! album and track list controls, local track focus, and the Inline Search
//! session.  It is a plain [`LibraryContentOwner`]; the legacy
//! `MusicWorkspaceComponent` temporarily borrows this owner for its existing
//! painters until the later Music panel slices move painting and registration.

use mbv_core::api::EmbyItem;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::inline_search::{InlineSearch, InlineSearchHost};
use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow, Workspace,
};
use super::library_panel::hero::hero_content_music_album;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaSemanticState, RowIntent,
};
use super::msg::{AlbumCursorKind, Msg, ShellRequest};
use super::msg::{LeafKeyResult, TerminalObserverEvent};
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
        .map(|(index, track)| {
            let number = if track.index_number > 0 {
                track.index_number
            } else {
                index as i64 + 1
            };
            // Library lists carry no time column (only the Queue list and
            // the sessions modal show one).
            let duration = None;
            MediaListRow::Item {
                target: track.id.clone(),
                primary: format!("{number}. {}", track.name),
                secondary: None,
                trailing: None,
                duration,
                kind: MediaKind::Media,
                // The one canonical state derivation.
                semantic_state: MediaSemanticState::from_progress(
                    track.played,
                    track.playback_position_ticks,
                    track.runtime_ticks,
                ),
            }
        })
        .collect()
}

/// The plain Music content owner. Its list controls retain cursor, scroll,
/// selected targets, and track focus locally; shell pushes replace only the
/// content snapshot and never mirror those interaction values.
pub struct MusicContent {
    pub(in crate::app) context: MusicWideRenderCtx,
    pub(in crate::app) carrier: MediaListCarrier<String>,
    pub(in crate::app) track_list: MediaListCarrier<String>,
    pub(in crate::app) track_focused: bool,
    /// Whether this frame's geometry hosts the inline track list (the Wide
    /// pane). Pushed each sync pass beside the track-focus clear; narrow
    /// selects the Library Hero overlay instead.
    pub(in crate::app) inline_track_focus_enabled: bool,
    last_album_id: Option<String>,
    pub(in crate::app) inline_search: InlineSearch,
    hero_image: HeroImageState,
    /// Whether the Library Hero overlay is open over this owner (pushed by
    /// the panel). The overlay takes the Workspace's keyboard focus once, on
    /// its open transition; ordinary pushes never focus or re-focus the
    /// track pane, and an explicit shell focus clear always wins.
    hero_overlay_open: bool,
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
                None,
            ),
            carrier: MediaListCarrier::new(),
            track_list: MediaListCarrier::new(),
            track_focused: false,
            inline_track_focus_enabled: false,
            last_album_id: None,
            inline_search: InlineSearch::new(),
            hero_image: HeroImageState::None,
            hero_overlay_open: false,
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: MusicWideRenderCtx) {
        let album_changed = self.last_album_id.as_deref()
            != context
                .selected_album
                .as_ref()
                .map(|album| album.id.as_str());
        if album_changed {
            self.track_focused = false;
        }
        self.last_album_id = context
            .selected_album
            .as_ref()
            .map(|album| album.id.clone());

        // Content projection never carries framework focus; preserve the
        // component-owned value across the shell snapshot swap.
        let focused = self.context.focused;
        self.context = context;
        self.context.focused = focused;

        let album_rows = self.context.grouped_rows();
        if self.carrier.rows() != album_rows.as_slice() {
            self.carrier.set_content(album_rows);
        }
        let track_rows = build_track_rows(self.context.album_tracks.as_deref().unwrap_or_default());
        // The overlay's Workspace can outrun the album's track fetch: the open
        // transition cannot take the focus while the rows are still empty, so
        // it is taken the moment they arrive. Only the empty-to-non-empty edge
        // fires, so an explicit Esc (or any later push) never re-seizes the
        // focus the user left behind.
        let track_rows_arrived = self.track_list.rows().is_empty() && !track_rows.is_empty();
        if self.track_list.rows() != track_rows.as_slice() {
            self.track_list.set_content(track_rows);
        }
        if album_changed {
            self.track_list.select_first();
        }
        if track_rows_arrived && self.hero_overlay_open {
            self.enter_track_focus();
        }
    }

    pub(in crate::app) fn selected_item(&self) -> Option<EmbyItem> {
        self.context
            .list
            .items
            .get(self.selected_album_index())
            .cloned()
    }

    fn selected_album_index(&self) -> usize {
        self.carrier
            .selected_target()
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

    /// Move the shared album owner through the common delegation seam
    /// (design.md D3) and report the resulting selection as the shell's
    /// `MusicAlbumCursor` request; the owner stays authoritative for the
    /// selected album and scroll.
    fn move_album(&mut self, input: MediaListSurfaceInput, kind: AlbumCursorKind) -> Option<Msg> {
        self.carrier.delegate_operation(
            input
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        let target = self.carrier.selected_target()?;
        let index = self
            .context
            .album_targets
            .iter()
            .position(|candidate| candidate == target)?;
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: index,
            kind,
        }))
    }

    pub(in crate::app) fn re_anchor(&mut self, cursor: usize, scroll: usize) {
        let cursor = cursor.min(self.context.list.item_count().saturating_sub(1));
        if let Some(target) = self.context.album_targets.get(cursor).cloned() {
            self.carrier.select_target(&target);
            self.carrier.set_scroll(scroll);
        }
    }

    pub(in crate::app) fn set_inline_track_focus_enabled(&mut self, enabled: bool) {
        // The shell pushes this frame's breakpoint: only the Wide pane hosts
        // the inline track list, so the Enter chord must not silently focus a
        // narrow track pane nothing paints.
        self.inline_track_focus_enabled = enabled;
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
    pub(in crate::app) fn clear_track_focus(&mut self) {
        self.track_focused = false;
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn album_cursor(&self) -> usize {
        self.selected_album_index()
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn album_scroll(&self) -> usize {
        self.carrier.scroll()
    }
    pub(in crate::app) fn track_focused(&self) -> bool {
        self.track_focused
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn album_flow_targets(&self) -> Vec<Option<String>> {
        (0..self.carrier.current_flow_len().unwrap_or(0))
            .map(|row| self.carrier.current_flow_target_at(row).flatten().cloned())
            .collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn album_target_rows(&self, target: usize) -> Vec<usize> {
        self.album_flow_targets()
            .iter()
            .enumerate()
            .filter_map(|(row, value)| {
                (value.as_deref() == self.context.album_targets.get(target).map(String::as_str))
                    .then_some(row)
            })
            .collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn track_selected_row(&self) -> Option<usize> {
        let target = self.track_list.selected_target()?;
        self.context
            .album_tracks
            .as_deref()?
            .iter()
            .position(|track| track.id == *target)
    }
    pub(in crate::app) fn selected_track_item(&self) -> Option<EmbyItem> {
        let target = self.track_list.selected_target()?;
        self.context
            .album_tracks
            .as_deref()?
            .iter()
            .find(|track| track.id == *target)
            .cloned()
    }

    fn resolved_hero_data(&self) -> Option<HeroContentData> {
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
        Some(data)
    }

    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        self.resolved_hero_data().map(|mut data| {
            data.facts.artwork.image = self.hero_image.clone();
            data
        })
    }

    pub(in crate::app) fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    pub(in crate::app) fn panel_content(&mut self) -> LibraryPanelContent<'_> {
        // Copy the selected snapshot and focus bit before borrowing either
        // list mutably for the returned slots.
        let focused = self.context.focused;
        let track_focused = self.track_focused;
        let hero = self.resolved_hero_data().map(|mut data| {
            data.facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts: data.facts,
                // Music has no separate overview box when the album does not
                // provide an overview; the panel's generic producer already
                // represents that as `None`.
                overview: data.overview,
                credits: data.credits,
                workspace: Some(Workspace {
                    header: Some("Tracklist"),
                    selector: None,
                    list: &mut self.track_list,
                    focused: focused && track_focused,
                }),
            }
        });
        let selector = (!self.context.groups.is_empty()).then(|| SelectorRow {
            pills: self
                .context
                .groups
                .iter()
                .map(|group| trunc_str(&group.name, 12).to_string())
                .collect(),
            active: Some(self.context.group_cursor),
        });
        let list = if self.inline_search.is_active() {
            // Search owns the result geometry for this frame; invalidate the
            // ordinary album presentation so stale rail hits cannot survive
            // a search transition.
            self.carrier.invalidate_paint();
            ListSlot::Search(&mut self.inline_search)
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            controls: None,
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
}

impl LibraryContentOwner for MusicContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
    }

    fn hero_overlay_target_available(&mut self) -> bool {
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
        self.carrier.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
        Some(self.carrier.selection_summary())
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.panel_content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.on_slot_event(event)
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.inline_search.is_active() {
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
                    None
                }
                Some(super::inline_search::InlineSearchAction::QueryStarted) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchQueryStarted))
                }
                None => None,
            };
        }
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
        }
        // The LibraryPanel is the framework focus boundary; reaching this
        // method already proves Music is focused.
        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused {
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
            Key::Enter if self.track_focused => {
                let track = self.selected_track_item()?;
                let album = self.selected_item()?;
                Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                    album_id: album.id,
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
                self.inline_search.open();
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
                        Some(Msg::Shell(ShellRequest::RowContextMenu(
                            crate::app::types_context_menu::ContextMenuTargets::Emby(
                                targets
                                    .into_iter()
                                    .filter_map(|target| {
                                        self.context
                                            .album_tracks
                                            .as_deref()
                                            .unwrap_or_default()
                                            .iter()
                                            .find(|track| track.id == target)
                                            .cloned()
                                    })
                                    .collect(),
                            ),
                            None,
                        )))
                    }
                    Some(RowIntent::Context(target)) => self
                        .context
                        .album_tracks
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .find(|track| track.id == target)
                        .cloned()
                        .map(|track| {
                            Msg::Shell(ShellRequest::RowContextMenu(
                                crate::app::types_context_menu::ContextMenuTargets::Emby(vec![
                                    track,
                                ]),
                                None,
                            ))
                        }),
                    _ => None,
                }
            }
            Key::Char('.') => match self
                .carrier
                .delegate_operation(
                    MediaListSurfaceInput::Context
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                )
                .external_intent
            {
                Some(RowIntent::ContextSelection(targets)) => {
                    Some(Msg::Shell(ShellRequest::RowContextMenu(
                        crate::app::types_context_menu::ContextMenuTargets::Emby(
                            targets
                                .into_iter()
                                .filter_map(|target| {
                                    self.context
                                        .list
                                        .items
                                        .iter()
                                        .find(|item| item.id == target)
                                        .cloned()
                                })
                                .collect(),
                        ),
                        None,
                    )))
                }
                Some(RowIntent::Context(_target)) => self.selected_item().map(|item| {
                    Msg::Shell(ShellRequest::RowContextMenu(
                        crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]),
                        None,
                    ))
                }),
                _ => None,
            },
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
            // track pane holds local focus.
            Key::Up | Key::Char('k') => {
                self.move_album(MediaListSurfaceInput::Move(-1), AlbumCursorKind::Move)
            }
            Key::Down | Key::Char('j') => {
                self.move_album(MediaListSurfaceInput::Move(1), AlbumCursorKind::Move)
            }
            Key::Home => self.move_album(MediaListSurfaceInput::First, AlbumCursorKind::Jump),
            Key::End => self.move_album(MediaListSurfaceInput::Last, AlbumCursorKind::Jump),
            Key::PageUp => self.move_album(MediaListSurfaceInput::Page(-1), AlbumCursorKind::Page),
            Key::PageDown => self.move_album(MediaListSurfaceInput::Page(1), AlbumCursorKind::Page),
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
