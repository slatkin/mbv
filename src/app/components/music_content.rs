//! Grouped Music's embedded Library panel content owner (task 9.1).
//!
//! `MusicContent` owns the shell-projected album/track snapshot, the shared
//! album and track list controls, local track focus, and the Inline Search
//! session.  It is a plain [`LibraryContentOwner`]; the legacy
//! `MusicWorkspaceComponent` temporarily borrows this owner for its existing
//! painters until the later Music panel slices move painting and registration.

use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::inline_search::{InlineSearch, InlineSearchHost, SearchPool};
use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow, Workspace,
};
use super::library_panel::hero::hero_content_emby;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, RowLocalInput,
};
use super::msg::TerminalObserverEvent;
use super::msg::{AlbumCursorKind, Msg, ShellRequest};
use crate::app::render::MusicWideRenderCtx;
use crate::app::ui_util::{list_duration_secs, trunc_str};

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
            let duration = list_duration_secs(track.runtime_ticks / TICKS_PER_SECOND);
            MediaListRow::Item {
                target: track.id.clone(),
                primary: format!("{number}. {}", track.name),
                trailing: None,
                duration,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
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
    last_album_id: Option<String>,
    pub(in crate::app) inline_search: InlineSearch,
    hero_image: HeroImageState,
    library_search_active: bool,
}

impl MusicContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            context: MusicWideRenderCtx::new(
                crate::app::render::LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
                None,
                String::new(),
                Vec::new(),
                0,
                Vec::new(),
                Vec::new(),
                false,
                None,
                false,
                false,
            ),
            carrier: MediaListCarrier::new(Presentation::Inline),
            track_list: MediaListCarrier::new(Presentation::Wide),
            track_focused: false,
            last_album_id: None,
            inline_search: InlineSearch::new(),
            hero_image: HeroImageState::None,
            library_search_active: false,
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
        if self.track_list.rows() != track_rows.as_slice() {
            self.track_list.set_content(track_rows);
        }
        if album_changed {
            self.track_list.select_first();
        }

        // The legacy library-search projection still reaches Music while its
        // Narrow painter remains in place. Reuse the same InlineSearch owner
        // for Wide rather than teaching the panel a Music-specific search arm.
        if let Some(query) = self.context.list.search_query.clone() {
            if !self.inline_search.is_active() {
                self.inline_search.open();
                self.inline_search
                    .set_pool(SearchPool::Items(self.context.list.items.clone()));
                self.inline_search.restore_query(query);
            } else if self.inline_search.query() != query {
                self.inline_search.restore_query(query);
            }
            self.inline_search
                .set_loading(self.context.list.search_loading);
            self.library_search_active = true;
        } else if self.library_search_active {
            self.inline_search.close();
            self.library_search_active = false;
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

    pub(in crate::app) fn re_anchor(&mut self, cursor: usize, scroll: usize) {
        let cursor = cursor.min(self.context.list.item_count().saturating_sub(1));
        if let Some(target) = self.context.album_targets.get(cursor).cloned() {
            self.carrier.select_target(&target);
            self.carrier.set_scroll(scroll);
        }
    }

    pub(in crate::app) fn set_inline_track_focus_enabled(&mut self, enabled: bool) {
        if !enabled {
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
    pub(in crate::app) fn album_cursor(&self) -> usize {
        self.selected_album_index()
    }
    pub(in crate::app) fn album_scroll(&self) -> usize {
        self.carrier.scroll()
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
        let mut data = hero_content_emby(&album);
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
                workspace: Some(Workspace {
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

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                let delta = index as i64 - self.context.group_cursor as i64;
                (delta != 0).then_some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta }))
            }
            LibrarySlotEvent::List(input) => {
                if self.inline_search.is_active() {
                    match input {
                        RowLocalInput::Wheel { delta, .. } => {
                            self.inline_search.move_cursor_by(delta);
                            Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                        }
                        RowLocalInput::DoubleClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            self.inline_search.selected_item().map(|item| {
                                Msg::Shell(ShellRequest::InlineSearchActivate {
                                    id: item.id,
                                    item_type: item.item_type,
                                })
                            })
                        }
                        RowLocalInput::ContextClick(at) => {
                            self.inline_search.select_row_at_point(at);
                            self.inline_search.selected_item().map(|item| {
                                Msg::Shell(ShellRequest::EmbyLibraryContextMenu { item })
                            })
                        }
                        RowLocalInput::Click(at) => {
                            self.inline_search.select_row_at_point(at);
                            None
                        }
                        _ => None,
                    }
                } else {
                    match input {
                        RowLocalInput::Wheel { at, delta } => {
                            if self.carrier.claims_current_point(at) {
                                self.carrier
                                    .delegate(RowLocalInput::Wheel { at, delta }, None);
                                let target = self.carrier.selected_target()?;
                                let index = self
                                    .context
                                    .album_targets
                                    .iter()
                                    .position(|candidate| candidate == target)?;
                                Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                                    target: index,
                                    kind: AlbumCursorKind::Move,
                                }))
                            } else {
                                None
                            }
                        }
                        RowLocalInput::Click(at) => {
                            let target = self.carrier.resolve_current_point(at)?.clone();
                            self.carrier
                                .delegate(RowLocalInput::Click(at), Some(target));
                            Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                                target: self.selected_album_index(),
                                kind: AlbumCursorKind::Move,
                            }))
                        }
                        RowLocalInput::DoubleClick(_) => self
                            .selected_item()
                            .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item })),
                        RowLocalInput::ContextClick(at) => self.selected_item().map(|item| {
                            Msg::Shell(ShellRequest::MusicAlbumContextMenu {
                                item,
                                anchor: (at.x, at.y),
                            })
                        }),
                        _ => None,
                    }
                }
            }
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::ControlPicked(_) => {
                None
            }
            LibrarySlotEvent::HeroPane(input) => match input {
                RowLocalInput::Wheel { .. } => None,
                RowLocalInput::Click(at) | RowLocalInput::DoubleClick(at) => {
                    let target = self.track_list.resolve_current_point(at)?.clone();
                    self.track_focused = true;
                    self.track_list
                        .delegate(RowLocalInput::Click(at), Some(target));
                    (matches!(input, RowLocalInput::DoubleClick(_))).then(|| {
                        let track_target = self.track_list.selected_target()?;
                        let track = self
                            .context
                            .album_tracks
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .find(|track| &track.id == track_target)
                            .cloned()?;
                        let album_target = self.carrier.selected_target()?;
                        let album_index = self
                            .context
                            .album_targets
                            .iter()
                            .position(|candidate| candidate == album_target)?;
                        let album = self.context.list.items.get(album_index)?;
                        Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                            album_id: album.id.clone(),
                            track,
                        }))
                    })?
                }
                RowLocalInput::ContextClick(at) => self
                    .track_list
                    .resolve_current_point(at)
                    .and_then(|target| {
                        self.context
                            .album_tracks
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .find(|track| track.id == *target)
                            .cloned()
                            .map(|track| {
                                Msg::Shell(ShellRequest::MusicTrackContextMenuAt {
                                    track,
                                    anchor: (at.x, at.y),
                                })
                            })
                    }),
                _ => None,
            },
        }
    }
}

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
    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.panel_content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.on_slot_event(event)
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.inline_search.is_active() {
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
                None => None,
            };
        }
        if !self.context.focused {
            return None;
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
            Key::Enter => {
                self.enter_track_focus();
                None
            }
            Key::Esc | Key::Backspace if self.track_focused => {
                self.clear_track_focus();
                None
            }
            Key::Up | Key::Char('k') if self.track_focused => {
                self.track_list.delegate(RowLocalInput::Move(-1), None);
                None
            }
            Key::Down | Key::Char('j') if self.track_focused => {
                self.track_list.delegate(RowLocalInput::Move(1), None);
                None
            }
            Key::Char('/') => {
                self.inline_search.open();
                Some(Msg::Shell(ShellRequest::OpenInlineSearch))
            }
            Key::Char('[') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: -1 }))
            }
            Key::Char(']') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: 1 }))
            }
            _ => None,
        }
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
mod tests {
    use super::*;
    use crate::app::render::LibraryListRenderCtx;
    use crate::app::tests::make_item;
    use ratatui::backend::TestBackend;
    use ratatui::layout::{Position, Rect};
    use ratatui::Terminal;
    use tuirealm::component::Component;

    fn context(album: EmbyItem, overview: &str) -> MusicWideRenderCtx {
        let mut album = album;
        album.overview = overview.into();
        MusicWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![album.clone()], 0, 0),
            Some(album),
            "Artist".into(),
            vec![make_item("Artist", "MusicArtist")],
            0,
            vec![("Artist".into(), "2024".into(), "Album".into())],
            vec![0],
            true,
            None,
            false,
            false,
        )
    }

    #[test]
    fn content_uses_square_artwork_for_an_album() {
        let mut owner = MusicContent::new();
        owner.set_content(context(make_item("Album", "MusicAlbum"), "overview"));
        let content = owner.content();
        assert_eq!(
            content.hero.unwrap().facts.artwork.shape,
            super::super::library_panel::ArtworkShape::Square
        );
    }

    #[test]
    fn content_omits_an_absent_overview() {
        let mut owner = MusicContent::new();
        owner.set_content(context(make_item("Album", "MusicAlbum"), ""));
        let content = owner.content();
        assert!(content.hero.unwrap().overview.is_none());
    }

    #[test]
    fn content_exposes_tracks_as_the_workspace() {
        let mut owner = MusicContent::new();
        let mut ctx = context(make_item("Album", "MusicAlbum"), "overview");
        ctx.album_tracks = Some(vec![make_item("Track", "Audio")]);
        owner.set_content(ctx);
        let has_workspace = owner
            .content()
            .hero
            .as_ref()
            .is_some_and(|hero| hero.workspace.is_some());
        assert!(has_workspace);
        assert_eq!(owner.track_list.rows().len(), 1);
    }

    #[test]
    fn hero_double_click_activates_the_selected_track() {
        let album = make_item("Album", "MusicAlbum");
        let track = make_item("Track", "Audio");
        let mut owner = MusicContent::new();
        let mut ctx = context(album.clone(), "overview");
        ctx.album_tracks = Some(vec![track.clone()]);
        owner.set_content(ctx);

        let area = Rect::new(0, 0, 30, 1);
        owner.track_list.wide_mut().set_geometry(area, area);
        let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
        terminal
            .draw(|frame| owner.track_list.wide_mut().view(frame, area))
            .unwrap();

        let message = owner.on_slot_event(LibrarySlotEvent::HeroPane(RowLocalInput::DoubleClick(
            Position { x: 0, y: 0 },
        )));
        match message {
            Some(Msg::Shell(ShellRequest::MusicTrackActivate {
                album_id,
                track: activated,
            })) => {
                assert_eq!(album_id, album.id);
                assert_eq!(activated.id, track.id);
            }
            other => panic!("expected track activation, got {other:?}"),
        }
    }

    #[test]
    fn album_wheel_emits_cursor_for_owner_target_and_noop_for_unknown_target() {
        let mut owner = MusicContent::new();
        owner.set_content(context(make_item("Album", "MusicAlbum"), "overview"));

        let area = Rect::new(0, 0, 30, 1);
        owner.carrier.inline_mut().set_geometry(area, area);
        let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
        terminal
            .draw(|frame| owner.carrier.inline_mut().view(frame, area))
            .unwrap();

        let event = LibrarySlotEvent::List(RowLocalInput::Wheel {
            at: Position { x: 0, y: 0 },
            delta: 1,
        });
        assert!(matches!(
            owner.on_slot_event(event),
            Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                target: 0,
                kind: AlbumCursorKind::Move,
            }))
        ));

        owner.context.album_targets.clear();
        assert_eq!(owner.on_slot_event(event), None);
    }

    #[test]
    fn wide_album_metadata_removes_artist_and_year_prefix() {
        // The old `wide_album_metadata` characterization (rehomed here by task
        // 9.2): a tagged album whose display name still carries the
        // `Artist (Year) Title` folder prefix must present the bare title and
        // the parsed release year, even though `derive_album_display_name`
        // leaves a tagged album's name untouched.
        let mut album = make_item("Bob Dylan (1970) New Morning", "MusicAlbum");
        album.artist = "Bob Dylan".into();
        album.production_year = 1970;

        assert_eq!(
            wide_album_metadata(&album, "Bob Dylan"),
            ("New Morning".to_string(), 1970)
        );
    }

    #[test]
    fn resolved_hero_data_uses_parsed_title_year_and_cached_artist() {
        // The `album_artist_cache` fallback names the artist; the folder-name
        // parse supplies the title/year the Wide hero presents.
        let mut owner = MusicContent::new();
        let mut album = make_item("Folder Artist (2024) First Album", "MusicAlbum");
        album.artist.clear();
        album.production_year = 0;
        let mut ctx = context(album, "overview");
        ctx.album_info = vec![("Folder Artist".into(), "2024".into(), "First Album".into())];
        owner.set_content(ctx);

        let data = owner.hero_data().expect("hero data");
        assert_eq!(data.facts.title, "First Album");
        assert_eq!(data.facts.meta_rows, vec!["Folder Artist", "2024"]);
    }
}
