//! Interactive Component for the Feeds destination.
//!
//! The shell supplies validated feed snapshots. This component owns the
//! subscription/group selector and the watched filter (parent chrome); the
//! embedded canonical controls (`WideMediaList` for hero-on-left Wide,
//! `InlineMediaBrowser` for inline Narrow) own the cursor and scroll over the
//! grouped-entry projection. `render_feeds_content` is the parent-owned pill
//! strip + chrome + hero painter and mounts the active control into the list
//! sub-rect below the pill strip. Refresh, playback, enqueue, and the legacy
//! `*HitRegion` mouse path remain shell/pre-#638 work.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    InlineMediaBrowser, MediaListRow, MediaSemanticState, ViewportAnchor, WideMediaList,
};
use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::render::{
    current_time_secs, feed_display_rows, format_duration, render_feeds_content, FeedDisplayRow,
    FeedsRenderModel,
};
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;

pub struct FeedsComponent {
    subscriptions: Vec<FeedSubscription>,
    entries: Vec<Vec<FeedEntry>>,
    all_entries: Vec<FeedEntry>,
    visible_entries: Vec<FeedEntry>,
    /// Canonical grouped-entry projection; selectors remain parent chrome.
    /// Both controls hold the same rows every `rebuild_visible_entries`; only
    /// one is painted per breakpoint, and they own cursor/scroll — the
    /// component keeps no mirror.
    canonical_list: WideMediaList<String>,
    inline_list: InlineMediaBrowser<String>,
    watched_filter: WatchedFilter,
    selected_group: usize,
    /// Which canonical control the last `view()` painted (hero-on-left Wide vs
    /// inline Narrow). Drives the single `ViewportAnchor` handoff on a
    /// breakpoint flip and which control `cursor()` reads.
    wide: bool,
    /// The scroll offset the painter resolved this frame — observability only
    /// (characterization tests), never fed back into the control.
    painted_offset: usize,
    loading: bool,
    focused: bool,
    layout: LayoutMain,
    last_subscription_urls: Vec<String>,
}

impl FeedsComponent {
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
            entries: Vec::new(),
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            canonical_list: WideMediaList::new(),
            inline_list: InlineMediaBrowser::new(),
            watched_filter: WatchedFilter::default(),
            selected_group: 0,
            wide: false,
            painted_offset: 0,
            loading: false,
            focused: false,
            layout: LayoutMain::default(),
            last_subscription_urls: Vec::new(),
        }
    }

    /// Replace the shell-owned snapshot while preserving the component's
    /// render and input state shape.
    pub(in crate::app) fn set_content(
        &mut self,
        subscriptions: &[FeedSubscription],
        entries: &[Vec<FeedEntry>],
        all_entries: &[FeedEntry],
        loading: bool,
        focused: bool,
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
        self.focused = focused;
        self.rebuild_visible_entries();
        // An ordinary refresh keeps the active control authoritative (the
        // selected target is preserved by `ListCore::set_content`); only a
        // subscription-set change resets the selection.
        if subscriptions_changed {
            self.reset_selection();
        }
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        if self.wide {
            self.canonical_list.cursor()
        } else {
            self.inline_list.cursor()
        }
    }

    pub(in crate::app) fn watched_filter(&self) -> WatchedFilter {
        self.watched_filter
    }

    pub(in crate::app) fn selected_group(&self) -> usize {
        self.selected_group
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.painted_offset
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

    pub(in crate::app) fn group_count(&self) -> usize {
        1 + self.subscriptions.len()
    }

    fn rebuild_visible_entries(&mut self) {
        // Navigation uses maps produced by the previous render; invalidate
        // them whenever filtering/group content changes.
        self.layout.left_item_rows.clear();
        self.layout.left_row_map.clear();
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
                    let duration = format_duration(entry.duration_ticks);
                    MediaListRow::Item {
                        target: entry.guid.clone(),
                        primary: entry.title.clone(),
                        trailing: None,
                        duration: (!duration.is_empty()).then_some(duration),
                        semantic_state: if entry.played {
                            MediaSemanticState::Played
                        } else {
                            MediaSemanticState::Ordinary
                        },
                    }
                }
            })
            .collect();
        self.canonical_list.set_content(rows.clone());
        self.inline_list.set_content(rows);
    }

    /// Park the selection at the first entry on both controls (a discrete
    /// group/filter change; there is no per-group cursor cache).
    fn reset_selection(&mut self) {
        self.canonical_list.select_first();
        self.inline_list.select_first();
        self.painted_offset = 0;
    }

    /// Move the selection by `delta` selectable rows on both controls in
    /// lockstep, so they stay cursor-aligned across a breakpoint flip.
    fn move_selection(&mut self, delta: i64) {
        self.canonical_list.move_selection(delta);
        self.inline_list.move_selection(delta);
    }

    /// Place the selection at selectable index `index` (which, for Feeds, is
    /// the `visible_entries` index) on both controls. Pre-#638 mouse
    /// compatibility: the bespoke `*HitRegion` path reads control-exported
    /// geometry.
    fn select_entry(&mut self, index: usize) {
        self.canonical_list.select_index(index);
        self.inline_list.select_index(index);
    }

    fn page_size(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
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
            Key::Up | Key::Char('k') => {
                self.move_selection(-1);
                None
            }
            Key::Down | Key::Char('j') => {
                self.move_selection(1);
                None
            }
            Key::Left | Key::Char('h') => {
                self.move_selection(-1);
                None
            }
            Key::Right | Key::Char('l') => {
                self.move_selection(1);
                None
            }
            Key::PageUp => {
                self.move_selection(-self.page_size());
                None
            }
            Key::PageDown => {
                self.move_selection(self.page_size());
                None
            }
            Key::Home => {
                self.canonical_list.select_first();
                self.inline_list.select_first();
                None
            }
            Key::End => {
                self.canonical_list.select_last();
                self.inline_list.select_last();
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
            Key::Enter => self
                .visible_entries
                .get(self.cursor())
                .map(|entry| Msg::Shell(ShellRequest::FeedsPlay(Some(entry.clone()))))
                .or(Some(Msg::Shell(ShellRequest::FeedsPlay(None)))),
            Key::Char('e') => self
                .visible_entries
                .get(self.cursor())
                .map(|entry| Msg::Shell(ShellRequest::FeedsEnqueue(Some(entry.clone()))))
                .or(Some(Msg::Shell(ShellRequest::FeedsEnqueue(None)))),
            _ => None,
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if !self.focused {
            return None;
        }
        let position: Position = (mouse.column, mouse.row).into();
        match mouse.kind {
            MouseEventKind::ScrollDown if self.layout.left_area.contains(position) => {
                self.move_selection(1);
            }
            MouseEventKind::ScrollUp if self.layout.left_area.contains(position) => {
                self.move_selection(-1);
            }
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right) => {
                if let Some(target) = self
                    .layout
                    .selector_tabs
                    .iter()
                    .find(|(rect, _)| rect.contains(position))
                    .map(|(_, target)| *target)
                {
                    if target < self.group_count() {
                        self.selected_group = target;
                        self.rebuild_visible_entries();
                        self.reset_selection();
                    }
                    return None;
                }
                if self.layout.left_area.contains(position) {
                    let list_area = self.layout.left_area;
                    let click_y = (mouse.row.saturating_sub(list_area.y)) as usize;
                    let n = self.visible_entries.len();
                    if let Some(Some(item_idx)) = self.layout.left_row_map.get(click_y) {
                        if *item_idx < n {
                            self.select_entry(*item_idx);
                        }
                    } else if let Some(item_idx) = self
                        .layout
                        .left_item_rows
                        .get(self.painted_offset + click_y)
                        .and_then(|row| row.first().copied())
                    {
                        if item_idx < n {
                            self.select_entry(item_idx);
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }
}

impl Default for FeedsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FeedsComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // One `ViewportAnchor` handoff at a breakpoint flip: carry the
        // outgoing control's selected target + screen-row offset into the
        // incoming control (design.md D2/D3). The cursors already track in
        // lockstep.
        let wide = crate::app::render::shared_hero_presentation(area).is_some();
        if wide != self.wide {
            let viewport_height = self.layout.left_area.height.max(1) as usize;
            let anchor: Option<ViewportAnchor<String>> = if self.wide {
                self.canonical_list.viewport_anchor(viewport_height)
            } else {
                self.inline_list.viewport_anchor(viewport_height)
            };
            if let Some(anchor) = anchor {
                if wide {
                    self.canonical_list
                        .apply_viewport_anchor(&anchor, viewport_height);
                } else {
                    self.inline_list
                        .apply_viewport_anchor(&anchor, viewport_height);
                }
            }
            self.wide = wide;
        }

        let mut layout = LayoutMain::default();
        let selected_entry = self.visible_entries.get(self.cursor()).cloned();
        let offset = render_feeds_content(
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
            },
            &self.canonical_list,
            &self.inline_list,
        );
        self.painted_offset = offset;
        self.layout = layout;
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
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

impl AppComponent<Msg, UserEvent> for FeedsComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
