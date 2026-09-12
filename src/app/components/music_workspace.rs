//! Interactive Component for grouped Music's Wide workspace.
//!
//! The embedded [`MusicContent`] owner is authoritative for the album and
//! track cursors, scroll, selected targets, track focus, and Inline Search
//! session. The component keeps only legacy painter geometry, breakpoint
//! presentation facts, group hit geometry, and typed shell intents until the
//! later Music panel slices move painting and registration. Row-local input
//! reaches the owner through the common delegation seam (design.md D3/D5).

use std::ops::{Deref, DerefMut};

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::inline_search::{InlineSearch, InlineSearchHost, InlineSearchMouse};
use super::library_panel::{
    render_narrow_skeleton, render_wide_skeleton, PanelHeroImagePaint, SkeletonHits,
    WideSkeletonGeometry,
};
use super::media_list::{Presentation, RowLocalInput, RowLocalOutcome};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{AlbumCursorKind, Msg, ShellRequest, TerminalObserverEvent};
use super::music_content::MusicContent;
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::render::{wide_hero_fits, MusicWideRenderCtx};

pub struct MusicWorkspaceComponent {
    /// The embedded content owner is the sole store for Music's content,
    /// selection, track focus and Inline Search session. The component keeps
    /// only legacy painter geometry and transitional paint state until tasks
    /// 9.2–9.4 complete.
    pub(super) content: MusicContent,
    pub(super) album_columns: usize,
    pub(super) page_rows: usize,
    layout: LayoutMain,
    /// The selected item's hero geometry the last painted skeleton produced
    /// (Wide places it beside `layout.left_area`; Narrow places it inside
    /// the list as the selected parent's replacement).
    hero_area: Rect,
    /// Screen rect of the selected row/cell the last painted skeleton
    /// produced.
    selected_item_rect: Option<Rect>,
    inline_track_focus_enabled: bool,
    mouse_gestures: MouseGestureState,
    wide_browser_content_height: Option<usize>,
    /// Retained role geometry from the shared Wide skeleton. The mounted
    /// Music owner remains the event boundary until registration in task 9.4.
    wide_geometry: Option<WideSkeletonGeometry>,
    panel_image_paint: Option<PanelHeroImagePaint>,
    pill_regions: HitRegions<usize>,
    /// Session-only Wide hero list-pane width override (per-draw shell push,
    /// `None` = default ratio). Written onto the cloned render context in
    /// `view()` before the wide composer reads it; never stored clamped.
    list_pane_width: Option<u16>,
}

impl Deref for MusicWorkspaceComponent {
    type Target = MusicContent;

    fn deref(&self) -> &Self::Target {
        &self.content
    }
}

impl DerefMut for MusicWorkspaceComponent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.content
    }
}

impl MusicWorkspaceComponent {
    pub fn new() -> Self {
        Self {
            content: MusicContent::new(),
            album_columns: 1,
            page_rows: 1,
            layout: LayoutMain::default(),
            hero_area: Rect::default(),
            selected_item_rect: None,
            inline_track_focus_enabled: false,
            mouse_gestures: MouseGestureState::new(),
            wide_browser_content_height: None,
            wide_geometry: None,
            panel_image_paint: None,
            pill_regions: HitRegions::new(),
            list_pane_width: None,
        }
    }

    pub(super) fn active_is_wide(&self) -> bool {
        self.carrier.active() == Presentation::Wide
    }

    /// The one seam through which Grouped Music offers an already-normalized
    /// row-local key or pointer gesture to the shared album owner. The owner
    /// applies the local state transition and returns the closed
    /// provider-neutral outcome; Music translates external row intents into
    /// its typed Msgs (design.md D3).
    pub(super) fn delegate_row_local_input(
        &mut self,
        input: RowLocalInput,
        pointer_target: Option<String>,
    ) -> RowLocalOutcome<String> {
        self.carrier.delegate(input, pointer_target)
    }

    /// The shared album owner's current stable target, if any.
    pub(super) fn active_target(&self) -> Option<String> {
        self.carrier.selected_target().cloned()
    }

    pub(super) fn select_active_target(&mut self, target: &str) {
        self.carrier.select_target(&target.to_string());
    }

    fn set_active_scroll(&mut self, scroll: usize) {
        // Adjudicated discrete-boundary re-anchor (design.md D5): `re_anchor`
        // has already selected the shell's stable album target, so this one
        // seeded resting scroll is sanctioned rather than delegated.
        self.carrier.set_scroll(scroll);
    }

    pub(super) fn selected_album_index(&self) -> usize {
        self.active_target()
            .and_then(|target| {
                self.context
                    .album_targets
                    .iter()
                    .position(|candidate| candidate == &target)
            })
            .unwrap_or(0)
    }

    /// Test-only: drive framework focus the way `Component::attr` does when
    /// TuiRealm delivers `Attribute::Focus`.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.context.focused = focused;
    }

    pub(in crate::app) fn set_inline_track_focus_enabled(&mut self, enabled: bool) {
        self.inline_track_focus_enabled = enabled;
        if !enabled {
            self.track_focused = false;
        }
    }

    /// Records the session-only Wide hero list-pane width override for the
    /// next `view()`. Pushed each frame by
    /// `render_music_workspace_component`; it is a layout fact, not content,
    /// so it never enters the event-scoped `set_content` projection.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.list_pane_width = list_pane_width;
    }

    pub(in crate::app) fn set_content(&mut self, context: MusicWideRenderCtx) {
        self.content.set_content(context);
    }

    /// Shell-driven re-anchor of the active album control at a navigation
    /// event: group switch, recursive-album activation, saved-position
    /// restore, or the first projection after mount. Unlike a content push
    /// this adopts the shell's value unconditionally -- the outcome does not
    /// depend on whether the user moved the cursor since the last push.
    pub(in crate::app) fn re_anchor(&mut self, cursor: usize, scroll: usize) {
        let cursor = cursor.min(self.context.list.item_count().saturating_sub(1));
        if let Some(target) = self
            .context
            .list
            .items
            .get(cursor)
            .map(|_| self.context.album_targets[cursor].as_str().to_owned())
        {
            self.select_active_target(&target);
            self.set_active_scroll(scroll);
        }
    }
    pub(in crate::app) fn set_album_columns(&mut self, columns: usize) {
        self.album_columns = columns.max(1);
    }
    pub(in crate::app) fn set_page_rows(&mut self, rows: usize) {
        self.page_rows = rows.max(1);
    }

    pub(in crate::app) fn album_cursor(&self) -> usize {
        self.selected_album_index()
    }

    pub(in crate::app) fn album_scroll(&self) -> usize {
        self.carrier.scroll()
    }

    pub(in crate::app) fn painted_album_cursor_and_order(&self) -> (usize, &[usize]) {
        (self.selected_album_index(), &self.context.album_order)
    }

    /// The album item under the component's own album cursor, cloned out of
    /// the cached render context. Mirrors the TV owner's `selected_item()`
    /// for outcome 3 readers (R16/R18): the shell supplies this instead of
    /// reading `BrowseLevel.cursor` for selected-album construction.
    /// First-mount fallback to App-derived item when the component is
    /// freshly mounted (before the first content push from the shell).
    pub(in crate::app) fn selected_item(&self) -> Option<mbv_core::api::EmbyItem> {
        self.context
            .list
            .items
            .get(self.selected_album_index())
            .cloned()
    }

    /// Whether the parent-owned track pane currently has focus (design.md
    /// D5). Independent of the track owner's selected row.
    pub(in crate::app) fn track_focused(&self) -> bool {
        self.track_focused
    }

    /// Whether inline track focus can be entered right now: wide mode
    /// (`inline_track_focus_enabled`) with the selected album's tracks
    /// cached. Narrow mode keeps the track pane unfocused by construction.
    pub(super) fn can_enter_track_focus(&self) -> bool {
        self.inline_track_focus_enabled
            && self.context.focused
            && self
                .context
                .album_tracks
                .as_ref()
                .is_some_and(|tracks| !tracks.is_empty())
    }

    /// Shell-driven entry into inline track focus (recursive album
    /// activation): enters only when the feature is enabled and the selected
    /// album's tracks are cached; a no-op in narrow mode. The discrete entry
    /// boundary parks the track owner at its first row (design.md D5).
    pub(in crate::app) fn enter_track_focus(&mut self) {
        if self.can_enter_track_focus() {
            self.track_focused = true;
            self.track_list.select_first();
        }
    }

    /// Shell-driven clear of inline track focus (position restore): the
    /// deleted track-focus-clear rehome. This is a focus-only boundary, so the
    /// track owner keeps its selected row and scroll (design.md D5); the later
    /// entry boundary (`enter_track_focus`) re-parks the selection explicitly.
    pub(in crate::app) fn clear_track_focus(&mut self) {
        self.track_focused = false;
    }

    /// Handle a mouse event against the wide workspace's painted geometry.
    ///
    /// Recognition comes from the private `MouseGestureState` (ADR 0024,
    /// design.md D3). Row identities come only from the embedded controls'
    /// completed current-frame retained results (design.md D6); album pills
    /// remain the parent's irregular chrome.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Inline Search gets first refusal while active (design.md D6): it
        // is painted over the same area the ordinary album rail/rows would
        // occupy, so they never mutate for points there.
        if self.inline_search.is_active() {
            return match self.inline_search.handle_mouse(mouse) {
                Some(InlineSearchMouse::ContextMenu) => self
                    .inline_search
                    .selected_item()
                    .map(|item| Msg::Shell(ShellRequest::EmbyLibraryContextMenu { item })),
                Some(InlineSearchMouse::Consumed) => {
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
                None => None,
            };
        }
        // Music does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        let wide = self.active_is_wide();
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                if wide && self.track_list.claims_current_point(at) {
                    // The track owner is local to this workspace; the shell
                    // never recomputes a wheel step. Track-pane focus is not
                    // a selection and does not move here.
                    self.track_list
                        .delegate(RowLocalInput::Wheel { at, delta }, None);
                    return None;
                }
                if !self.carrier.claims_current_point(at) {
                    return None;
                }
                self.delegate_row_local_input(RowLocalInput::Wheel { at, delta }, None);
                self.album_cursor_msg(AlbumCursorKind::Move)
            }
            // Wide album rail and provider-owned track table both resolve from
            // their own retained current-frame geometry. A double click
            // activates the pointed row; a single click only focuses it.
            MouseGesture::Click(at) if wide => {
                if let Some(msg) = self.claim_group_pill(at) {
                    return Some(msg);
                }
                if let Some(target) = self.track_list.resolve_current_point(at).cloned() {
                    self.track_focused = true;
                    self.track_list
                        .delegate(RowLocalInput::Click(at), Some(target));
                    return None;
                }
                let target = self.carrier.resolve_current_point(at)?.clone();
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                self.album_cursor_msg(AlbumCursorKind::Move)
            }
            MouseGesture::DoubleClick(at) if wide => {
                if let Some(msg) = self.claim_group_pill(at) {
                    return Some(msg);
                }
                if let Some(target) = self.track_list.resolve_current_point(at).cloned() {
                    self.track_focused = true;
                    self.track_list
                        .delegate(RowLocalInput::Click(at), Some(target));
                    return self.music_track_activate_msg();
                }
                let target = self.carrier.resolve_current_point(at)?.clone();
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                Some(Msg::Shell(ShellRequest::MusicAlbumActivate {
                    item: self.selected_item()?,
                }))
            }
            MouseGesture::RightClick(at) if wide => {
                if let Some(target) = self.track_list.resolve_current_point(at).cloned() {
                    self.track_focused = true;
                    self.track_list
                        .delegate(RowLocalInput::Click(at), Some(target));
                    return self.music_track_context_msg((at.x, at.y));
                }
                let target = self.carrier.resolve_current_point(at)?.clone();
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                Some(Msg::Shell(ShellRequest::MusicAlbumContextMenu {
                    item: self.selected_item()?,
                    anchor: (at.x, at.y),
                }))
            }
            // Narrow: group pills, then album rows.
            MouseGesture::Click(at) => {
                if let Some(msg) = self.claim_group_pill(at) {
                    return Some(msg);
                }
                let target = self.carrier.resolve_current_point(at)?.clone();
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                self.album_cursor_msg(AlbumCursorKind::Move)
            }
            MouseGesture::DoubleClick(at) => {
                if let Some(msg) = self.claim_group_pill(at) {
                    return Some(msg);
                }
                let target = self.carrier.resolve_current_point(at)?.clone();
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                Some(Msg::Shell(ShellRequest::MusicAlbumActivate {
                    item: self.selected_item()?,
                }))
            }
            MouseGesture::RightClick(at) if !wide => {
                let target = self.carrier.resolve_current_point(at)?.clone();
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                Some(Msg::Shell(ShellRequest::MusicAlbumContextMenu {
                    item: self.selected_item()?,
                    anchor: (mouse.column, mouse.row),
                }))
            }
            _ => None,
        }
    }

    /// The pushed-context item index for an album stable target, or `None`
    /// when the target is not part of the pushed catalog.
    fn album_index_for_target(&self, target: &str) -> Option<usize> {
        self.context.album_targets.iter().position(|t| t == target)
    }

    /// The album-cursor request for the shared owner's current selection. The
    /// shell only mirrors this index into the App resting cursor; the owner
    /// stays authoritative (the `BrowserCursorIndex` shape).
    pub(super) fn album_cursor_msg(&self, kind: AlbumCursorKind) -> Option<Msg> {
        let target = self.active_target()?;
        let index = self.album_index_for_target(&target)?;
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: index,
            kind,
        }))
    }

    /// The album item under the shared owner's current selection, resolved
    /// from the pushed catalog (never an App cursor re-read).
    pub(in crate::app) fn selected_album_item(&self) -> Option<mbv_core::api::EmbyItem> {
        let target = self.active_target()?;
        let index = self.album_index_for_target(&target)?;
        self.context.list.items.get(index).cloned()
    }

    /// The track item under the track owner's current selection.
    pub(in crate::app) fn selected_track_item(&self) -> Option<mbv_core::api::EmbyItem> {
        let target = self.track_list.selected_target()?;
        self.context
            .album_tracks
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|track| &track.id == target)
            .cloned()
    }

    /// The typed track-activation request carrying the owner-resolved album
    /// and track identities (design.md D4).
    pub(super) fn music_track_activate_msg(&self) -> Option<Msg> {
        let album = self.selected_album_item()?;
        let track = self.selected_track_item()?;
        Some(Msg::Shell(ShellRequest::MusicTrackActivate {
            album_id: album.id,
            track,
        }))
    }

    /// The typed track context-menu request carrying the owner-resolved track.
    pub(super) fn music_track_context_msg(&self, anchor: (u16, u16)) -> Option<Msg> {
        let track = self.selected_track_item()?;
        Some(Msg::Shell(ShellRequest::MusicTrackContextMenuAt {
            track,
            anchor,
        }))
    }

    /// If `at` lands on a group pill, emit the relative `MusicGroupSwitch` that
    /// reaches the clicked group (the only group-selection keyboard action is
    /// the relative `[`/`]`). A click on the current pill is a no-op.
    fn claim_group_pill(&self, at: Position) -> Option<Msg> {
        let &target = self.pill_regions.resolve(at)?;
        let delta = target as i64 - self.context.group_cursor as i64;
        (delta != 0).then_some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta }))
    }

    pub(in crate::app) fn take_panel_image_paint(&mut self) -> Option<PanelHeroImagePaint> {
        self.panel_image_paint.take()
    }

    pub(in crate::app) fn hero_data(&mut self) -> Option<super::library_panel::HeroContentData> {
        self.content.hero_data()
    }

    pub(in crate::app) fn set_hero_image(&mut self, state: super::library_panel::HeroImageState) {
        self.content.set_hero_image(state);
    }

    #[cfg(test)]
    pub(in crate::app) fn test_wide_geometry(&self) -> Option<&WideSkeletonGeometry> {
        self.wide_geometry.as_ref()
    }

    /// Geometry painted during the last view pass. The shell mirrors the
    /// interaction targets into App layout for legacy readers that still
    /// consume frame geometry.
    pub(in crate::app) fn layout(&self) -> &LayoutMain {
        &self.layout
    }

    /// The selected item's hero geometry the last painted skeleton produced,
    /// for the hero-boundary test path.
    #[cfg(test)]
    pub(in crate::app) fn test_hero_area(&self) -> Rect {
        self.hero_area
    }

    /// Screen rect of the selected row/cell the last painted skeleton
    /// produced, for the selected-row test path.
    #[cfg(test)]
    pub(in crate::app) fn test_selected_item_rect(&self) -> Option<Rect> {
        self.selected_item_rect
    }

    #[cfg(test)]
    pub(in crate::app) fn test_narrow_content_rect(&self) -> Rect {
        self.carrier
            .inline()
            .current_content_rect()
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(in crate::app) fn test_track_selected_row_rect(&self) -> Option<Rect> {
        self.track_list.current_selected_row_rect()
    }

    #[cfg(test)]
    pub(in crate::app) fn test_track_content_rect(&self) -> Option<Rect> {
        self.track_list.current_content_rect()
    }

    #[cfg(test)]
    pub(in crate::app) fn test_pill_regions(&self) -> &[(Rect, usize)] {
        self.pill_regions.regions()
    }
}

impl Default for MusicWorkspaceComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineSearchHost for MusicWorkspaceComponent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }

    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }
}

impl Component for MusicWorkspaceComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.layout = LayoutMain::default();
        self.hero_area = Rect::default();
        self.selected_item_rect = None;
        self.wide_geometry = None;
        self.panel_image_paint = None;
        let wide = wide_hero_fits(area);
        let target = if wide {
            Presentation::Wide
        } else {
            Presentation::Inline
        };

        // One shared album owner per logical flow (design.md D1): a breakpoint
        // change reconfigures the same owner and preserves only the outgoing
        // selected-row viewport offset. The receiving content height is the
        // height the retained offset is restored against: the Wide
        // presentation's own last painted browser content height (design.md
        // D3/D6), never a re-derived arrangement.
        let incoming_height = if wide {
            self.wide_browser_content_height
                .unwrap_or(area.height as usize)
        } else if self.context.groups.is_empty() {
            area.height as usize
        } else {
            crate::app::render::arrangements::wide_hero::pill_bar_areas(area)
                .content_area
                .height as usize
        };
        let selected_album_index = self.selected_album_index();
        let content = &mut self.content;
        content
            .carrier
            .set_presentation(target, incoming_height.max(1));

        // The active control owns the painted selection.
        content.context.list.set_cursor(selected_album_index);
        if wide {
            // Wide Music uses the shared Library panel skeleton while the
            // destination remains mounted as the event boundary until 9.4.
            let browser_focused = content.context.focused && !content.track_focused;
            let mut panel_content = content.panel_content();
            let mut hits = SkeletonHits::default();
            if let Some(geometry) = render_wide_skeleton(
                frame,
                area,
                &mut panel_content,
                browser_focused,
                self.list_pane_width,
                &mut hits,
            ) {
                self.panel_image_paint = geometry.hero_image.clone();
                self.layout.left_area = geometry.list_area;
                self.hero_area = geometry.hero_area;
                self.selected_item_rect = geometry.selected;
                self.wide_geometry = Some(geometry);
                self.pill_regions.clear();
                for (rect, target) in hits.selector.regions() {
                    self.pill_regions.push(*rect, *target);
                }
                if let Some(height) = self.wide_geometry.as_ref().map(|g| g.list_area.height) {
                    self.wide_browser_content_height = Some(height as usize);
                }
                return;
            }
        }
        if !wide {
            // Narrow Music uses the same panel skeleton and HeroContent as the
            // Wide path. Search is a ListSlot state, so the panel owns both
            // the search-box placement and ordinary row painting; no
            // Music-specific fallback painter may paint this surface.
            let focused = content.context.focused;
            let mut panel_content = content.panel_content();
            let mut hits = SkeletonHits::default();
            let geometry =
                render_narrow_skeleton(frame, area, &mut panel_content, focused, &mut hits);
            self.panel_image_paint = geometry.inline_hero_image.clone();
            self.layout.left_area = geometry.list_area;
            self.hero_area = geometry.inline_hero.unwrap_or_default();
            self.selected_item_rect = geometry.selected;
            self.pill_regions.clear();
            for (rect, target) in hits.selector.regions() {
                self.pill_regions.push(*rect, *target);
            }
        } else {
            // The Wide skeleton painted nothing this frame (no geometry):
            // nothing to adopt into the pill hit-registry.
            self.pill_regions.clear();
        }
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.context.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for MusicWorkspaceComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
