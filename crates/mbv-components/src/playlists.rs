use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use crate::media_list::{MediaListCarrier, MediaListOperation};
use mbv_emby_model::EmbyItem;
use mbv_render::components::media_list::{MediaKind, MediaListRow, MediaSemanticState};
use mbv_render::{PlaylistsRenderGeometry, PlaylistsViewState, render_playlists_content};
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{
    LeafKeyResult, Msg, MusicTreeAction, PlaylistsTarget, ShellRequest, TerminalObserverEvent,
};

#[derive(Debug)]
pub struct PlaylistsComponent {
    playlists: Vec<EmbyItem>,
    /// The saved-playlists rows over the shared list owner (design D6),
    /// one row per `EmbyItem`, targeted by the playlist `id`.
    list: MediaListCarrier<String>,
    loading: bool,
    open: Option<EmbyItem>,
    open_items: Vec<EmbyItem>,
    /// The open-playlist item rows over the shared list owner, targeted by
    /// `playlist_item_id`, not `id`: one `EmbyItem` can appear twice in a
    /// playlist, so the item id does not identify a row.
    open_list: MediaListCarrier<String>,
    open_loading: bool,
    loaded_id: Option<String>,
    panel_area: Option<Rect>,
    geometry: PlaylistsRenderGeometry,
    /// Irregular painted chrome (task 5.2, design.md D6): playlist rows and
    /// open-playlist item rows, repopulated in `view()` from the geometry
    /// the painter just produced. Tag = `(open, row index)`; open rows are
    /// pushed last so they win overlaps, matching the retired
    /// `PlaylistsRenderGeometry::hit_test` ordering.
    hit_rows: HitRegions<(bool, usize)>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    /// Owns the double-click window the hand-rolled `last_click` field used
    /// to keep.
    mouse_gestures: MouseGestureState,
}

/// Owned snapshot of playlist state, handed to the component whenever the
/// shell refreshes it. Grouped into one value because the fields always
/// travel together. Cursor and scroll stay component-local (AGENTS.md:
/// projection carries shell-owned content only).
#[derive(Debug)]
pub struct PlaylistsContent {
    pub playlists: Vec<EmbyItem>,
    pub loading: bool,
    pub open: Option<EmbyItem>,
    pub open_items: Vec<EmbyItem>,
    pub open_loading: bool,
    pub loaded_id: Option<String>,
}

/// One shared-owner row per playlist: the flow the row pointer indexes.
fn playlist_rows(playlists: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    playlists
        .iter()
        .map(|playlist| MediaListRow::Item {
            target: playlist.id.clone(),
            primary: playlist.name.clone(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        })
        .collect()
}

/// One shared-owner row per open-playlist item, targeted by
/// `playlist_item_id` (design D6).
fn item_rows(items: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    items
        .iter()
        .map(|item| MediaListRow::Item {
            target: item.playlist_item_id.clone(),
            primary: item.display_name(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        })
        .collect()
}

impl PlaylistsComponent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            playlists: Vec::new(),
            list: MediaListCarrier::new(),
            loading: false,
            open: None,
            open_items: Vec::new(),
            open_list: MediaListCarrier::new(),
            open_loading: false,
            loaded_id: None,
            panel_area: None,
            geometry: PlaylistsRenderGeometry::default(),
            hit_rows: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    pub fn set_content(&mut self, content: PlaylistsContent) {
        let PlaylistsContent {
            playlists,
            loading,
            open,
            open_items,
            open_loading,
            loaded_id,
        } = content;
        let playlists_changed = self.playlists != playlists;
        let open_playlist_changed =
            self.open.as_ref().map(|p| p.id.as_str()) != open.as_ref().map(|p| p.id.as_str());
        self.playlists = playlists;
        self.loading = loading;
        self.open = open;
        self.open_items = open_items;
        self.open_loading = open_loading;
        self.loaded_id = loaded_id;
        // Content replacement preserves selection by stable target
        // (shared-list-components); only a changed playlist vec re-publishes
        // the rows, so an identical refresh holds the selected playlist.
        if playlists_changed {
            self.list.set_content(playlist_rows(&self.playlists));
        }
        // The open item list always re-publishes its rows (an item refresh
        // can change rows under the same playlist id). Opening a different
        // playlist resets the presentation to the first row, matching the
        // reset the shell did before the shared owner (design D6).
        self.open_list.set_content(item_rows(&self.open_items));
        if open_playlist_changed {
            self.open_list.reset_presentation();
        }
    }

    pub fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.list.cursor()
    }

    /// The visible carrier (design D6): the open-playlist rows when a
    /// playlist is open, else the saved-playlist rows. Every key, click and
    /// wheel arm routes through it in place of paired `open.is_some()`
    /// branches.
    fn active_list_mut(&mut self) -> &mut MediaListCarrier<String> {
        if self.open.is_some() {
            &mut self.open_list
        } else {
            &mut self.list
        }
    }

    /// Today's painted page distance: the panel minus its title and hint
    /// rows.
    fn page(&self) -> i64 {
        i64::from(self.geometry.panel_area.height.saturating_sub(4))
    }

    fn local_change() -> Option<Msg> {
        None
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.handle_navigation(key) {
            return Self::local_change();
        }
        if let Some(msg) = self.handle_back(key) {
            return Some(msg);
        }
        if let Some(msg) = Self::handle_dismiss(key, self.open.is_none()) {
            return Some(msg);
        }
        if let Some(msg) = Self::handle_global_command(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_open(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_activation(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_rename(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_delete(key) {
            return Some(msg);
        }
        self.handle_refresh(key).or_else(Self::local_change)
    }

    fn handle_navigation(&mut self, key: &KeyEvent) -> bool {
        let operation = match key.code {
            Key::Up => MediaListOperation::Move(-1),
            Key::Down => MediaListOperation::Move(1),
            Key::PageUp => MediaListOperation::Move(-self.page()),
            Key::PageDown => MediaListOperation::Move(self.page()),
            Key::Home => MediaListOperation::First,
            Key::End => MediaListOperation::Last,
            _ => return false,
        };
        self.active_list_mut().delegate_operation(operation);
        true
    }

    fn handle_back(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Left | Key::Esc | Key::Backspace | Key::Function(4) if self.open.is_some() => {}
            _ => return None,
        }
        self.open = None;
        self.open_items.clear();
        Some(Msg::Shell(Box::new(ShellRequest::PlaylistsBack)))
    }

    fn handle_dismiss(key: &KeyEvent, closed: bool) -> Option<Msg> {
        match key.code {
            Key::Esc | Key::Function(4) if closed => {
                Some(Msg::Shell(Box::new(ShellRequest::DismissPlaylists)))
            }
            _ => None,
        }
    }

    fn handle_global_command(key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Function(2) => Some(Msg::Shell(Box::new(ShellRequest::OpenSettings))),
            Key::Function(3) => Some(Msg::Shell(Box::new(ShellRequest::OpenSessions))),
            Key::Char('q') if key.modifiers.is_empty() => {
                Some(Msg::Shell(Box::new(ShellRequest::Quit)))
            }
            _ => None,
        }
    }

    fn handle_open(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Right if self.open.is_none() && self.list.cursor() < self.playlists.len() => Some(
                Msg::Shell(Box::new(ShellRequest::PlaylistsOpen(self.list.cursor()))),
            ),
            _ => None,
        }
    }

    fn handle_activation(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !key.modifiers.is_empty() {
            return None;
        }
        let action = match key.code {
            Key::Enter => MusicTreeAction::Play,
            Key::Char('s') => MusicTreeAction::Shuffle,
            Key::Char('a') => MusicTreeAction::Enqueue,
            _ => return None,
        };
        // Every row is selectable, so the carrier cursor equals the
        // projected-vec index the shell resolves.
        let index = self.active_list_mut().cursor();
        let target = if self.open.is_some() {
            PlaylistsTarget::Row(index)
        } else {
            PlaylistsTarget::Playlist(index)
        };
        Some(Msg::Shell(Box::new(ShellRequest::PlaylistsAction {
            target,
            action,
        })))
    }

    fn handle_rename(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.code != Key::Char('n') || !key.modifiers.is_empty() || self.open.is_some() {
            return None;
        }
        (self.list.cursor() < self.playlists.len()).then_some(Msg::Shell(Box::new(
            ShellRequest::PlaylistsRename(self.list.cursor()),
        )))
    }

    fn handle_delete(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.code != Key::Char('d') || !key.modifiers.is_empty() || self.open.is_some() {
            return None;
        }
        (self.list.cursor() < self.playlists.len()).then_some(Msg::Shell(Box::new(
            ShellRequest::PlaylistsDelete(self.list.cursor()),
        )))
    }

    fn handle_refresh(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.code != Key::Char('r') {
            return None;
        }
        if self.open.is_some() {
            self.open = None;
            self.open_items.clear();
        }
        Some(Msg::Shell(Box::new(ShellRequest::PlaylistsRefresh)))
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            // Until 5.3c: the wheel still moves the cursor one row through
            // the shared owner's `Move`, so the viewport follows.
            MouseGesture::Scroll { delta, .. } => {
                self.active_list_mut()
                    .delegate_operation(MediaListOperation::Move(delta.signum()));
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::RightClick(_) if self.open.is_some() => {
                self.open = None;
                self.open_items.clear();
                Some(Msg::Shell(Box::new(ShellRequest::PlaylistsBack)))
            }
            gesture @ (MouseGesture::Click { at, .. } | MouseGesture::DoubleClick(at)) => {
                if !self.geometry.panel_area.contains(at) {
                    return Some(Msg::Shell(Box::new(ShellRequest::DismissPlaylists)));
                }
                let &(open, index) = self.hit_rows.resolve(at)?;
                // The painted row index selects the row's stable target
                // (design D6): one carrier row per `EmbyItem`, in slice
                // order.
                let rows = if open {
                    self.open_list.rows()
                } else {
                    self.list.rows()
                };
                let Some(MediaListRow::Item { target, .. }) = rows.get(index) else {
                    return None;
                };
                let target = target.clone();
                let list = if open {
                    &mut self.open_list
                } else {
                    &mut self.list
                };
                list.delegate_operation(MediaListOperation::Select(target));
                let playlists_target = if open {
                    PlaylistsTarget::Row(index)
                } else {
                    PlaylistsTarget::Playlist(index)
                };
                matches!(gesture, MouseGesture::DoubleClick(_)).then_some(Msg::Shell(Box::new(
                    ShellRequest::PlaylistsAction {
                        target: playlists_target,
                        action: MusicTreeAction::Play,
                    },
                )))
            }
            _ => None,
        }
    }
}

impl Default for PlaylistsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PlaylistsComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let open = self.open.is_some();
        let playlists_cursor = self.list.cursor();
        let open_cursor = self.open_list.cursor();
        let open_list = &mut self.open_list;
        let list = &mut self.list;
        let mut resolve_offset = move |height: usize| -> usize {
            let carrier: &mut MediaListCarrier<String> =
                if open { &mut *open_list } else { &mut *list };
            carrier.clamp_viewport(height);
            carrier.scroll()
        };
        render_playlists_content(
            frame,
            area,
            PlaylistsViewState {
                panel_area: self.panel_area,
                playlists: &self.playlists,
                playlists_cursor,
                playlists_loading: self.loading,
                playlists_open: self.open.as_ref(),
                open_items: &self.open_items,
                open_cursor,
                open_loading: self.open_loading,
                loaded_id: self.loaded_id.as_deref(),
                resolve_offset: &mut resolve_offset,
                geometry: &mut self.geometry,
            },
        );
        // Adopt the rows the painter just produced into the irregular-chrome
        // registry (task 5.2, design.md D6). Open rows are pushed last so
        // they win overlaps, matching the old `hit_test` ordering.
        self.hit_rows.clear();
        for (rect, index) in &self.geometry.playlist_rows {
            self.hit_rows.push(*rect, (false, *index));
        }
        for (rect, index) in &self.geometry.open_rows {
            self.hit_rows.push(*rect, (true, *index));
        }
    }

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
        None
    }
    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}
    fn state(&self) -> State {
        State::None
    }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for PlaylistsComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))).into_option(),
                None if matches!(
                    key.code,
                    Key::Up
                        | Key::Down
                        | Key::PageUp
                        | Key::PageDown
                        | Key::Home
                        | Key::End
                        | Key::Left
                        | Key::Right
                        | Key::Esc
                        | Key::Backspace
                        | Key::Enter
                        | Key::Char(_)
                ) =>
                {
                    LeafKeyResult::Consumed(None).into_option()
                }
                None => LeafKeyResult::Unhandled.into_option(),
            },
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: Key) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }
    }

    fn item(id: &str, name: &str, playlist_item_id: &str) -> EmbyItem {
        let mut item = mbv_emby_model::test_support::make_item(name, "Movie");
        item.id = id.into();
        item.playlist_item_id = playlist_item_id.into();
        item
    }

    fn content(
        playlists: Vec<EmbyItem>,
        open: Option<EmbyItem>,
        open_items: Vec<EmbyItem>,
    ) -> PlaylistsContent {
        PlaylistsContent {
            playlists,
            loading: false,
            open,
            open_items,
            open_loading: false,
            loaded_id: None,
        }
    }

    #[test]
    fn closed_playlist_keys_open_and_clamp_navigation() {
        let mut component = PlaylistsComponent::new();
        component.set_content(content(
            vec![item("p1", "First", ""), item("p2", "Second", "")],
            None,
            Vec::new(),
        ));

        component.handle_key(&key(Key::End));
        assert_eq!(component.list.cursor(), 1);
        assert_eq!(
            component.handle_key(&key(Key::Right)),
            Some(Msg::Shell(Box::new(ShellRequest::PlaylistsOpen(1))))
        );
    }

    /// Two saved playlists with the first one open holding three items; the
    /// visible list's cursor sits on its last row (closed: row 1, open: row
    /// 2), matching the cursor the action tests act on.
    fn component(open: bool) -> PlaylistsComponent {
        let mut component = PlaylistsComponent::new();
        let playlists = vec![item("p1", "First", ""), item("p2", "Second", "")];
        let open_items = vec![
            item("a", "Film", "row-a"),
            item("b", "Show", "row-b"),
            item("c", "Clip", "row-c"),
        ];
        component.set_content(content(
            playlists.clone(),
            open.then(|| playlists[0].clone()),
            if open {
                open_items.clone()
            } else {
                Vec::default()
            },
        ));
        component.handle_key(&key(Key::End));
        component
    }

    // Contract: Enter/s/a on a row send Play/Shuffle/Enqueue for the current
    // view's cursor (playlist-management spec, D1).
    #[rstest::rstest]
    #[case::list_enter(false, Key::Enter, PlaylistsTarget::Playlist(1), MusicTreeAction::Play)]
    #[case::list_shuffle(
        false,
        Key::Char('s'),
        PlaylistsTarget::Playlist(1),
        MusicTreeAction::Shuffle
    )]
    #[case::list_enqueue(
        false,
        Key::Char('a'),
        PlaylistsTarget::Playlist(1),
        MusicTreeAction::Enqueue
    )]
    #[case::open_enter(true, Key::Enter, PlaylistsTarget::Row(2), MusicTreeAction::Play)]
    #[case::open_shuffle(
        true,
        Key::Char('s'),
        PlaylistsTarget::Row(2),
        MusicTreeAction::Shuffle
    )]
    #[case::open_enqueue(
        true,
        Key::Char('a'),
        PlaylistsTarget::Row(2),
        MusicTreeAction::Enqueue
    )]
    fn action_keys_send_playlists_action(
        #[case] open: bool,
        #[case] code: Key,
        #[case] target: PlaylistsTarget,
        #[case] action: MusicTreeAction,
    ) {
        let mut component = component(open);

        assert_eq!(
            component.handle_key(&key(code)),
            Some(Msg::Shell(Box::new(ShellRequest::PlaylistsAction {
                target,
                action,
            })))
        );
    }

    #[test]
    fn open_playlist_back_clears_open_items() {
        let mut component = PlaylistsComponent::new();
        component.set_content(content(
            Vec::new(),
            Some(item("p1", "Playlist", "")),
            vec![item("a", "Film", "row-a")],
        ));

        assert_eq!(
            component.handle_key(&key(Key::Esc)),
            Some(Msg::Shell(Box::new(ShellRequest::PlaylistsBack)))
        );
        assert!(component.open.is_none());
        assert_eq!(component.open_items, [] as [mbv_emby_model::EmbyItem; 0]);
    }

    /// Contract: the overlay's own reset rule (design D6) — opening a
    /// different playlist starts its item list at the first row.
    #[test]
    fn opening_a_different_playlist_starts_its_item_list_at_the_first_row() {
        let mut component = PlaylistsComponent::new();
        component.set_content(content(
            Vec::new(),
            Some(item("p1", "First", "")),
            vec![
                item("a", "Film", "row-a"),
                item("b", "Show", "row-b"),
                item("c", "Clip", "row-c"),
            ],
        ));

        component.handle_key(&key(Key::Down));
        component.handle_key(&key(Key::Down));
        assert_eq!(component.open_list.cursor(), 2);
        component.set_content(content(
            Vec::new(),
            Some(item("p2", "Second", "")),
            vec![item("d", "Tale", "row-d"), item("e", "Fable", "row-e")],
        ));

        assert_eq!(component.open_list.cursor(), 0);
    }
}
