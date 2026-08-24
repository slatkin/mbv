//! Interactive Component for the Feeds destination.
//!
//! The shell supplies validated feed snapshots. This component owns the
//! selector/filter/list cursor, grouping presentation, inline hero painting,
//! and render geometry. Refresh, playback, enqueue, and the legacy mouse path
//! remain shell work during the mirror-first stage.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::render::{render_feeds_content, FeedsRenderModel};
use crate::app::types_feed_tab::WatchedFilter;
use crate::app::ui_util::move_cursor;
use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;

pub struct FeedsComponent {
    subscriptions: Vec<FeedSubscription>,
    entries: Vec<Vec<FeedEntry>>,
    all_entries: Vec<FeedEntry>,
    visible_entries: Vec<FeedEntry>,
    watched_filter: WatchedFilter,
    selected_group: usize,
    cursor: usize,
    scroll: usize,
    loading: bool,
    focused: bool,
    layout: LayoutMain,
}

impl FeedsComponent {
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
            entries: Vec::new(),
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            watched_filter: WatchedFilter::default(),
            selected_group: 0,
            cursor: 0,
            scroll: 0,
            loading: false,
            focused: false,
            layout: LayoutMain::default(),
        }
    }

    /// Replace the shell-owned snapshot while preserving the component's
    /// render and input state shape.
    pub(in crate::app) fn set_content(
        &mut self,
        subscriptions: &[FeedSubscription],
        entries: &[Vec<FeedEntry>],
        all_entries: &[FeedEntry],
        watched_filter: WatchedFilter,
        selected_group: usize,
        cursor: usize,
        scroll: usize,
        loading: bool,
        focused: bool,
    ) {
        self.subscriptions = subscriptions.to_vec();
        self.entries = entries.to_vec();
        self.all_entries = all_entries.to_vec();
        self.watched_filter = watched_filter;
        self.selected_group = selected_group.min(self.group_count().saturating_sub(1));
        self.cursor = cursor;
        self.scroll = scroll;
        self.loading = loading;
        self.focused = focused;
        self.rebuild_visible_entries();
        self.clamp_cursor();
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn watched_filter(&self) -> WatchedFilter {
        self.watched_filter
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

    fn group_count(&self) -> usize {
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
    }

    fn clamp_cursor(&mut self) {
        if self.visible_entries.is_empty() {
            self.cursor = 0;
            self.scroll = 0;
        } else {
            self.cursor = self.cursor.min(self.visible_entries.len() - 1);
        }
    }

    fn move_cursor(&mut self, delta: i64) {
        self.cursor = move_cursor(self.cursor, delta, self.visible_entries.len());
    }

    fn move_cursor_rows(&mut self, delta: i64) {
        if self.visible_entries.is_empty() {
            self.cursor = 0;
            return;
        }
        let rows: Vec<&Vec<usize>> = self
            .layout
            .left_item_rows
            .iter()
            .filter(|row| !row.is_empty())
            .collect();
        let Some((current_row, current_column)) =
            rows.iter().enumerate().find_map(|(row, items)| {
                items
                    .iter()
                    .position(|&index| index == self.cursor)
                    .map(|column| (row, column))
            })
        else {
            self.move_cursor(delta);
            return;
        };
        let target_row = if delta < 0 {
            current_row.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            current_row
                .saturating_add(delta as usize)
                .min(rows.len().saturating_sub(1))
        };
        if let Some(index) = rows[target_row]
            .get(current_column)
            .copied()
            .or_else(|| rows[target_row].last().copied())
        {
            self.cursor = index;
        }
    }

    fn cycle_group(&mut self, delta: i64) {
        let count = self.group_count();
        self.selected_group =
            (self.selected_group as i64 + delta).rem_euclid(count as i64) as usize;
        self.cursor = 0;
        self.scroll = 0;
        self.rebuild_visible_entries();
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        }
        let noop = || Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        match key.code {
            Key::Char('r') => Some(Msg::Shell(ShellRequest::RefreshFeeds)),
            Key::Char('w') => {
                self.watched_filter = self.watched_filter.cycle();
                self.cursor = 0;
                self.scroll = 0;
                self.rebuild_visible_entries();
                noop()
            }
            Key::Up | Key::Char('k') => {
                self.move_cursor_rows(-1);
                noop()
            }
            Key::Down | Key::Char('j') => {
                self.move_cursor_rows(1);
                noop()
            }
            Key::Left | Key::Char('h')
                if self.layout.left_area.width > 0
                    && crate::app::library_column_width::library_column_count(
                        self.layout.left_area.width,
                    ) > 1 =>
            {
                self.move_cursor(-1);
                noop()
            }
            Key::Right | Key::Char('l')
                if self.layout.left_area.width > 0
                    && crate::app::library_column_width::library_column_count(
                        self.layout.left_area.width,
                    ) > 1 =>
            {
                self.move_cursor(1);
                noop()
            }
            Key::PageUp => {
                self.cursor = self
                    .cursor
                    .saturating_sub(self.layout.left_area.height.saturating_sub(1).max(1) as usize);
                noop()
            }
            Key::PageDown => {
                self.cursor = (self.cursor
                    + self.layout.left_area.height.saturating_sub(1).max(1) as usize)
                    .min(self.visible_entries.len().saturating_sub(1));
                noop()
            }
            Key::Home => {
                self.cursor = 0;
                noop()
            }
            Key::End => {
                self.cursor = self.visible_entries.len().saturating_sub(1);
                noop()
            }
            Key::Char('[') => {
                self.cycle_group(-1);
                noop()
            }
            Key::Char(']') => {
                self.cycle_group(1);
                noop()
            }
            Key::Enter => Some(Msg::Shell(ShellRequest::FeedsPlay(self.cursor))),
            Key::Char('e') => Some(Msg::Shell(ShellRequest::FeedsEnqueue(self.cursor))),
            _ => Some(Msg::Legacy(LegacyTerminalEvent::Key(
                to_crossterm_key_event(key),
            ))),
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(
            to_crossterm_mouse_event(mouse),
        )))
    }
}

impl Default for FeedsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FeedsComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
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
                cursor: &mut self.cursor,
                scroll: &mut self.scroll,
            },
        );
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
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }
}
