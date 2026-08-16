use super::widgets::{
    content_width, render_pill_bar, render_placeholder, render_right_scrollbar_with_viewport,
    PillBar,
};
use crate::app::layout::LayoutMain;
use crate::app::palette;
use crate::app::App;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::{SystemTime, UNIX_EPOCH};

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeedAgeGroup {
    New,
    Recent,
    OlderThanTwoWeeks,
    OlderThanMonth,
    Unknown,
}

impl FeedAgeGroup {
    fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Recent => "Recent",
            Self::OlderThanTwoWeeks => "Older than two weeks",
            Self::OlderThanMonth => "Older than a month",
            Self::Unknown => "Unknown date",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FeedDisplayRow {
    Spacer,
    Heading(FeedAgeGroup),
    Entry(usize),
}

fn feed_age_group(pub_date_secs: Option<u64>, now_secs: u64) -> FeedAgeGroup {
    let Some(pub_date_secs) = pub_date_secs else {
        return FeedAgeGroup::Unknown;
    };

    match now_secs.saturating_sub(pub_date_secs) / SECONDS_PER_DAY {
        0..=1 => FeedAgeGroup::New,
        2..=13 => FeedAgeGroup::Recent,
        14..=29 => FeedAgeGroup::OlderThanTwoWeeks,
        _ => FeedAgeGroup::OlderThanMonth,
    }
}

fn feed_display_rows(
    entries: &[mbv_core::playback_queue::FeedEntry],
    now_secs: u64,
) -> Vec<FeedDisplayRow> {
    let mut rows = Vec::new();
    let mut last_group = None;

    for (idx, entry) in entries.iter().enumerate() {
        let group = feed_age_group(entry.pub_date_secs, now_secs);
        if last_group != Some(group) {
            if last_group.is_some() {
                rows.push(FeedDisplayRow::Spacer);
            }
            rows.push(FeedDisplayRow::Heading(group));
            last_group = Some(group);
        }
        rows.push(FeedDisplayRow::Entry(idx));
    }

    rows
}

fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Format a tick count into a human-readable duration string.
fn format_duration(ticks: Option<u64>) -> String {
    match ticks {
        Some(t) if t > 0 => {
            let total_secs = t / TICKS_PER_SECOND as u64;
            let h = total_secs / 3600;
            let m = (total_secs % 3600) / 60;
            let s = total_secs % 60;
            if h > 0 {
                format!("{h}:{m:02}:{s:02}")
            } else {
                format!("{m}:{s:02}")
            }
        }
        _ => String::new(),
    }
}

/// Format a pub_date_secs value into a short date string for display.
fn format_pub_date(secs: Option<u64>) -> String {
    match secs {
        Some(s) => {
            // Simple YYYY-MM-DD from unix seconds
            let days = (s / 86400) as i64 + 719468;
            let era = if days >= 0 { days } else { days - 146096 } / 146097;
            let doe = days - era * 146097;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            let yr = if m <= 2 { y + 1 } else { y };
            format!("{yr:04}-{m:02}-{d:02}")
        }
        None => String::new(),
    }
}

impl App {
    pub(super) fn render_feeds(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        let state = &self.feed_tab;
        let subscriptions = &state.subscriptions;
        let has_subs = !subscriptions.is_empty();
        let loading = state.loading;

        let max_y = area.y + area.height;
        let mut row = area.y;
        let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();

        // Pill bar: "All" + one per subscription.
        if row < max_y && has_subs {
            const MAX_LABEL: usize = 12;
            let mut labels: Vec<String> = vec!["All".to_string()];
            for sub in subscriptions {
                let name = if sub.name.len() > MAX_LABEL {
                    format!("{}…", &sub.name[..MAX_LABEL])
                } else {
                    sub.name.clone()
                };
                labels.push(name);
            }
            let ids: Vec<usize> = (0..labels.len()).collect();
            selector_tabs = render_pill_bar(
                f,
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: state.selected_group,
                    prefix: Some(" ⌘ "),
                },
            );
        }
        if row < max_y {
            row += 1;
        }
        // Keep the playback-state filter visually separated from the group
        // pill bar.
        if row < max_y {
            row += 1;
        }
        // Watched filter indicator line.
        if row < max_y && has_subs {
            let filter = state.watched_filter;
            let mut spans = Vec::new();
            for (i, f_variant) in [
                super::super::types_feed_tab::WatchedFilter::All,
                super::super::types_feed_tab::WatchedFilter::Watched,
                super::super::types_feed_tab::WatchedFilter::Unwatched,
            ]
            .iter()
            .enumerate()
            {
                if i > 0 {
                    spans.push(Span::styled(" · ", Style::default().fg(palette::MUTED)));
                }
                let active = *f_variant == filter;
                spans.push(Span::styled(
                    f_variant.label().to_string(),
                    if active {
                        Style::default()
                            .fg(palette::AQUA)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::MUTED)
                    },
                ));
            }
            f.render_widget(
                Paragraph::new(Line::from(spans))
                    .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }
        if row < max_y {
            row += 1;
        }
        layout.selector_tabs = selector_tabs;

        let list_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        layout.left_area = list_area;
        if list_area.height == 0 {
            return;
        }

        // Empty / help state.
        if !has_subs {
            render_placeholder(
                f,
                Rect {
                    x: list_area.x,
                    y: list_area.y,
                    width: list_area.width,
                    height: 1,
                },
                " No feed subscriptions configured",
            );
            return;
        }

        let n = self.feed_tab.visible_entries().len();
        if n == 0 {
            let msg = if loading {
                " Loading…"
            } else {
                " Press r to load feeds"
            };
            render_placeholder(
                f,
                Rect {
                    x: list_area.x,
                    y: list_area.y,
                    width: list_area.width,
                    height: 1,
                },
                msg,
            );
            return;
        }

        // Render the entry list. Headings are presentation-only: the cursor
        // and all actions continue to address entries by their canonical
        // index in `visible_entries()`.
        let cursor = self.feed_tab.cursor.min(n.saturating_sub(1));
        let display_rows = feed_display_rows(self.feed_tab.visible_entries(), current_time_secs());
        let visible = list_area.height as usize;
        let display_cursor = display_rows
            .iter()
            .position(|row| matches!(row, FeedDisplayRow::Entry(idx) if *idx == cursor))
            .unwrap_or(0);
        let lower_bound = display_cursor.saturating_add(1).saturating_sub(visible);
        let upper_bound = display_cursor.min(display_rows.len().saturating_sub(visible));
        let scroll = self.feed_tab.scroll.clamp(lower_bound, upper_bound);
        self.feed_tab.scroll = scroll;
        let entries = self.feed_tab.visible_entries();
        let text_w_with_sb = (list_area.width as usize).saturating_sub(1);
        let text_w = content_width(list_area.width, true);
        let visible_count = display_rows.len().saturating_sub(scroll).min(visible);
        let mut row_map: Vec<Option<usize>> = Vec::with_capacity(list_area.height as usize);

        for display_row in display_rows.iter().skip(scroll).take(visible) {
            if row >= list_area.y + list_area.height {
                break;
            }

            match display_row {
                FeedDisplayRow::Spacer => {
                    f.render_widget(
                        Paragraph::new(Line::default())
                            .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                        Rect {
                            x: list_area.x,
                            y: row,
                            width: text_w.min(text_w_with_sb) as u16,
                            height: 1,
                        },
                    );
                    row_map.push(None);
                    row += 1;
                    continue;
                }
                FeedDisplayRow::Heading(group) => {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                group.label(),
                                Style::default()
                                    .fg(palette::YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]))
                        .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                        Rect {
                            x: list_area.x,
                            y: row,
                            width: text_w.min(text_w_with_sb) as u16,
                            height: 1,
                        },
                    );
                    row_map.push(None);
                    row += 1;
                    continue;
                }
                FeedDisplayRow::Entry(i) => {
                    let entry = &entries[*i];
                    let selected = *i == cursor;

                    let selected_bg = palette::resolve_surface_focus(focused);

                    let bg = if selected {
                        selected_bg
                    } else {
                        palette::SURFACE_BACKDROP
                    };
                    let fg = if selected {
                        if focused {
                            palette::WHITE
                        } else {
                            palette::SUBTLE
                        }
                    } else {
                        palette::TEXT
                    };

                    let marker = if selected { "▶ " } else { "  " };
                    let title = &entry.title;
                    let duration = format_duration(entry.duration_ticks);
                    let date = format_pub_date(entry.pub_date_secs);
                    let mime = entry.mime_type.as_deref().unwrap_or("");

                    // Build the display line.
                    let mut spans = vec![Span::styled(
                        marker,
                        Style::default()
                            .fg(palette::AQUA)
                            .add_modifier(Modifier::BOLD),
                    )];
                    if entry.played {
                        spans.push(Span::styled("✓ ", Style::default().fg(palette::GREEN)));
                    }
                    spans.push(Span::styled(
                        format!("{title}  "),
                        Style::default().fg(fg).add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ));
                    if !duration.is_empty() {
                        spans.push(Span::styled(
                            format!("{duration} "),
                            Style::default().fg(palette::PLAYBACK_META_FG),
                        ));
                    }
                    if !date.is_empty() {
                        spans.push(Span::styled(
                            format!("{date} "),
                            Style::default().fg(palette::MUTED),
                        ));
                    }
                    if !mime.is_empty() {
                        spans.push(Span::styled(
                            mime.to_string(),
                            Style::default().fg(palette::MUTED),
                        ));
                    }

                    // Truncate to available width.
                    let line = Line::from(spans);
                    let display_w = text_w.min(text_w_with_sb) as u16;
                    f.render_widget(
                        Paragraph::new(line).style(Style::default().bg(bg)),
                        Rect {
                            x: list_area.x,
                            y: row,
                            width: display_w,
                            height: 1,
                        },
                    );

                    row_map.push(Some(*i));
                    row += 1;
                }
            }
        }
        row_map.resize(list_area.height as usize, None);
        layout.left_row_map = row_map;

        if visible_count > 0 && visible_count < display_rows.len() {
            render_right_scrollbar_with_viewport(
                f,
                list_area,
                display_rows.len(),
                visible_count,
                scroll,
                palette::SCROLLBAR,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::config::FeedKind;
    use mbv_core::playback_queue::FeedEntry;

    fn entry(title: &str, pub_date_secs: Option<u64>) -> FeedEntry {
        FeedEntry {
            guid: title.to_string(),
            title: title.to_string(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs,
            feed_kind: Some(FeedKind::Video),
            feed_id: None,
            position_ticks: 0,
            played: false,
        }
    }

    #[test]
    fn age_groups_cover_boundaries_and_unknown_dates() {
        let now = 30 * SECONDS_PER_DAY;
        let cases = [
            (Some(now + SECONDS_PER_DAY), FeedAgeGroup::New),
            (Some(now), FeedAgeGroup::New),
            (Some(now - SECONDS_PER_DAY), FeedAgeGroup::New),
            (Some(now - 2 * SECONDS_PER_DAY), FeedAgeGroup::Recent),
            (Some(now - 13 * SECONDS_PER_DAY), FeedAgeGroup::Recent),
            (
                Some(now - 14 * SECONDS_PER_DAY),
                FeedAgeGroup::OlderThanTwoWeeks,
            ),
            (
                Some(now - 29 * SECONDS_PER_DAY),
                FeedAgeGroup::OlderThanTwoWeeks,
            ),
            (
                Some(now - 30 * SECONDS_PER_DAY),
                FeedAgeGroup::OlderThanMonth,
            ),
            (None, FeedAgeGroup::Unknown),
        ];

        for (date, expected) in cases {
            assert_eq!(feed_age_group(date, now), expected);
        }
    }

    #[test]
    fn display_rows_insert_non_selectable_groups_without_changing_indices() {
        let now = 30 * SECONDS_PER_DAY;
        let entries = vec![
            entry("new", Some(now)),
            entry("recent", Some(now - 2 * SECONDS_PER_DAY)),
            entry("two weeks", Some(now - 14 * SECONDS_PER_DAY)),
            entry("month", Some(now - 30 * SECONDS_PER_DAY)),
            entry("unknown", None),
        ];

        assert_eq!(
            feed_display_rows(&entries, now),
            vec![
                FeedDisplayRow::Heading(FeedAgeGroup::New),
                FeedDisplayRow::Entry(0),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::Recent),
                FeedDisplayRow::Entry(1),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::OlderThanTwoWeeks),
                FeedDisplayRow::Entry(2),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::OlderThanMonth),
                FeedDisplayRow::Entry(3),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::Unknown),
                FeedDisplayRow::Entry(4),
            ]
        );
    }
}
