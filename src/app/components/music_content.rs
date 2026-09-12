//! Grouped Music's embedded Library panel content owner (task 9.1).
//!
//! `MusicContent` owns the shell-projected album/track snapshot, the shared
//! album and track list controls, local track focus, and the Inline Search
//! session.  It is a plain [`LibraryContentOwner`]; the legacy
//! `MusicWorkspaceComponent` temporarily borrows this owner for its existing
//! painters until the later Music panel slices move painting and registration.

use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use tuirealm::event::KeyEvent;

use super::inline_search::InlineSearch;
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

    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        self.context.selected_album.as_ref().map(hero_content_emby)
    }

    pub(in crate::app) fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    pub(in crate::app) fn panel_content(&mut self) -> LibraryPanelContent<'_> {
        // Copy the selected snapshot and focus bit before borrowing either
        // list mutably for the returned slots.
        let album = self.selected_item();
        let focused = self.context.focused;
        let track_focused = self.track_focused;
        let hero = album.map(|album| {
            let mut data = hero_content_emby(&album);
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

impl LibraryContentOwner for MusicContent {
    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.panel_content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.on_slot_event(event)
    }

    fn on_key(&mut self, _key: &KeyEvent) -> Option<Msg> {
        // Keyboard translation remains on MusicWorkspaceComponent until task
        // 9.4 removes that mounted component. The session itself is retained
        // here and is available to the panel once it becomes the boundary.
        None
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
}
