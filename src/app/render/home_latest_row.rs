use super::super::ui_util::*;
use super::RENDER_FILTER;

use crate::app::{images, palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// Generic single-line Home "Latest" row for a non-Emby `QueueItem`
/// (Audiobookshelf today, Feeds in Part 3): a `display_name()` title and a
/// right-aligned duration with marker/selection styling matching the existing
/// Emby Home row look. No per-provider metadata.
pub(super) fn render_home_latest_row(
    f: &mut Frame,
    row_rect: Rect,
    item: &QueueItem,
    selected: bool,
    focused: bool,
    wide_unfocused: bool,
) {
    let avail = (row_rect.width as usize).saturating_sub(2);
    const META_COL_W: usize = 10;
    const META_INNER_PAD: usize = 1;
    let title_col_w = avail.saturating_sub(META_COL_W + META_INNER_PAD * 2);

    let bold = selected && focused;
    let mut spans: Vec<Span> = vec![
        if selected && focused {
            Span::styled("▍", Style::default().fg(palette::AQUA))
        } else {
            Span::raw(" ")
        },
        Span::raw(" "),
        Span::styled(
            trunc_str(&item.display_name(), title_col_w),
            Style::default()
                .fg(if wide_unfocused {
                    palette::MUTED
                } else {
                    palette::WHITE
                })
                .add_modifier(if bold {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ];
    let actual_title_w: usize = spans.iter().map(|s| s.content.width()).sum();

    let meta_text = item
        .duration()
        .map(|ticks| fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64))
        .unwrap_or_default();
    let meta_w = meta_text.width();
    let pad_to_right = (row_rect.width as usize).saturating_sub(actual_title_w + META_COL_W);
    if pad_to_right > 0 {
        spans.push(Span::raw(" ".repeat(pad_to_right)));
    }
    let content_w = META_COL_W - META_INNER_PAD * 2;
    let inner_pad = content_w.saturating_sub(meta_w);
    let full_meta = format!(
        "{:width$}",
        format!("{}{}", " ".repeat(META_INNER_PAD + inner_pad), meta_text),
        width = META_COL_W
    );
    spans.push(Span::styled(full_meta, Style::default().fg(palette::GREEN)));
    f.render_widget(Paragraph::new(Line::from(spans)), row_rect);
}

impl App {
    /// Minimal generic detail for a selected non-Emby Home item (Task 9.2):
    /// title, duration if known, and cover art when one resolves. Audiobookshelf
    /// covers load through the existing `ImageSource::Audiobookshelf` path
    /// (Task 9.3); items with no artwork degrade to no image, not an error.
    pub(super) fn render_home_latest_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        item: &QueueItem,
    ) {
        let mut row = area.y;
        let max_y = area.y + area.height;

        if row < max_y {
            f.render_widget(
                Paragraph::new(Span::styled(
                    trunc_str(&item.display_name(), area.width as usize),
                    Style::default()
                        .fg(palette::WHITE)
                        .add_modifier(Modifier::BOLD),
                )),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }
        if let Some(ticks) = item.duration() {
            if row < max_y {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64),
                        Style::default().fg(palette::SUBTLE),
                    )),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
                row += 1;
            }
        }

        // Cover art: only Audiobookshelf episodes carry artwork today. The
        // cover is fetched via `fetch_audiobookshelf_cover` (which routes
        // through `ImageSource::Audiobookshelf`) and rendered from the cache.
        let QueueItem::Audiobookshelf(episode) = item else {
            return;
        };
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone();
        let Some(setup) = setup else {
            return;
        };
        if self.images_enabled() {
            self.fetch_audiobookshelf_cover(
                setup.server_url.clone(),
                episode.library_item_id.clone(),
            );
        }
        let image_key = images::audiobookshelf_cover_cache_key(
            &setup.server_url,
            &episode.library_item_id,
            self.current_protocol_suffix(),
        );
        let Some(image) = self.cached_image_protocol_mut(&image_key) else {
            return;
        };
        let img_w = (area.width / 3).clamp(8, 40);
        let img_h = (img_w.saturating_mul(9).saturating_add(31) / 32).max(1);
        let image_rect = Rect {
            x: area.x,
            y: row.saturating_add(1),
            width: img_w,
            height: img_h,
        };
        type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
        f.render_stateful_widget(
            SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
            image_rect,
            image,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::buffer_to_string;
    use super::*;
    use crate::app::tests::make_app_stub;
    use mbv_core::playback_queue::{AudiobookshelfQueueItem, FeedEntry};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn feed_item(id: &str) -> QueueItem {
        QueueItem::Feed(FeedEntry {
            guid: format!("guid-{id}"),
            title: format!("Feed entry {id}"),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: Some(mbv_core::config::FeedKind::Audio),
            feed_id: None,
            position_ticks: 0,
            played: false,
        })
    }

    fn abs_item(id: &str, duration_ticks: Option<u64>, cover_path: Option<String>) -> QueueItem {
        QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
            library_item_id: format!("show-{id}"),
            episode_id: format!("episode-{id}"),
            title: format!("Episode {id}"),
            show_title: Some("Podcast".into()),
            author: None,
            duration_ticks,
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path,
        })
    }

    fn render_row(row_w: u16, item: &QueueItem, selected: bool, focused: bool) -> String {
        let backend = TestBackend::new(row_w, 1);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_home_latest_row(f, Rect::new(0, 0, row_w, 1), item, selected, focused, false);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    /// Task 10.3: a known duration right-aligns in the fixed-width metadata
    /// column; the title truncates to fit.
    #[test]
    fn row_right_aligns_duration_and_truncates_title() {
        let item = abs_item("1", Some(42 * TICKS_PER_SECOND as u64), None);
        let out = render_row(80, &item, true, true);
        let line = out.split('\n').next().unwrap();
        assert!(
            line.trim_end().ends_with("0:42"),
            "expected duration right-aligned, got: {line:?}"
        );
        assert!(line.contains("Podcast - Episode 1"));
        assert!(line.trim_start().starts_with('▍'), "selected marker");
    }

    /// Task 10.1: an item with no known duration leaves the metadata column
    /// empty rather than rendering a bogus 0:00.
    #[test]
    fn row_without_duration_has_empty_meta() {
        let item = abs_item("2", None, None);
        let out = render_row(40, &item, false, true);
        let line = out.split('\n').next().unwrap();
        let after_title = line.trim_start().trim_start_matches("Podcast - Episode 2");
        assert!(
            after_title.trim().is_empty(),
            "no meta text expected, got: {line:?}"
        );
    }

    /// Task 10.1: an unselected / unfocused row drops the marker glyph.
    #[test]
    fn row_unselected_has_no_marker() {
        let item = abs_item("3", None, None);
        let out = render_row(20, &item, false, false);
        let line = out.split('\n').next().unwrap();
        assert!(
            !line.contains('▍'),
            "no marker without selection, got: {line:?}"
        );
    }

    /// Task 10.2: the generic detail shows the title and, when known, the
    /// duration; a missing cover or unknown duration degrades gracefully
    /// rather than panicking or rendering an empty duration row.
    #[test]
    fn detail_shows_title_and_duration_when_known() {
        let mut app = make_app_stub();
        let item = abs_item(
            "a",
            Some(65 * TICKS_PER_SECOND as u64),
            Some("cover.jpg".into()),
        );
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_home_latest_detail(f, Rect::new(0, 0, 40, 6), &item);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.split('\n').collect();
        assert!(
            lines[0].contains("Podcast - Episode a"),
            "title row: {out:?}"
        );
        assert!(lines[1].contains("1:05"), "duration row: {out:?}");
    }

    /// Task 10.2: detail with no known duration skips the duration row but
    /// still renders the title; no configured server means no cover fetch.
    #[test]
    fn detail_without_duration_omits_duration_row() {
        let mut app = make_app_stub();
        let item = abs_item("b", None, None);
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_home_latest_detail(f, Rect::new(0, 0, 40, 6), &item);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.split('\n').collect();
        assert!(
            lines[0].contains("Podcast - Episode b"),
            "title row: {out:?}"
        );
        assert!(
            lines[1..].iter().all(|l| !l.contains("0:00")),
            "no fabricated duration: {out:?}"
        );
    }

    /// Task 14.3: a Feed entry in the generic row renderer shows its title
    /// with an empty metadata column (no known duration), matching the no-
    /// duration Audiobookshelf row. No artwork is ever attempted for a Feed.
    #[test]
    fn feed_row_without_duration_has_empty_meta() {
        let item = feed_item("1");
        let out = render_row(40, &item, false, true);
        let line = out.split('\n').next().unwrap();
        assert!(line.contains("Feed entry 1"), "title: {line:?}");
        let after_title = line.trim_start().trim_start_matches("Feed entry 1");
        assert!(
            after_title.trim().is_empty(),
            "no meta text expected for a Feed with unknown duration: {line:?}"
        );
    }

    /// Task 14.3: the generic detail renders a Feed entry's title with no
    /// duration row and no cover (the cover-fetch branch only ever runs for
    /// Audiobookshelf items), never panicking.
    #[test]
    fn feed_detail_renders_title_without_duration_or_artwork() {
        let mut app = make_app_stub();
        let item = feed_item("2");
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_home_latest_detail(f, Rect::new(0, 0, 40, 6), &item);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.split('\n').collect();
        assert!(lines[0].contains("Feed entry 2"), "title row: {out:?}");
        assert!(
            lines[1..].iter().all(|l| !l.contains("0:00")),
            "no fabricated duration: {out:?}"
        );
        assert!(
            !out.contains("image"),
            "no image path reached for a Feed entry: {out:?}"
        );
    }
}
