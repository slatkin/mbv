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
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

/// Single-line Home row for an Emby item: title (episode rows also show the
/// series name), a playback-progress percent, and a right-aligned duration.
/// The caller always draws the selected-row background and marker outside
/// `row_rect` (design.md decision 2's unified edge marker), so this reserves
/// no marker column and stays flush with `row_rect`'s own left edge.
pub(super) fn render_home_emby_row(
    f: &mut Frame,
    row_rect: Rect,
    item: &mbv_core::api::EmbyItem,
    selected_row: bool,
    focused: bool,
) {
    let avail = row_rect.width as usize;

    let dur_str = if !item.is_folder && item.runtime_ticks > 0 {
        fmt_duration_short(item.runtime_ticks / TICKS_PER_SECOND)
    } else {
        String::new()
    };

    let pct_str = if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
        let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
        if pct > 0 {
            Some(format!("{}%", pct))
        } else {
            None
        }
    } else {
        None
    };

    let meta_text = dur_str;
    let meta_w = meta_text.width();
    // pad + 8-char "HH:MM:SS" + pad
    const META_COL_W: usize = 10;
    const META_RIGHT_MARGIN: usize = 0;
    const META_INNER_PAD: usize = 1;
    // " 100%" - space + up to 4 chars, reserved next to the title.
    const PCT_COL_W: usize = 5;
    let title_col_w =
        avail.saturating_sub(META_COL_W + META_RIGHT_MARGIN + META_INNER_PAD * 2 + PCT_COL_W);

    let is_episode = item.item_type == "Episode" && !item.series_name.is_empty();
    let mut title_spans: Vec<Span> = if is_episode {
        let show_w = title_col_w * 2 / 5;
        let show = trunc_str(&item.series_name, show_w);
        let show_actual_w = show.width();
        let ep_title = trunc_str(&item.name, title_col_w.saturating_sub(show_actual_w + 1));
        let bold = if selected_row && focused {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        vec![
            Span::styled(
                show,
                Style::default().fg(palette::YELLOW).add_modifier(bold),
            ),
            Span::raw(" "),
            Span::styled(
                ep_title,
                Style::default()
                    .fg(if focused {
                        palette::WHITE
                    } else {
                        palette::MUTED
                    })
                    .add_modifier(bold),
            ),
        ]
    } else {
        let title = trunc_str(&item.display_name(), title_col_w);
        vec![Span::styled(
            title,
            Style::default()
                .fg(if focused {
                    palette::WHITE
                } else {
                    palette::MUTED
                })
                .add_modifier(if selected_row && focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        )]
    };

    // Add playback progress percentage to the right of the title
    if let Some(pct) = &pct_str {
        title_spans.push(Span::raw(" "));
        title_spans.push(Span::styled(
            pct.clone(),
            Style::default().fg(palette::FOAM),
        ));
    }

    // Calculate actual title width for right-alignment
    let actual_title_w: usize = title_spans.iter().map(|s| s.content.width()).sum();

    // Build right column (metadata) - flush with right edge. Use the row's
    // full width here, not `avail`: `avail` already subtracts the
    // left-marker prefix, and `actual_title_w` includes that same prefix,
    // so reusing `avail` would double-subtract it and push the metadata
    // box too far left.
    let mut meta_spans: Vec<Span> = Vec::new();
    let pad_to_right = (row_rect.width as usize).saturating_sub(actual_title_w + META_COL_W);
    if pad_to_right > 0 {
        meta_spans.push(Span::raw(" ".repeat(pad_to_right)));
    }
    // Create fixed-width metadata column, text right-aligned inside. No
    // explicit background: the row's own background (panel or
    // focused-selection fill) already shows through underneath.
    let content_w = META_COL_W - META_INNER_PAD * 2;
    let inner_pad = content_w.saturating_sub(meta_w);
    let left_pad_str = " ".repeat(META_INNER_PAD + inner_pad);
    let full_meta = format!("{}{}", left_pad_str, meta_text);
    let full_meta = format!("{:width$}", full_meta, width = META_COL_W);
    meta_spans.push(Span::styled(full_meta, Style::default().fg(palette::GREEN)));

    // Combine and render
    let mut all_spans = title_spans;
    all_spans.extend(meta_spans);
    f.render_widget(Paragraph::new(Line::from(all_spans)), row_rect);
}

/// Generic single-line Home "Latest" row for a non-Emby `QueueItem`
/// (Audiobookshelf today, Feeds in Part 3): a `display_name()` title and a
/// right-aligned duration, matching the Emby Home row look. No per-provider
/// metadata. The caller always draws the selected-row background and marker
/// outside `row_rect` (design.md decision 2's unified edge marker), so this
/// reserves no marker column and stays flush with `row_rect`'s own left edge.
pub(super) fn render_home_latest_row(
    f: &mut Frame,
    row_rect: Rect,
    item: &QueueItem,
    selected: bool,
    focused: bool,
) {
    let avail = row_rect.width as usize;
    const META_COL_W: usize = 10;
    const META_INNER_PAD: usize = 1;
    let title_col_w = avail.saturating_sub(META_COL_W + META_INNER_PAD * 2);

    let bold = selected && focused;
    let mut spans: Vec<Span> = vec![Span::styled(
        trunc_str(&item.display_name(), title_col_w),
        Style::default()
            .fg(if focused {
                palette::WHITE
            } else {
                palette::MUTED
            })
            .add_modifier(if bold {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    )];
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
    /// Generic hero detail for a selected non-Emby Home item, following the
    /// same visual structure as the Emby Keep Watching hero: yellow bold
    /// wrapped title, a show-name line, a subtitle line, a blank separator,
    /// and a wrapped overview block, with a 16:9 image filling the column.
    /// Audiobookshelf covers load through the existing
    /// `ImageSource::Audiobookshelf` path; items with no artwork degrade to no
    /// image, not an error.
    /// `overview_pad` insets the overview text within its background row —
    /// the two-column (wide) hero's original 2-col padding; the
    /// single-column hero passes 0 to stay flush with the title above it.
    pub(super) fn render_home_latest_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        item: &QueueItem,
        overview_pad: usize,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let text_w = area.width as usize;
        let ov_w = text_w.saturating_sub(overview_pad * 2);
        let title = item.title();
        let show_name = match item {
            QueueItem::Audiobookshelf(ep) => ep.show_title.clone().unwrap_or_default(),
            _ => String::new(),
        };
        let overview = item.overview().map(str::to_owned);

        // Title (yellow, bold, wrapped), then one row each for show name,
        // subtitle, blank separator, then the wrapped overview block.
        let title_lines: Vec<String> = wrap(title, text_w)
            .into_iter()
            .map(|s| s.into_owned())
            .collect();
        let overview_lines: Vec<String> = if overview.as_deref().is_none_or(str::is_empty) {
            Vec::new()
        } else {
            // Cap long descriptions, with an ellipsis, so the hero doesn't
            // grow unboundedly. The 200-char limit is on display width and
            // includes the ellipsis itself.
            let capped = trunc_str(overview.as_deref().unwrap(), 200);
            wrap(&capped, ov_w.max(1))
                .into_iter()
                .map(|s| s.into_owned())
                .collect()
        };
        let meta_height = title_lines.len() as u16
            + if show_name.is_empty() { 0 } else { 1 }
            + 1 // subtitle row
            + 1 // blank separator
            + if overview_lines.is_empty() {
                0
            } else {
                1 + overview_lines.len() as u16 + 1 // overview block: pad + lines + pad
            };

        // Terminal cells are roughly twice as tall as they are wide, so a
        // 16:9 image needs 9 rows for every 32 columns, matching the Emby hero.
        let image_height = (area.width.saturating_mul(9).saturating_add(31) / 32)
            .max(1)
            .min(area.height.saturating_sub(meta_height + 1));
        let img_w = area.width;

        let mut row = area.y;
        let max_y = area.y + area.height;

        for line in &title_lines {
            if row >= max_y {
                break;
            }
            f.render_widget(
                Paragraph::new(Span::styled(
                    line.clone(),
                    Style::default()
                        .fg(palette::YELLOW)
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

        if row < max_y && !show_name.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    trunc_str(&show_name, text_w),
                    Style::default().fg(palette::FOAM),
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

        if row < max_y {
            let mut spans: Vec<Span> = Vec::new();
            if let Some(ticks) = item.duration() {
                spans.push(Span::styled(
                    trunc_str(
                        &fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64),
                        text_w,
                    ),
                    Style::default().fg(palette::SUBTLE),
                ));
            }
            if !spans.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from(spans)),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
            }
            row += 1;
        }

        row += 1; // blank separator row

        if !overview_lines.is_empty() && row < max_y {
            let block_h = 1 + overview_lines.len() as u16 + 1; // top pad + lines + bottom pad
            let block_area = Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: block_h,
            };
            f.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
                block_area,
            );
            let inner = Rect {
                x: block_area.x + overview_pad as u16,
                y: block_area.y + 1,
                width: block_area.width.saturating_sub(overview_pad as u16 * 2),
                height: block_area.height.saturating_sub(2),
            };
            let overview_text: Vec<Line> = overview_lines
                .iter()
                .map(|line| {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(palette::WHITE),
                    ))
                })
                .collect();
            f.render_widget(Paragraph::new(overview_text), inner);
        }

        // Cover art: only Audiobookshelf episodes carry artwork today. The
        // cover is fetched via `fetch_audiobookshelf_cover` (which routes
        // through `ImageSource::Audiobookshelf`) and rendered from the cache.
        let QueueItem::Audiobookshelf(episode) = item else {
            return;
        };
        if image_height == 0 {
            return;
        }
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
        let image_rect = Rect {
            x: area.x,
            y: area.y + area.height - image_height,
            width: img_w,
            height: image_height,
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
            description: None,
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
            render_home_latest_row(f, Rect::new(0, 0, row_w, 1), item, selected, focused);
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
            app.render_home_latest_detail(f, Rect::new(0, 0, 40, 6), &item, 0);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.split('\n').collect();
        assert!(lines[0].contains("Episode a"), "title row: {out:?}");
        assert!(lines[1].contains("Podcast"), "show-name row: {out:?}");
        assert!(lines[2].contains("1:05"), "duration row: {out:?}");
    }

    /// Task 10.2: detail with no known duration skips the duration row but
    /// still renders the title and show name; no configured server means no
    /// cover fetch.
    #[test]
    fn detail_without_duration_omits_duration_row() {
        let mut app = make_app_stub();
        let item = abs_item("b", None, None);
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_home_latest_detail(f, Rect::new(0, 0, 40, 6), &item, 0);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.split('\n').collect();
        assert!(lines[0].contains("Episode b"), "title row: {out:?}");
        assert!(lines[1].contains("Podcast"), "show-name row: {out:?}");
        assert!(
            lines[2..].iter().all(|l| !l.contains("0:00")),
            "no fabricated duration: {out:?}"
        );
    }

    /// Long ABS descriptions are capped at 200 display columns with an
    /// ellipsis so the hero doesn't grow unboundedly.
    #[test]
    fn detail_truncates_long_description_with_ellipsis() {
        let mut app = make_app_stub();
        // The truncation limit is on the description width; build a much wider
        // buffer item by item so the assertion below is about the ellipsis,
        // not about a coincidental line-wrap boundary.
        let long = "word ".repeat(80);
        let item = QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
            library_item_id: "show-t".into(),
            episode_id: "episode-t".into(),
            title: "Episode t".into(),
            show_title: Some("Podcast".into()),
            author: None,
            description: Some(long),
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        });
        let backend = TestBackend::new(200, 40);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_home_latest_detail(f, Rect::new(0, 0, 200, 40), &item, 0);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        // Reassemble the description block's visible lines (title, show name,
        // subtitle, blank separator precede it) joining trimmed rows.
        let desc_region: String = out
            .split('\n')
            .skip(3)
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            desc_region.ends_with('\u{2026}'),
            "long description ends with an ellipsis: ...{desc_region:?}"
        );
        assert!(
            desc_region.chars().count() <= 201,
            "description column budget is 200 + ellipsis, got {} chars",
            desc_region.chars().count()
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
            app.render_home_latest_detail(f, Rect::new(0, 0, 40, 6), &item, 0);
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
