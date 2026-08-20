use super::hero::{
    inline_detail_flow, inline_display_row, inline_display_row_count, paint_hero_content,
    selected_detail_shell, HeroContent, InlineDisplayRow, HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use super::hero_left;
use super::list_rows::{draw_column_selection_markers, SELECTED_BLOCK_SIDE_PADDING};
use super::widgets::{
    render_pill_bar, render_placeholder, render_right_scrollbar_with_viewport, PillBar,
};
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{
    library_cell_width, library_column_count, LIBRARY_COLUMN_GAP,
};
use crate::app::palette;
use crate::app::App;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::FeedEntry;
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

/// Row budget for the feeds hero's text content (design.md decision 6:
/// element presence -- no image, since feed entries carry no artwork), a
/// title row (two-column lists only) plus a single metadata line and its
/// trailing spacer.
fn feed_hero_content_rows(show_title: bool) -> u16 {
    let title_rows = if show_title { HERO_TITLE_ROWS } else { 0 };
    title_rows + 2
}

/// The feeds hero's one metadata line: duration, publish date, MIME type,
/// and watched state, in that order -- feeds' declared metadata set
/// (design.md decision 6).
fn feed_entry_meta_line(entry: &FeedEntry) -> String {
    let mut parts = Vec::new();
    let duration = format_duration(entry.duration_ticks);
    if !duration.is_empty() {
        parts.push(duration);
    }
    let date = format_pub_date(entry.pub_date_secs);
    if !date.is_empty() {
        parts.push(date);
    }
    if let Some(mime) = entry.mime_type.as_deref() {
        if !mime.is_empty() {
            parts.push(mime.to_string());
        }
    }
    if entry.played {
        parts.push("Watched".to_string());
    }
    parts.join("   ")
}

/// One physical list row after packing `FeedDisplayRow`s into `cols`-wide
/// rows (design.md's two-column list): headings and spacers always occupy
/// their own row; entries pack `cols` per row, never crossing a heading/
/// spacer boundary (same rule `list_letter_groups.rs`'s `push_item_row`
/// uses for the Emby library list).
enum PackedFeedRow {
    Spacer,
    Heading(FeedAgeGroup),
    Items(Vec<usize>),
}

fn pack_feed_rows(display_rows: &[FeedDisplayRow], cols: usize) -> Vec<PackedFeedRow> {
    let cols = cols.max(1);
    let mut packed = Vec::new();
    let mut current: Vec<usize> = Vec::with_capacity(cols);
    for row in display_rows {
        match row {
            FeedDisplayRow::Entry(idx) => {
                current.push(*idx);
                if current.len() >= cols {
                    packed.push(PackedFeedRow::Items(std::mem::take(&mut current)));
                }
            }
            FeedDisplayRow::Spacer | FeedDisplayRow::Heading(_) => {
                if !current.is_empty() {
                    packed.push(PackedFeedRow::Items(std::mem::take(&mut current)));
                }
                packed.push(match row {
                    FeedDisplayRow::Spacer => PackedFeedRow::Spacer,
                    FeedDisplayRow::Heading(g) => PackedFeedRow::Heading(*g),
                    FeedDisplayRow::Entry(_) => unreachable!(),
                });
            }
        }
    }
    if !current.is_empty() {
        packed.push(PackedFeedRow::Items(current));
    }
    packed
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

        // Renders the group pill bar + watched-filter row starting at
        // `start_y`, returning the selector click targets and the first
        // free row after them (plus its trailing gap). Called either above
        // the list (no hero to show) or below the hero (unified with
        // Movies/TV's letter-range pills -- see `list.rs`/`hero.rs`).
        let render_selector_rows =
            |f: &mut Frame, pane: Rect, start_y: u16| -> (Vec<(Rect, usize)>, u16) {
                let max_y = pane.y + pane.height;
                let mut row = start_y;
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
                            x: pane.x,
                            y: row,
                            width: pane.width,
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
                            x: pane.x,
                            y: row,
                            width: pane.width,
                            height: 1,
                        },
                    );
                    row += 1;
                }
                if row < max_y {
                    row += 1;
                }
                (selector_tabs, row)
            };

        // Whether this frame will show a hero (has entries to select from):
        // if so, the selector rows move below it, unified with the
        // Movies/TV inline browser arrangement; otherwise (no subs / no
        // entries yet) they stay above the placeholder message, since
        // there's no hero to be below.
        let will_show_hero = has_subs && !self.feed_tab.visible_entries().is_empty();

        let (selector_tabs, list_area) = if will_show_hero {
            (Vec::new(), area)
        } else {
            let (selector_tabs, row) = render_selector_rows(f, area, area.y);
            let max_y = area.y + area.height;
            let list_area = Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: max_y.saturating_sub(row),
            };
            (selector_tabs, list_area)
        };
        layout.selector_tabs = selector_tabs;
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
        let cols = library_column_count(list_area.width);
        let packed = pack_feed_rows(&display_rows, cols);

        // Hero: the cursor-selected entry's title + metadata, painted above
        // the (now packed) list -- feeds' inline presentation
        // (design.md decision 6: no image, since feed entries carry none).
        // The group pill bar + watched-filter row render below the hero
        // (unified with Movies/TV's letter-range pills), so they're carved
        // out of the hero's own leftover space rather than `placement-neutral geometry`'s
        // built-in single-pill-row slot, which is too short for both rows.
        let selected_entry = self.feed_tab.visible_entries()[cursor].clone();
        let show_title = cols > 1;
        let wide_panes = hero_left::shared_hero_presentation(area);
        let wide = wide_panes.is_some();
        let hero_rows_desired =
            feed_hero_content_rows(if wide { show_title } else { true }) + HERO_BLOCK_EXTRA_ROWS;
        let (hero_area, post_hero_area, hero_rows) = if wide {
            let Some((hero_panel, right_panel)) = wide_panes else {
                unreachable!("wide_panes is present when wide is true");
            };
            (
                hero_panel,
                right_panel,
                hero_rows_desired.min(hero_panel.height),
            )
        } else {
            (Rect::default(), list_area, hero_rows_desired)
        };
        layout.hero_area = hero_area;
        if wide && hero_rows > 0 {
            selected_detail_shell(f, hero_area, hero_rows, focused);
            let content_rect = Rect {
                x: hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: hero_area.y + 2,
                width: hero_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: hero_rows.saturating_sub(HERO_BLOCK_EXTRA_ROWS),
            };
            let meta = feed_entry_meta_line(&selected_entry);
            let hero_content = HeroContent {
                title: show_title.then_some(selected_entry.title.as_str()),
                meta_line: Some(meta.as_str()),
                meta_color: palette::PLAYBACK_META_FG,
                show_playing: false,
                unconditional_spacer_after_meta: false,
                lines: &[],
                image: None,
            };
            paint_hero_content(f, content_rect, &hero_content, focused);
        }

        let (selector_tabs, row) = render_selector_rows(f, post_hero_area, post_hero_area.y);
        layout.selector_tabs = selector_tabs;
        let list_area = Rect {
            x: post_hero_area.x,
            y: row,
            width: post_hero_area.width,
            height: (post_hero_area.y + post_hero_area.height).saturating_sub(row),
        };

        layout.left_area = list_area;
        if list_area.height == 0 {
            return;
        }
        let hero_rows = if wide {
            hero_rows
        } else {
            (hero_rows >= HERO_BLOCK_EXTRA_ROWS && hero_rows < list_area.height)
                .then_some(hero_rows)
                .unwrap_or(0)
        };
        let visible = list_area.height as usize;
        let packed_cursor_row = packed
            .iter()
            .position(|row| matches!(row, PackedFeedRow::Items(idxs) if idxs.contains(&cursor)))
            .unwrap_or(0);
        let total_display = inline_display_row_count(packed.len(), packed_cursor_row, hero_rows);
        let (scroll, detail_screen_row) = if !wide && hero_rows > 0 {
            let flow = inline_detail_flow(
                packed_cursor_row,
                hero_rows,
                list_area.height,
                self.feed_tab.scroll,
            )
            .expect("admitted inline detail must fit");
            (flow.offset, Some(flow.detail_screen_row))
        } else {
            let lower_bound = packed_cursor_row.saturating_add(1).saturating_sub(visible);
            let upper_bound = packed_cursor_row.min(total_display.saturating_sub(visible));
            (self.feed_tab.scroll.clamp(lower_bound, upper_bound), None)
        };
        self.feed_tab.scroll = scroll;
        let cell_w = library_cell_width(list_area, cols);
        let row_w = list_area.width.saturating_sub(1);
        let visible_count = total_display.saturating_sub(scroll).min(visible);
        let mut row_map: Vec<Option<usize>> = Vec::with_capacity(list_area.height as usize);
        let entries = self.feed_tab.visible_entries();
        let mut row_y = list_area.y;

        for display_row in (scroll..total_display).take(visible) {
            if row_y >= list_area.y + list_area.height {
                break;
            }
            match inline_display_row(packed.len(), packed_cursor_row, hero_rows, display_row)
                .expect("visible row is within the replacement flow")
            {
                InlineDisplayRow::Replacement => {
                    row_map.push((display_row == packed_cursor_row).then_some(cursor));
                }
                InlineDisplayRow::Source(source_row) => match &packed[source_row] {
                    PackedFeedRow::Spacer => {
                        f.render_widget(
                            Paragraph::new(Line::default())
                                .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                            Rect {
                                x: list_area.x,
                                y: row_y,
                                width: row_w,
                                height: 1,
                            },
                        );
                        row_map.push(None);
                    }
                    PackedFeedRow::Heading(group) => {
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
                                y: row_y,
                                width: row_w,
                                height: 1,
                            },
                        );
                        row_map.push(None);
                    }
                    PackedFeedRow::Items(idxs) => {
                        for (cell_idx, &i) in idxs.iter().enumerate() {
                            let entry = &entries[i];
                            let selected = i == cursor;
                            let cell_x =
                                list_area.x + cell_idx as u16 * (cell_w + LIBRARY_COLUMN_GAP);
                            render_feed_entry_cell(
                                f,
                                entry,
                                cell_x,
                                row_y,
                                cell_w,
                                selected,
                                focused,
                                !selected || wide,
                            );
                        }
                        row_map.push(idxs.first().copied());
                    }
                },
            }
            row_y += 1;
        }
        row_map.resize(list_area.height as usize, None);
        layout.left_row_map = row_map;
        layout.left_item_rows = (0..total_display)
            .map(|display_row| {
                match inline_display_row(packed.len(), packed_cursor_row, hero_rows, display_row)
                    .expect("display row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => {
                        if display_row == packed_cursor_row {
                            vec![cursor]
                        } else {
                            Vec::new()
                        }
                    }
                    InlineDisplayRow::Source(source_row) => match &packed[source_row] {
                        PackedFeedRow::Items(idxs) => idxs.clone(),
                        PackedFeedRow::Spacer | PackedFeedRow::Heading(_) => Vec::new(),
                    },
                }
            })
            .collect();

        if visible_count > 0 && visible_count < total_display {
            render_right_scrollbar_with_viewport(
                f,
                list_area,
                total_display,
                visible_count,
                scroll,
                palette::SCROLLBAR,
            );
        }
        if wide {
            draw_column_selection_markers(f, list_area, cursor, &layout.left_item_rows, scroll);
        }
        if !wide {
            if let Some(detail_screen_row) = detail_screen_row {
                layout.hero_area = Rect {
                    x: list_area.x,
                    y: list_area.y + detail_screen_row as u16,
                    width: list_area.width,
                    height: hero_rows,
                };
                layout.inline_hero_area = layout.hero_area;
                layout.selected_item_rect = Some(layout.hero_area);
                selected_detail_shell(f, layout.hero_area, hero_rows, focused);
                let meta = feed_entry_meta_line(&selected_entry);
                paint_hero_content(
                    f,
                    Rect {
                        x: layout.hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                        y: layout.hero_area.y + 2,
                        width: layout
                            .hero_area
                            .width
                            .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                        height: hero_rows.saturating_sub(HERO_BLOCK_EXTRA_ROWS),
                    },
                    &HeroContent {
                        title: Some(selected_entry.title.as_str()),
                        meta_line: Some(meta.as_str()),
                        meta_color: palette::PLAYBACK_META_FG,
                        show_playing: false,
                        unconditional_spacer_after_meta: false,
                        lines: &[],
                        image: None,
                    },
                    focused,
                );
            }
        }
    }
}

/// Renders one feed entry into a `cell_w`-wide cell: selection marker
/// (painted separately, at the list's outer edge, by
/// `draw_column_selection_markers`), watched check, title, and duration.
/// The full metadata set (publish date, MIME type) lives in the hero for the
/// selected entry; list rows stay terse in both column counts, mirroring how
/// the movie/TV list rows stay terse while their hero holds the detail.
fn render_feed_entry_cell(
    f: &mut Frame,
    entry: &FeedEntry,
    x: u16,
    y: u16,
    cell_w: u16,
    selected: bool,
    focused: bool,
    show_title: bool,
) {
    if cell_w == 0 {
        return;
    }
    let bg = if selected {
        palette::resolve_surface_focus(focused)
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

    let duration = format_duration(entry.duration_ticks);
    let dur_str = if duration.is_empty() {
        String::new()
    } else {
        format!(" {duration}")
    };
    let dur_w = dur_str.chars().count();
    let prefix_w = 3usize; // leading space + watched check + space
    let title_w = (cell_w as usize).saturating_sub(prefix_w + dur_w);

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default().bg(bg))];
    spans.push(if entry.played {
        Span::styled("✓", Style::default().fg(palette::GREEN).bg(bg))
    } else {
        Span::styled(" ", Style::default().bg(bg))
    });
    let title = show_title
        .then(|| super::super::ui_util::trunc_str(&entry.title, title_w))
        .unwrap_or_default();
    spans.push(Span::styled(
        format!(" {title}"),
        Style::default().fg(fg).bg(bg).add_modifier(if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        }),
    ));
    if !dur_str.is_empty() {
        spans.push(Span::styled(
            dur_str,
            Style::default().fg(palette::PLAYBACK_META_FG).bg(bg),
        ));
    }
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let pad = (cell_w as usize).saturating_sub(used);
    if pad > 0 {
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            x,
            y,
            width: cell_w,
            height: 1,
        },
    );
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
