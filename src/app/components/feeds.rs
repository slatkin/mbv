//! Interactive Component for the Feeds destination.
//!
//! The shell supplies validated feed snapshots. This component owns the
//! selector/filter/list cursor, grouping presentation, inline hero painting,
//! and render geometry. Refresh, playback, enqueue, and the legacy mouse path
//! remain shell work during the mirror-first stage.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{InlineMediaBrowser, MediaListRow, MediaSemanticState, WideMediaList};
use super::msg::{Msg, ShellRequest};
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
    /// Canonical structural projection; selectors remain parent chrome.
    canonical_list: WideMediaList<String>,
    inline_list: InlineMediaBrowser<String>,
    watched_filter: WatchedFilter,
    selected_group: usize,
    cursor: usize,
    scroll: usize,
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
            cursor: 0,
            scroll: 0,
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
        if subscriptions_changed {
            self.cursor = 0;
            self.scroll = 0;
        }
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

    pub(in crate::app) fn selected_group(&self) -> usize {
        self.selected_group
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.scroll
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
        // Navigation uses rows produced by the previous render; invalidate
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
        let mut rows = Vec::new();
        let mut last_date = None;
        for entry in &self.visible_entries {
            let date = entry.pub_date_secs.map(|value| value / 86_400);
            if date != last_date {
                if last_date.is_some() {
                    rows.push(MediaListRow::Spacer);
                }
                rows.push(MediaListRow::Heading {
                    text: date.map_or_else(|| "Unknown".into(), |day| day.to_string()),
                });
                last_date = date;
            }
            rows.push(MediaListRow::Item {
                target: entry.guid.clone(),
                primary: entry.title.clone(),
                trailing: None,
                duration: None,
                semantic_state: if entry.played {
                    MediaSemanticState::Played
                } else {
                    MediaSemanticState::Ordinary
                },
            });
        }
        self.canonical_list.set_content(rows.clone());
        self.inline_list.set_content(rows);
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
                self.cursor = 0;
                self.scroll = 0;
                self.rebuild_visible_entries();
                None
            }
            Key::Up | Key::Char('k') => {
                self.move_cursor_rows(-1);
                None
            }
            Key::Down | Key::Char('j') => {
                self.move_cursor_rows(1);
                None
            }
            Key::Left | Key::Char('h')
                if self.layout.left_area.width > 0
                    && crate::app::library_column_width::library_column_count(
                        self.layout.left_area.width,
                    ) > 1 =>
            {
                self.move_cursor(-1);
                None
            }
            Key::Right | Key::Char('l')
                if self.layout.left_area.width > 0
                    && crate::app::library_column_width::library_column_count(
                        self.layout.left_area.width,
                    ) > 1 =>
            {
                self.move_cursor(1);
                None
            }
            Key::PageUp => {
                self.cursor = self
                    .cursor
                    .saturating_sub(self.layout.left_area.height.saturating_sub(1).max(1) as usize);
                None
            }
            Key::PageDown => {
                self.cursor = (self.cursor
                    + self.layout.left_area.height.saturating_sub(1).max(1) as usize)
                    .min(self.visible_entries.len().saturating_sub(1));
                None
            }
            Key::Home => {
                self.cursor = 0;
                None
            }
            Key::End => {
                self.cursor = self.visible_entries.len().saturating_sub(1);
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
                .get(self.cursor)
                .map(|entry| Msg::Shell(ShellRequest::FeedsPlay(Some(entry.clone()))))
                .or_else(|| Some(Msg::Shell(ShellRequest::FeedsPlay(None)))),
            Key::Char('e') => self
                .visible_entries
                .get(self.cursor)
                .map(|entry| Msg::Shell(ShellRequest::FeedsEnqueue(Some(entry.clone()))))
                .or_else(|| Some(Msg::Shell(ShellRequest::FeedsEnqueue(None)))),
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
                self.move_cursor(1);
            }
            MouseEventKind::ScrollUp if self.layout.left_area.contains(position) => {
                self.move_cursor(-1);
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
                        self.cursor = 0;
                        self.scroll = 0;
                        self.rebuild_visible_entries();
                    }
                    return None;
                }
                if self.layout.left_area.contains(position) {
                    let list_area = self.layout.left_area;
                    let click_y = (mouse.row - list_area.y) as usize;
                    let n = self.visible_entries.len();
                    let cell_target = {
                        let cols =
                            crate::app::library_column_width::library_column_count(list_area.width);
                        let cell_width =
                            crate::app::library_column_width::library_cell_width(list_area, cols)
                                as usize;
                        let x = (mouse.column as usize).saturating_sub(list_area.x as usize);
                        if !self.layout.left_item_rows.is_empty() && cols > 1 && cell_width > 0 {
                            let cell = x
                                / (cell_width
                                    + crate::app::library_column_width::LIBRARY_COLUMN_GAP
                                        as usize);
                            (cell < cols)
                                .then(|| {
                                    self.layout
                                        .left_item_rows
                                        .get(self.scroll + click_y)
                                        .and_then(|row| row.get(cell).copied())
                                })
                                .flatten()
                        } else {
                            None
                        }
                    };
                    if let Some(item_idx) = cell_target {
                        if item_idx < n {
                            self.cursor = item_idx;
                        }
                    } else if let Some(Some(item_idx)) = self.layout.left_row_map.get(click_y) {
                        if *item_idx < n {
                            self.cursor = *item_idx;
                        }
                    } else if self.layout.left_row_map.is_empty() {
                        let visible = list_area.height as usize;
                        let offset = if self.cursor >= visible {
                            self.cursor - visible + 1
                        } else {
                            0
                        };
                        let clicked = offset + click_y;
                        if clicked < n {
                            self.cursor = clicked;
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
        let wide = crate::app::library_column_width::library_column_count(layout.left_area.width)
            == 1
            && layout.left_area.width > 0;
        if wide {
            let offset = crate::app::render::render_wide_media_list(
                frame,
                layout.left_area,
                &self.canonical_list,
                self.focused,
                crate::app::palette::SURFACE_FOCUSED,
                &mut layout,
            );
            self.canonical_list.set_scroll(offset);
        } else {
            let offset = crate::app::render::render_inline_media_browser(
                frame,
                layout.left_area,
                &self.inline_list,
                0,
                self.focused,
                crate::app::palette::SURFACE_FOCUSED,
            )
            .offset;
            self.inline_list.set_scroll(offset);
        }
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
