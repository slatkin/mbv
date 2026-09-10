//! Interactive Component for the Feeds destination.
//!
//! The shell supplies validated feed snapshots. This component owns the
//! subscription/group selector and the watched filter (parent chrome); one
//! shared canonical `MediaList` owner of the grouped-entry projection moves
//! between the `WideMediaList` (Wide hero Wide) and `InlineMediaBrowser`
//! (inline Narrow) presentations, owns the cursor and scroll, and receives
//! every eligible row-local key and pointer gesture through the common
//! delegation seam. `render_feeds_content` is the parent-owned pill strip +
//! chrome + hero painter and mounts the active presentation into the list
//! sub-rect below the pill strip.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, RowIntent,
    RowLocalInput, RowLocalOutcome,
};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{Msg, ShellRequest, TerminalObserverEvent};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::render::{
    current_time_secs, feed_display_rows, feed_duration_text, render_feeds_content, FeedDisplayRow,
    FeedsPresentation, FeedsRenderModel,
};
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;

pub struct FeedsComponent {
    subscriptions: Vec<FeedSubscription>,
    entries: Vec<Vec<FeedEntry>>,
    all_entries: Vec<FeedEntry>,
    visible_entries: Vec<FeedEntry>,
    /// The one shared canonical owner of the grouped-entry projection, carried
    /// by exactly one of the persistent presentations (design.md D1). It owns
    /// cursor, scroll, and selected target; selectors remain parent chrome.
    carrier: MediaListCarrier<String>,
    watched_filter: WatchedFilter,
    selected_group: usize,
    /// Which presentation the last `view()` painted (Wide hero Wide vs inline
    /// Narrow). A breakpoint change moves the same shared owner between the
    /// persistent presentations; it also selects which owner `cursor()` reads.
    wide: bool,
    loading: bool,
    images_enabled: bool,
    focused: bool,
    layout: LayoutMain,
    last_subscription_urls: Vec<String>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
}

impl FeedsComponent {
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
            entries: Vec::new(),
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            carrier: MediaListCarrier::new(Presentation::Inline),
            watched_filter: WatchedFilter::default(),
            selected_group: 0,
            wide: false,
            loading: false,
            images_enabled: true,
            focused: false,
            layout: LayoutMain::default(),
            last_subscription_urls: Vec::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Replace the shell-owned snapshot while preserving the component's
    /// render and input state shape.
    pub(in crate::app) fn set_images_enabled(&mut self, images_enabled: bool) {
        self.images_enabled = images_enabled;
    }

    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn set_content(
        &mut self,
        subscriptions: &[FeedSubscription],
        entries: &[Vec<FeedEntry>],
        all_entries: &[FeedEntry],
        loading: bool,
    ) {
        let subscription_urls: Vec<String> = subscriptions
            .iter()
            .map(|subscription| subscription.url.clone())
            .collect();
        let subscriptions_changed = self.last_subscription_urls != subscription_urls;
        self.last_subscription_urls = subscription_urls;
        self.subscriptions = subscriptions.to_vec();
        self.entries = entries.to_vec();
        self.all_entries = all_entries.to_vec();
        self.selected_group = self
            .selected_group
            .min(self.group_count().saturating_sub(1));
        self.loading = loading;
        self.rebuild_visible_entries();
        // An ordinary refresh keeps the active control authoritative (the
        // selected target is preserved by `ListCore::set_content`); only a
        // subscription-set change resets the selection.
        if subscriptions_changed {
            self.reset_selection();
        }
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier.cursor()
    }

    pub(in crate::app) fn watched_filter(&self) -> WatchedFilter {
        self.watched_filter
    }

    pub(in crate::app) fn selected_group(&self) -> usize {
        self.selected_group
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier.scroll()
    }

    pub(in crate::app) fn visible_titles(&self) -> Vec<&str> {
        self.visible_entries
            .iter()
            .map(|entry| entry.title.as_str())
            .collect()
    }

    pub(in crate::app) fn subscription_names(&self) -> Vec<&str> {
        self.subscriptions
            .iter()
            .map(|subscription| subscription.name.as_str())
            .collect()
    }

    pub(in crate::app) fn layout(&self) -> &LayoutMain {
        &self.layout
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_rows(&self) -> &[MediaListRow<String>] {
        self.carrier.rows()
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_selectable_len(&self) -> usize {
        self.carrier
            .rows()
            .iter()
            .filter(|row| row.selectable_target().is_some())
            .count()
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_selected_target(&self) -> Option<&String> {
        self.carrier.selected_target()
    }

    pub(in crate::app) fn group_count(&self) -> usize {
        1 + self.subscriptions.len()
    }

    fn rebuild_visible_entries(&mut self) {
        let source = if self.selected_group == 0 {
            &self.all_entries
        } else {
            self.entries
                .get(self.selected_group - 1)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        };
        self.visible_entries = source
            .iter()
            .filter(|entry| self.watched_filter.matches(entry.played))
            .cloned()
            .collect();

        // Project grouped `FeedEntries` into the canonical row vocabulary:
        // `FeedAgeGroup` labels become non-selectable `Heading` rows, group
        // separators become `Spacer` rows, entries become selectable `Item`
        // rows carrying the stable `entry.guid` target and the watched
        // semantic state. Structural rows are filtered out of the control's
        // selectable index, so cursor movement skips them and the control's
        // `RowGeometry` owns the selectable-index vs display-index mapping.
        let now = current_time_secs();
        let rows: Vec<MediaListRow<String>> = feed_display_rows(&self.visible_entries, now)
            .into_iter()
            .map(|row| match row {
                FeedDisplayRow::Spacer => MediaListRow::Spacer,
                FeedDisplayRow::Heading(group) => MediaListRow::Heading {
                    text: group.label().to_string(),
                },
                FeedDisplayRow::Entry(index) => {
                    let entry = &self.visible_entries[index];
                    MediaListRow::Item {
                        target: entry.guid.clone(),
                        primary: entry.title.clone(),
                        trailing: None,
                        duration: feed_duration_text(entry.duration_ticks),
                        kind: MediaKind::Media,
                        semantic_state: if entry.played {
                            MediaSemanticState::Played
                        } else if entry.position_ticks > 0 {
                            let progress = entry
                                .duration_ticks
                                .filter(|duration| *duration > 0)
                                .map(|duration| {
                                    ((entry.position_ticks.max(0) as u64 * 100) / duration).min(100)
                                        as u16
                                });
                            MediaSemanticState::active(progress)
                        } else {
                            MediaSemanticState::Ordinary
                        },
                    }
                }
            })
            .collect();
        self.carrier.set_content(rows);
    }

    /// Park the shared owner at the first entry after a discrete group/filter
    /// change (design.md D5: re-project then explicitly select the required
    /// stable target).
    fn reset_selection(&mut self) {
        self.delegate_row_local_input(RowLocalInput::First, None);
    }

    /// The one seam through which Feeds offers an already-normalized row-local
    /// key or pointer gesture to the shared owner carrying its entry rows. The
    /// owner applies the local state transition and returns the closed
    /// provider-neutral outcome; Feeds translates external row intents into
    /// its typed Msgs (design.md D3).
    fn delegate_row_local_input(
        &mut self,
        input: RowLocalInput,
        pointer_target: Option<String>,
    ) -> RowLocalOutcome<String> {
        self.ensure_carrier();
        self.carrier.delegate(input, pointer_target)
    }

    /// The entry whose stable `guid` the shared owner selected. Effect
    /// requests are built from this owner-resolved target, never by indexing
    /// `visible_entries` with the cursor.
    fn entry_for_target(&self, target: &str) -> Option<&FeedEntry> {
        self.visible_entries
            .iter()
            .find(|entry| entry.guid == target)
    }

    /// Adopt `target` as the selected group and rebuild.
    fn select_group(&mut self, target: usize) {
        self.selected_group = target;
        self.rebuild_visible_entries();
        self.reset_selection();
    }

    fn page_size(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
    }

    fn active_presentation(&self) -> Presentation {
        if self.wide {
            Presentation::Wide
        } else {
            Presentation::Inline
        }
    }

    /// Move the shared owner into the active presentation when they diverge.
    /// A breakpoint change reads the same owner and preserves only the
    /// outgoing selected-row viewport offset (design.md D1); no cursor, scroll,
    /// or selection is copied between presentations.
    fn ensure_carrier(&mut self) {
        let target = self.active_presentation();
        let viewport_height = self.layout.left_area.height.max(1) as usize;
        self.carrier.ensure_presentation(target, viewport_height);
    }

    fn cycle_group(&mut self, delta: i64) {
        let count = self.group_count();
        self.selected_group =
            (self.selected_group as i64 + delta).rem_euclid(count as i64) as usize;
        self.rebuild_visible_entries();
        self.reset_selection();
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
                self.watched_filter = self.watched_filter.cycle();
                self.rebuild_visible_entries();
                self.reset_selection();
                None
            }
            Key::Up | Key::Char('k') | Key::Left | Key::Char('h') => {
                self.delegate_row_local_input(RowLocalInput::Move(-1), None);
                None
            }
            Key::Down | Key::Char('j') | Key::Right | Key::Char('l') => {
                self.delegate_row_local_input(RowLocalInput::Move(1), None);
                None
            }
            Key::PageUp => {
                self.delegate_row_local_input(RowLocalInput::Move(-self.page_size()), None);
                None
            }
            Key::PageDown => {
                self.delegate_row_local_input(RowLocalInput::Move(self.page_size()), None);
                None
            }
            Key::Home => {
                self.delegate_row_local_input(RowLocalInput::First, None);
                None
            }
            Key::End => {
                self.delegate_row_local_input(RowLocalInput::Last, None);
                None
            }
            Key::Char('[') => {
                self.cycle_group(-1);
                None
            }
            Key::Char(']') => {
                self.cycle_group(1);
                None
            }
            Key::Enter => match self.delegate_row_local_input(RowLocalInput::Activate, None) {
                RowLocalOutcome::External(RowIntent::Activate(target)) => Some(Msg::Shell(
                    ShellRequest::FeedsPlay(self.entry_for_target(&target).cloned()),
                )),
                _ => Some(Msg::Shell(ShellRequest::FeedsPlay(None))),
            },
            Key::Char('e') => match self.delegate_row_local_input(RowLocalInput::Activate, None) {
                RowLocalOutcome::External(RowIntent::Activate(target)) => Some(Msg::Shell(
                    ShellRequest::FeedsEnqueue(self.entry_for_target(&target).cloned()),
                )),
                _ => Some(Msg::Shell(ShellRequest::FeedsEnqueue(None))),
            },
            _ => None,
        }
    }

    /// Handle a TuiRealm mouse event via the private `MouseGestureState`
    /// (ADR 0024, design.md D3). Row identity comes from the active canonical
    /// control's `resolve_point` (design.md D6); the selector pills stay
    /// parent chrome resolved from `selector_tabs`. The component emits a
    /// semantic `Msg` — never raw coordinates. Feeds has no keyboard
    /// context-menu action (task 4.6), so right-click is ignored.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Feeds does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                if !self.carrier.claims_current_point(at) {
                    return None;
                }
                self.delegate_row_local_input(RowLocalInput::Wheel { at, delta }, None);
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::Click(at) => {
                if let Some((_, target)) = self
                    .layout
                    .selector_tabs
                    .iter()
                    .find(|(rect, _)| rect.contains(at))
                {
                    let filter_base = self.group_count();
                    if *target < filter_base {
                        self.select_group(*target);
                    } else if let Some(filter) = WatchedFilter::from_position(*target - filter_base)
                    {
                        self.watched_filter = filter;
                        self.rebuild_visible_entries();
                        self.reset_selection();
                    }
                    return None;
                }
                let target = self.resolve_row_id(at)?;
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                Some(Msg::Shell(ShellRequest::FeedsRowClick))
            }
            MouseGesture::DoubleClick(at) => {
                let target = self.resolve_row_id(at)?;
                // Select the painted row through the same delegation seam the
                // first click uses, then play the resolved entry (Home shape).
                // A target with no visible entry returns no message, matching
                // the pre-migration `position(...)?` guard.
                self.delegate_row_local_input(RowLocalInput::Click(at), Some(target.clone()));
                let entry = self.entry_for_target(&target)?.clone();
                Some(Msg::Shell(ShellRequest::FeedsPlay(Some(entry))))
            }
            _ => None,
        }
    }

    /// The stable row id under `point`, resolved by the shared owner that
    /// painted the active list (design.md D6).
    pub(in crate::app) fn resolve_row_id(&self, at: Position) -> Option<String> {
        self.carrier.resolve_current_point(at).cloned()
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
        // presentations.
        let wide = crate::app::render::wide_hero_presentation(area).is_some();
        self.wide = wide;
        self.ensure_carrier();

        let selected_entry = self
            .carrier
            .selected_target()
            .and_then(|target| self.entry_for_target(target))
            .cloned();
        let presentation = if self.carrier.active() == Presentation::Wide {
            FeedsPresentation::Wide(self.carrier.wide_mut())
        } else {
            FeedsPresentation::Inline(self.carrier.inline_mut())
        };
        let mut layout = LayoutMain::default();
        render_feeds_content(
            frame,
            area,
            self.focused,
            &mut layout,
            FeedsRenderModel {
                subscriptions: &self.subscriptions,
                visible_entries: &self.visible_entries,
                watched_filter: self.watched_filter,
                selected_group: self.selected_group,
                loading: self.loading,
                selected_entry: selected_entry.as_ref(),
                images_enabled: self.images_enabled,
            },
            presentation,
        );
        self.layout = layout;
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
        // Keep the shared owner in the presentation the painted breakpoint
        // currently selects before any row-local input touches it (design.md
        // D1).
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
