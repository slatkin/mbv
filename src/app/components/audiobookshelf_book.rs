//! Interactive Component for the Audiobookshelf book library.
//!
//! The embedded [`BookContent`] owner is authoritative for the book and
//! chapter cursors, scroll, selected targets, chapter-pane focus, and the
//! surname-bucket selection. Painting runs through the shared Library panel
//! skeleton (task 10.2): the component builds `BookContent::panel_content()`
//! and hands it to `render_wide_skeleton`/`render_narrow_skeleton`, painting
//! nothing itself. The component keeps only the panel's retained geometry,
//! breakpoint presentation facts, and typed shell intents until the later
//! Books panel slice (10.3) moves registration into the mounted
//! `LibraryPanel`. Row-local input reaches the owner through the common
//! delegation seam (design.md D3/D5).

use std::ops::{Deref, DerefMut};

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::book_content::BookContent;
use super::library_panel::{
    render_narrow_skeleton, render_wide_skeleton, PanelHeroImagePaint, SkeletonHits,
};
use super::media_list::{Presentation, RowLocalInput};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{AudiobookshelfBookIntent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::wide_hero_fits;
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;

/// The component's own retained paint geometry (task 10.2): the panel
/// skeleton's role rects the shell still reads (page stride, context-menu
/// anchor, pill hit resolution) until the component registers under the
/// mounted `LibraryPanel` (task 10.3).
#[derive(Default)]
pub(in crate::app) struct AudiobookshelfBookGeometry {
    pub selector_tabs: Vec<(Rect, usize)>,
    /// The painted list box's row-flow rect: the Wide browser pane, or the
    /// Narrow content area below the pill bar. This is the list geometry
    /// `page_size()` derives its real stride from (2.1j).
    pub left_area: Rect,
    /// The painted hero rect (Wide right pane, or the Narrow inline detail
    /// block), when one painted.
    pub hero_area: Option<Rect>,
    /// The selected-item rect the panel painted this frame (the context-menu
    /// anchor's painted truth).
    pub selected_item_rect: Option<Rect>,
}

pub struct AudiobookshelfBookComponent {
    /// The embedded content owner is the sole store for Books' content,
    /// selection, chapter focus and surname-bucket selection.
    content: BookContent,
    /// Session-only Wide hero list-pane width override (per-draw shell push,
    /// `None` = default ratio). Forwarded into the shared split; never stored
    /// clamped.
    list_pane_width: Option<u16>,
    geometry: AudiobookshelfBookGeometry,
    /// Whether the last rendered presentation actually exposes chapter focus.
    /// Narrow layouts may retain chapter state across a projection, so input
    /// must follow the rendered wide/chapter geometry rather than that state.
    chapters_visible: bool,
    panel_image_paint: Option<PanelHeroImagePaint>,
    /// The presentation the last `view` painted; `None` before the first paint.
    wide: bool,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
}

impl Deref for AudiobookshelfBookComponent {
    type Target = BookContent;

    fn deref(&self) -> &Self::Target {
        &self.content
    }
}

impl DerefMut for AudiobookshelfBookComponent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.content
    }
}

impl AudiobookshelfBookComponent {
    pub fn new() -> Self {
        Self {
            content: BookContent::new(),
            list_pane_width: None,
            geometry: AudiobookshelfBookGeometry::default(),
            chapters_visible: false,
            panel_image_paint: None,
            wide: false,
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// The presentation the painted breakpoint currently selects (design.md
    /// D2).
    fn active_presentation(&self) -> Presentation {
        if self.wide {
            Presentation::Wide
        } else {
            Presentation::Inline
        }
    }

    /// Move the shared book owner into the active presentation when they
    /// diverge. A responsive change reads the same owner and preserves only the
    /// outgoing selected-row viewport offset (design.md D1); no cursor, scroll,
    /// or selection is copied between presentations.
    fn ensure_carrier(&mut self) {
        let viewport_height = self.geometry.left_area.height.max(1) as usize;
        let presentation = self.active_presentation();
        self.carrier.set_presentation(presentation, viewport_height);
    }

    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether the parent-owned chapter pane currently has focus (design.md
    /// D5). Forwards to the embedded [`BookContent`] owner; kept as a thin
    /// wrapper (rather than relying on `Deref`) so `Type::method` function-
    /// pointer callers keep resolving against the component.
    pub(in crate::app) fn chapter_focused(&self) -> bool {
        self.content.chapter_focused()
    }

    #[cfg(test)]
    pub(crate) fn selected_bucket(&self) -> usize {
        self.content.selected_bucket()
    }

    #[cfg(test)]
    pub(crate) fn selected_book_id(&self) -> Option<&str> {
        self.content.selected_book_id()
    }

    /// Records the session-only Wide hero list-pane width override for the
    /// next `view()`. Pushed each frame by
    /// `render_audiobookshelf_book_component`; it is a layout fact, not
    /// content, so it never enters the event-scoped `set_content` projection.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.list_pane_width = list_pane_width;
    }

    /// The embedded owner keeps the whole book/chapter snapshot, cursor,
    /// scroll and selected targets (design.md D1/D2/D5); the component
    /// forwards the shell's projection unchanged.
    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBookBrowseState,
        images_enabled: bool,
    ) {
        self.content.set_content(snapshot, images_enabled);
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

    /// The geometry the component computed during its last `view`, exposed so
    /// the shell can anchor the context menu (task 5.3d.13, render ownership).
    pub(in crate::app) fn geometry(&self) -> &AudiobookshelfBookGeometry {
        &self.geometry
    }

    #[cfg(test)]
    pub(crate) fn selected_row_offset_for_test(&self) -> Option<usize> {
        let height = self.geometry.left_area.height.max(1) as usize;
        if self.carrier.active() == Presentation::Wide {
            self.carrier.wide().selected_row_offset(height)
        } else {
            self.carrier.inline().selected_row_offset(height)
        }
    }

    #[cfg(test)]
    pub(crate) fn book_row_rect_for_test(&self, row: usize) -> Option<Rect> {
        let content = if self.carrier.active() == Presentation::Wide {
            self.carrier.wide().current_content_rect()?
        } else {
            self.carrier.inline().current_content_rect()?
        };
        Some(Rect {
            y: content.y + row as u16,
            height: 1,
            ..content
        })
    }

    /// Test-only: the chapter owner's retained current-frame content rect, the
    /// shared geometry the chapter hit resolution uses (design.md D6).
    #[cfg(test)]
    pub(crate) fn chapter_content_rect_for_test(&self) -> Option<Rect> {
        self.chapter_list.current_content_rect()
    }

    /// The page stride from the component's own painted geometry
    /// (split-audiobookshelf-cursor-ownership D1): the list/content area's
    /// height minus its header line — the same value `App::lib_page_size()`
    /// derived from the projected `left_area`, now sourced locally so the
    /// shell applies no competing stride.
    fn page_size(&self) -> usize {
        (self.geometry.left_area.height as usize)
            .saturating_sub(1)
            .max(1)
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !self.focused {
            return None;
        }

        let chapters_focused = self.chapters_visible && self.chapter_focused;
        match key.code {
            Key::Char('[') if key.modifiers.is_empty() => {
                self.cycle_bucket(-1);
                self.bucket_request()
            }
            Key::Char(']') if key.modifiers.is_empty() => {
                self.cycle_bucket(1);
                self.bucket_request()
            }
            Key::Up | Key::Char('k') if chapters_focused => {
                self.move_chapter(-1);
                self.chapter_focus_request()
            }
            Key::Down | Key::Char('j') if chapters_focused => {
                self.move_chapter(1);
                self.chapter_focus_request()
            }
            Key::Right if chapters_focused => {
                self.clear_chapter_focus();
                self.chapter_focus_request()
            }
            Key::Left if self.chapters_visible && !self.chapter_focused => {
                self.enter_chapter_focus();
                self.chapter_focus_request()
            }
            Key::Up | Key::Char('k') => self.move_book(RowLocalInput::Move(-1)),
            Key::Down | Key::Char('j') => self.move_book(RowLocalInput::Move(1)),
            Key::PageUp if !chapters_focused => {
                let rows = self.page_size() as i64;
                self.move_book(RowLocalInput::Move(-rows))
            }
            Key::PageDown if !chapters_focused => {
                let rows = self.page_size() as i64;
                self.move_book(RowLocalInput::Move(rows))
            }
            Key::Home if !chapters_focused => {
                self.select_bucket_edge(false);
                self.book_request()
            }
            Key::End if !chapters_focused => {
                self.select_bucket_edge(true);
                self.book_request()
            }
            Key::Esc | Key::Backspace if chapters_focused => {
                self.clear_chapter_focus();
                self.chapter_focus_request()
            }
            Key::Char(' ') if chapters_focused => {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                )))
            }
            Key::Enter if chapters_focused => {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                )))
            }
            Key::Char(' ') => Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                AudiobookshelfBookIntent::Play,
            ))),
            Key::Enter => Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                AudiobookshelfBookIntent::Activate,
            ))),
            Key::Char('a')
                if !chapters_focused && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::Enqueue,
                )))
            }
            _ => None,
        }
    }

    /// Handle pointer input using retained geometry from the active book owner
    /// and the chapter owner (design.md D6).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // The book surface does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click(at) => {
                let picked_bucket = self
                    .geometry
                    .selector_tabs
                    .iter()
                    .find(|(rect, _)| rect.contains(at))
                    .map(|(_, bucket)| *bucket);
                if let Some(bucket) = picked_bucket {
                    self.select_bucket(bucket);
                    return self.bucket_request();
                }
                if let Some(target) = self.carrier.resolve_current_point(at).cloned() {
                    self.carrier
                        .delegate(RowLocalInput::Click(at), Some(target));
                    self.sync_book_from_owner();
                    return self.book_request();
                }
                if self.chapters_visible {
                    if let Some(target) = self.chapter_list.resolve_current_point(at).copied() {
                        self.enter_chapter_focus();
                        self.chapter_list
                            .delegate(RowLocalInput::Click(at), Some(target));
                        return self.chapter_focus_request();
                    }
                }
                None
            }
            MouseGesture::DoubleClick(at) => {
                self.carrier
                    .resolve_current_point(at)
                    .cloned()
                    .map(|target| {
                        self.carrier
                            .delegate(RowLocalInput::Click(at), Some(target));
                        self.sync_book_from_owner();
                        Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                            AudiobookshelfBookIntent::Activate,
                        ))
                    })
            }
            MouseGesture::Scroll { at, delta } => {
                let chapter_focus = self.chapters_visible && self.chapter_focused;
                if chapter_focus {
                    if !self.chapter_list.claims_current_point(at) {
                        return None;
                    }
                    self.move_chapter(delta);
                    self.chapter_focus_request()
                } else {
                    if !self.carrier.claims_current_point(at) {
                        return None;
                    }
                    self.move_book(RowLocalInput::Move(delta))
                }
            }
            _ => None,
        }
    }

    /// Test seam: reset the private gesture recognizer so a synchronous test
    /// loop can drive successive wheel/click events without the
    /// throttle/double-click window collapsing them.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }
}

impl Default for AudiobookshelfBookComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AudiobookshelfBookComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Chapter focus belongs only to the rendered wide Wide hero
        // presentation. Clear it before painting a narrow frame so a
        // wide→narrow resize cannot leave keyboard input targeting a hidden
        // chapter pane.
        self.wide = wide_hero_fits(area);
        self.ensure_carrier();
        self.chapters_visible = self.wide && !self.content.chapter_list.is_empty();
        if !self.chapters_visible {
            self.content.chapter_focused = false;
        }
        // The list pane loses its focused appearance while the chapter
        // Workspace holds focus (design D6).
        let browser_focused = self.content.focused && !self.content.chapter_focused;
        self.geometry = AudiobookshelfBookGeometry::default();
        self.panel_image_paint = None;
        let mut panel_content = self.content.panel_content();
        let mut hits = SkeletonHits::default();
        if self.wide {
            if let Some(geometry) = render_wide_skeleton(
                frame,
                area,
                &mut panel_content,
                browser_focused,
                self.list_pane_width,
                &mut hits,
            ) {
                self.panel_image_paint = geometry.hero_image.clone();
                self.geometry.left_area = geometry.list_area;
                self.geometry.hero_area = Some(geometry.hero_area);
                self.geometry.selected_item_rect = geometry.selected;
                self.geometry.selector_tabs = hits.selector.regions().to_vec();
            }
        } else {
            let geometry =
                render_narrow_skeleton(frame, area, &mut panel_content, browser_focused, &mut hits);
            self.panel_image_paint = geometry.inline_hero_image.clone();
            self.geometry.left_area = geometry.list_area;
            self.geometry.hero_area = geometry.inline_hero;
            self.geometry.selected_item_rect = geometry.selected;
            self.geometry.selector_tabs = hits.selector.regions().to_vec();
        }
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for AudiobookshelfBookComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => {
                self.ensure_carrier();
                self.handle_key(key)
            }
            Event::Mouse(mouse) => {
                self.ensure_carrier();
                self.handle_mouse(mouse)
            }
            _ => None,
        }
    }
}
