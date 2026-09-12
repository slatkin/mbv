//! Interactive Component for the Feeds destination.
//!
//! The shell supplies validated feed snapshots. This component keeps only the
//! presentation/geometry state the panel skeleton needs — which breakpoint was
//! painted, the private pointer gesture recognizer, the session list-pane width
//! override, and its own painted pill/row hit store — while the shell-projected
//! content, the feed-group/Watched filter selection, and the one shared
//! canonical `MediaList` owner of the grouped-entry projection live in the
//! embedded [`FeedsContent`] (task 7.1, design D2/D16 step 1). `view` paints the
//! Library panel's shared Wide/Narrow skeleton over `content()` (task 7.2);
//! Feeds owns no painter, and the hero image is the projected `HeroImageState`,
//! never a paint-time fetch (design D9). Registering the owner in the panel's
//! map (and deleting the mounted component) is task 7.3.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::feeds_content::{FeedsContent, FeedsOwnerPush};
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::{render_narrow_skeleton, render_wide_skeleton, SkeletonHits};
#[cfg(test)]
use super::media_list::MediaListRow;
use super::media_list::{RowIntent, RowLocalInput, RowLocalOutcome};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::render::wide_hero_fits;
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;

pub struct FeedsComponent {
    /// The embedded content owner (task 7.1): the shell-projected feed
    /// snapshot, the shared entry-list owner, and the feed-group/Watched
    /// filter selection.
    content: FeedsContent,
    /// Which presentation the last `view()` painted (Wide hero Wide vs inline
    /// Narrow). A breakpoint change moves the same shared owner between the
    /// persistent presentations; it also selects which owner `cursor()` reads.
    wide: bool,
    focused: bool,
    /// Session-only Wide hero list-pane width override (per-draw shell push,
    /// `None` = default ratio). Forwarded into the shared split; never stored
    /// clamped.
    list_pane_width: Option<u16>,
    /// The last painted frame's panel geometry, in `LayoutMain` form: the
    /// component's own pill/row hit store (`selector_tabs`) plus the list/hero
    /// rects its input path resolves against. The Library panel owns this
    /// skeleton once Feeds registers (task 7.3).
    layout: LayoutMain,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
}

/// The panel skeleton's painted pills as the `selector_tabs` store the
/// component's pointer path reads (design D8): feed-group pills keep their
/// indices, the List-controls pills are offset past them so one lookup
/// distinguishes the two rows, exactly as the legacy painter published them.
fn combined_selector_tabs(hits: &SkeletonHits, filter_base: usize) -> Vec<(Rect, usize)> {
    let mut tabs: Vec<(Rect, usize)> = hits.selector.regions().to_vec();
    tabs.extend(
        hits.controls
            .regions()
            .iter()
            .map(|(rect, id)| (*rect, filter_base + *id)),
    );
    tabs
}

impl FeedsComponent {
    pub fn new() -> Self {
        Self {
            content: FeedsContent::new(),
            wide: false,
            focused: false,
            list_pane_width: None,
            layout: LayoutMain::default(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Records the session-only Wide hero list-pane width override for the
    /// next `view()`. Pushed each frame by `render_feeds_component`; it is a
    /// layout fact, not content, so it never enters the event-scoped
    /// `set_content` projection.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.list_pane_width = list_pane_width;
    }

    pub(in crate::app) fn set_content(
        &mut self,
        subscriptions: &[FeedSubscription],
        entries: &[Vec<FeedEntry>],
        all_entries: &[FeedEntry],
        loading: bool,
    ) {
        self.content.set_content(FeedsOwnerPush {
            subscriptions: subscriptions.to_vec(),
            entries: entries.to_vec(),
            all_entries: all_entries.to_vec(),
            loading,
        });
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.content.cursor()
    }

    pub(in crate::app) fn watched_filter(&self) -> WatchedFilter {
        self.content.watched_filter()
    }

    pub(in crate::app) fn selected_group(&self) -> usize {
        self.content.selected_group()
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.content.scroll()
    }

    pub(in crate::app) fn visible_titles(&self) -> Vec<&str> {
        self.content.visible_titles()
    }

    /// Whether the active group/watched filter leaves any entry for the
    /// skeleton to project. The shell resolves the Wide split boundary's
    /// eligibility from this fact (the pre-panel painter's gate) instead of
    /// mirroring the owner-local selection; registering Feeds in the panel
    /// moves the boundary into the panel (task 7.3).
    pub(in crate::app) fn has_visible_entries(&self) -> bool {
        self.content.has_visible_entries()
    }

    pub(in crate::app) fn subscription_names(&self) -> Vec<&str> {
        self.content.subscription_names()
    }

    pub(in crate::app) fn layout(&self) -> &LayoutMain {
        &self.layout
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_rows(&self) -> &[MediaListRow<String>] {
        self.content.canonical_rows()
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_selectable_len(&self) -> usize {
        self.content.canonical_selectable_len()
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_selected_target(&self) -> Option<&String> {
        self.content.canonical_selected_target()
    }

    pub(in crate::app) fn group_count(&self) -> usize {
        self.content.group_count()
    }

    fn page_size(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
    }

    /// Keep the shared owner in the presentation the painted breakpoint
    /// currently selects before any row-local input touches it (design.md
    /// D1).
    fn ensure_carrier(&mut self) {
        self.content
            .ensure_presentation(self.wide, self.layout.left_area.height.max(1) as usize);
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !self.focused {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return None;
        }
        match key.code {
            Key::Char('r') => Some(Msg::Shell(ShellRequest::RefreshFeeds)),
            Key::Char('w') => {
                self.content.cycle_watched_filter();
                None
            }
            Key::Up | Key::Char('k') | Key::Left | Key::Char('h') => {
                self.content
                    .delegate_row_local_input(RowLocalInput::Move(-1), None);
                None
            }
            Key::Down | Key::Char('j') | Key::Right | Key::Char('l') => {
                self.content
                    .delegate_row_local_input(RowLocalInput::Move(1), None);
                None
            }
            Key::PageUp => {
                let page = self.page_size();
                self.content
                    .delegate_row_local_input(RowLocalInput::Move(-page), None);
                None
            }
            Key::PageDown => {
                let page = self.page_size();
                self.content
                    .delegate_row_local_input(RowLocalInput::Move(page), None);
                None
            }
            Key::Home => {
                self.content
                    .delegate_row_local_input(RowLocalInput::First, None);
                None
            }
            Key::End => {
                self.content
                    .delegate_row_local_input(RowLocalInput::Last, None);
                None
            }
            Key::Char('[') => {
                self.content.cycle_group(-1);
                None
            }
            Key::Char(']') => {
                self.content.cycle_group(1);
                None
            }
            Key::Enter => {
                match self
                    .content
                    .delegate_row_local_input(RowLocalInput::Activate, None)
                {
                    RowLocalOutcome::External(RowIntent::Activate(target)) => Some(Msg::Shell(
                        ShellRequest::FeedsPlay(self.content.entry_for_target(&target).cloned()),
                    )),
                    _ => Some(Msg::Shell(ShellRequest::FeedsPlay(None))),
                }
            }
            Key::Char('e') => {
                match self
                    .content
                    .delegate_row_local_input(RowLocalInput::Activate, None)
                {
                    RowLocalOutcome::External(RowIntent::Activate(target)) => Some(Msg::Shell(
                        ShellRequest::FeedsEnqueue(self.content.entry_for_target(&target).cloned()),
                    )),
                    _ => Some(Msg::Shell(ShellRequest::FeedsEnqueue(None))),
                }
            }
            _ => None,
        }
    }

    /// Handle a TuiRealm mouse event via the private `MouseGestureState`
    /// (ADR 0024, design.md D3). This component resolves only its own painted
    /// geometry — the selector/filter pill hit store and the list's
    /// point claim — and offers the normalized row-local input to the owner,
    /// which owns the typed target resolution and message translation
    /// (design.md D6). Feeds has no keyboard context-menu action (task 4.6),
    /// so right-click is ignored.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Feeds does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => self
                .content
                .on_slot_event(LibrarySlotEvent::List(RowLocalInput::Wheel { at, delta })),
            MouseGesture::Click(at) => {
                if let Some((_, target)) = self
                    .layout
                    .selector_tabs
                    .iter()
                    .find(|(rect, _)| rect.contains(at))
                {
                    let filter_base = self.content.group_count();
                    let event = if *target < filter_base {
                        LibrarySlotEvent::SelectorPicked(*target)
                    } else {
                        LibrarySlotEvent::ControlPicked(*target - filter_base)
                    };
                    return self.content.on_slot_event(event);
                }
                self.content
                    .on_slot_event(LibrarySlotEvent::List(RowLocalInput::Click(at)))
            }
            MouseGesture::DoubleClick(at) => self
                .content
                .on_slot_event(LibrarySlotEvent::List(RowLocalInput::DoubleClick(at))),
            _ => None,
        }
    }

    /// The stable row id under `point`, resolved by the shared owner that
    /// painted the active list (design.md D6).
    pub(in crate::app) fn resolve_row_id(&self, at: Position) -> Option<String> {
        self.content.resolve_row_id(at)
    }

    /// Test seam: reset the private gesture recognizer so a synchronous test
    /// loop can drive successive wheel/click events without the
    /// throttle/double-click window collapsing them.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }
}

impl Default for FeedsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FeedsComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // One shared owner per logical row flow (design.md D1): a breakpoint
        // change reconfigures the same owner and preserves only the outgoing
        // selected-row viewport offset — the owner is never copied between
        // presentations. Task 7.2 paints the panel's shared skeleton over
        // `content()`; Feeds owns no painter and reads only projected image
        // state (design D9).
        let wide = wide_hero_fits(area);
        self.wide = wide;
        // Controls-pill ids are offset past the feed groups so one lookup
        // distinguishes the two rows (design D8).
        let filter_base = self.content.group_count();
        let mut content = self.content.content();
        let mut hits = SkeletonHits::default();
        if wide {
            if let Some(geometry) = render_wide_skeleton(
                frame,
                area,
                &mut content,
                self.focused,
                self.list_pane_width,
                &mut hits,
            ) {
                self.layout = LayoutMain {
                    feeds_area: area,
                    left_area: geometry.list_area,
                    hero_area: geometry.hero,
                    selected_item_rect: geometry.selected,
                    selector_tabs: combined_selector_tabs(&hits, filter_base),
                    ..LayoutMain::default()
                };
                return;
            }
        }
        let geometry = render_narrow_skeleton(frame, area, &mut content, self.focused, &mut hits);
        let inline_hero = geometry.inline_hero.unwrap_or_default();
        self.layout = LayoutMain {
            feeds_area: area,
            left_area: geometry.list_area,
            hero_area: inline_hero,
            inline_hero_area: inline_hero,
            selected_item_rect: geometry.selected,
            selector_tabs: combined_selector_tabs(&hits, filter_base),
            ..LayoutMain::default()
        };
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

impl AppComponent<Msg, UserEvent> for FeedsComponent {
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
