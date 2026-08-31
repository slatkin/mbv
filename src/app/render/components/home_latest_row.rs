use crate::app::render::components::hero::{paint_hero_content, HeroContent};
use crate::app::ui_util::*;
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

fn home_title_color(selected: bool, focused: bool) -> Color {
    if selected {
        palette::TEXT_FOCUS_ACCENT
    } else if focused {
        palette::TEXT_STRONG
    } else {
        palette::TEXT_MUTED
    }
}

/// Single-line Home row for an Emby item: title (episode rows also show the
/// series name), a playback-progress percent, and a right-aligned duration.
/// The caller always draws the selected-row background and marker outside
/// `row_rect` (design.md decision 2's unified edge marker), so this reserves
/// no marker column and stays flush with `row_rect`'s own left edge.
pub(in crate::app::render) fn render_home_emby_row(
    f: &mut Frame,
    row_rect: Rect,
    item: &mbv_core::api::EmbyItem,
    selected_row: bool,
    focused: bool,
    show_title: bool,
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
    // " 100%" - space + up to 4 chars, reserved next to the title only when
    // a percentage badge will actually be drawn (most rows have none, and
    // reserving it unconditionally truncated titles into unused blank space).
    const PCT_COL_W: usize = 5;
    let pct_reserve = if pct_str.is_some() { PCT_COL_W } else { 0 };
    let title_col_w =
        avail.saturating_sub(META_COL_W + META_RIGHT_MARGIN + META_INNER_PAD * 2 + pct_reserve);

    let is_episode = item.item_type == "Episode" && !item.series_name.is_empty();
    let mut title_spans: Vec<Span> = if !show_title {
        Vec::new()
    } else if is_episode {
        let show_w = title_col_w * 2 / 5;
        let show = trunc_str(&item.series_name, show_w);
        let show_actual_w = show.width();
        let ep_title = trunc_str(&item.name, title_col_w.saturating_sub(show_actual_w + 1));
        vec![
            Span::styled(show, Style::default().fg(palette::TEXT_METADATA)),
            Span::raw(" "),
            Span::styled(
                ep_title,
                Style::default().fg(home_title_color(selected_row, focused)),
            ),
        ]
    } else {
        let title = trunc_str(&item.display_name(), title_col_w);
        vec![Span::styled(
            title,
            Style::default().fg(home_title_color(selected_row, focused)),
        )]
    };

    // Add playback progress percentage to the right of the title
    if let Some(pct) = &pct_str {
        title_spans.push(Span::raw(" "));
        title_spans.push(Span::styled(
            pct.clone(),
            Style::default().fg(palette::TEXT_METADATA),
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
    meta_spans.push(Span::styled(
        full_meta,
        Style::default().fg(palette::STATUS_AVAILABLE),
    ));

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
pub(in crate::app::render) fn render_home_latest_row(
    f: &mut Frame,
    row_rect: Rect,
    item: &QueueItem,
    selected: bool,
    focused: bool,
    show_title: bool,
) {
    let avail = row_rect.width as usize;
    const META_COL_W: usize = 10;
    const META_INNER_PAD: usize = 1;
    let title_col_w = avail.saturating_sub(META_COL_W + META_INNER_PAD * 2);

    let mut spans: Vec<Span> = vec![Span::styled(
        if show_title {
            trunc_str(&item.display_name(), title_col_w)
        } else {
            Default::default()
        },
        Style::default().fg(home_title_color(selected, focused)),
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
    spans.push(Span::styled(
        full_meta,
        Style::default().fg(palette::STATUS_AVAILABLE),
    ));
    f.render_widget(Paragraph::new(Line::from(spans)), row_rect);
}

/// Pre-wrapped text for the generic Home hero detail (title lines, optional
/// show-name row, overview lines) plus the total row count the text needs.
/// Computed once (mirroring `KeepWatchingHeroLayout`'s measure-before-render
/// pattern) so `render_home_list` can size the narrow hero to its content
/// instead of asking for the whole content area, and the renderer can wrap
/// each piece exactly once per frame.
pub(in crate::app::render) struct HomeLatestDetailText {
    pub(in crate::app::render) title_lines: Vec<String>,
    pub(in crate::app::render) show_name: String,
    pub(in crate::app::render) overview_lines: Vec<(String, bool)>,
    pub(in crate::app::render) meta_height: u16,
}

/// Measures/wraps the generic hero detail's text for `item` at the given
/// content width. `text_w` is where the title wraps; `ov_w` where the
/// overview wraps (`overview_pad` inset in the wide hero). The overview's
/// `false` flag is irrelevant here (no beside-image wrap-around in this
/// layout) but kept so the renderer can pass the lines straight through.
pub(in crate::app::render) fn home_latest_detail_text(
    item: &QueueItem,
    text_w: usize,
    ov_w: usize,
) -> HomeLatestDetailText {
    let title_lines: Vec<String> = wrap(item.title(), text_w)
        .into_iter()
        .map(|s| s.into_owned())
        .collect();
    let show_name = match item {
        QueueItem::Audiobookshelf(ep) => ep.show_title.clone().unwrap_or_default(),
        _ => String::new(),
    };
    let overview_lines: Vec<(String, bool)> = match item.overview() {
        None | Some("") => Vec::new(),
        Some(overview) => {
            // Strip URLs (an unbroken URL is one giant unbreakable word to
            // the wrapper below) and cap long descriptions, with an
            // ellipsis, matching the 600-char cap Emby's home-video/podcast
            // library views already use (`trunc_overview`) -- podcast
            // overviews routinely carry ad copy that would otherwise grow
            // the hero unboundedly.
            let capped = trunc_overview(overview);
            wrap(&capped, ov_w.max(1))
                .into_iter()
                .map(|s| (s.into_owned(), false))
                .collect()
        }
    };
    let meta_height = title_lines.len() as u16
        + if show_name.is_empty() { 0 } else { 1 }
        + 1 // meta row
        + 1 // blank separator
        + if overview_lines.is_empty() {
            0
        } else {
            1 + overview_lines.len() as u16 + 1 // overview block: pad + lines + pad
        };
    HomeLatestDetailText {
        title_lines,
        show_name,
        overview_lines,
        meta_height,
    }
}

impl App {
    /// Triggers the Audiobookshelf cover fetch for `library_item_id` and
    /// returns its image cache key, or `None` with no server configured.
    /// Shared by every generic-hero cover: the two-column/Feed stacked-below
    /// detail (`render_home_latest_detail_content`) and the narrow beside-image
    /// hero (`home.rs`'s `HeroData::GenericBeside`).
    pub(in crate::app::render) fn audiobookshelf_cover_key(
        &mut self,
        library_item_id: &str,
    ) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        if self.images_enabled() {
            self.fetch_audiobookshelf_cover(setup.server_url.clone(), library_item_id.to_string());
        }
        Some(images::audiobookshelf_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.current_protocol_suffix(),
        ))
    }

    /// Triggers the Audiobookshelf book-cover fetch for `library_item_id` and
    /// returns its isolated image cache key, or `None` with no server configured.
    /// The book-browsing spec requires book artwork to remain isolated from
    /// podcast artwork (line 124).
    pub(in crate::app::render) fn audiobookshelf_book_cover_key(
        &mut self,
        library_item_id: &str,
    ) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        if self.images_enabled() {
            self.fetch_audiobookshelf_book_cover(
                setup.server_url.clone(),
                library_item_id.to_string(),
            );
        }
        Some(images::audiobookshelf_book_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.current_protocol_suffix(),
        ))
    }
}

/// Renders the generic Home hero detail's non-image content (title/meta/
/// overview, or the whole no-image hero for a Feed) without `App`, returning
/// the Audiobookshelf cover's target `Rect` (if any) still needing paint.
/// Shared by Home's render paths (task 3.4's confirmed extraction).
pub(in crate::app::render) fn render_home_latest_detail_content(
    f: &mut Frame,
    area: Rect,
    item: &QueueItem,
    focused: bool,
    overview_pad: usize,
) -> Option<super::home_hero::HomeImagePaint> {
    if area.width == 0 || area.height == 0 {
        return None;
    }

    // Feed entries have no artwork; use the shared Model A no-image hero
    // rather than the legacy Home metadata painter. Audiobookshelf keeps
    // the beside-image path below because its cover is a wide thumbnail.
    if !matches!(item, QueueItem::Audiobookshelf(_)) {
        let meta = item
            .duration()
            .map(|ticks| fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64));
        let content = HeroContent {
            title: Some(item.title()),
            meta_line: meta.as_deref(),
            meta_color: palette::TEXT_SECONDARY,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &[],
            image: None,
        };
        paint_hero_content(f, area, &content, focused);
        return None;
    }

    let text_w = area.width as usize;
    let ov_w = text_w.saturating_sub(overview_pad * 2);
    let HomeLatestDetailText {
        title_lines,
        show_name,
        overview_lines,
        meta_height,
    } = home_latest_detail_text(item, text_w, ov_w);

    // Terminal cells are roughly twice as tall as they are wide, so a
    // 16:9 image needs 9 rows for every 32 columns, matching the Emby hero.
    let image_height = (area.width.saturating_mul(9).saturating_add(31) / 32)
        .max(1)
        .min(area.height.saturating_sub(meta_height + 1));
    let img_w = area.width;

    super::hero::render_home_hero_meta_block(
        f,
        area,
        area,
        &title_lines,
        &show_name,
        None,
        item.duration()
            .map(|ticks| {
                vec![vec![Span::styled(
                    trunc_str(
                        &fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64),
                        text_w,
                    ),
                    Style::default().fg(palette::TEXT_SECONDARY),
                )]]
            })
            .unwrap_or_default(),
        &overview_lines,
        overview_pad as u16,
        focused,
    );

    // Cover art: only Audiobookshelf episodes carry artwork today.
    let QueueItem::Audiobookshelf(episode) = item else {
        return None;
    };
    if image_height == 0 {
        return None;
    }
    Some(super::home_hero::HomeImagePaint::AudiobookshelfCover {
        area: Rect {
            x: area.x,
            y: area.y + area.height - image_height,
            width: img_w,
            height: image_height,
        },
        library_item_id: episode.library_item_id.clone(),
        show_placeholder: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render::test_helpers::buffer_to_string;
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
            render_home_latest_row(f, Rect::new(0, 0, row_w, 1), item, selected, focused, true);
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
        let item = abs_item(
            "a",
            Some(65 * TICKS_PER_SECOND as u64),
            Some("cover.jpg".into()),
        );
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let _ = render_home_latest_detail_content(f, Rect::new(0, 0, 40, 6), &item, true, 0);
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
        let item = abs_item("b", None, None);
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let _ = render_home_latest_detail_content(f, Rect::new(0, 0, 40, 6), &item, true, 0);
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

    /// Long ABS descriptions are capped at 600 display columns with an
    /// ellipsis so the hero doesn't grow unboundedly.
    #[test]
    fn detail_truncates_long_description_with_ellipsis() {
        // The truncation limit is on the description width; build a much wider
        // buffer item by item so the assertion below is about the ellipsis,
        // not about a coincidental line-wrap boundary.
        let long = "word ".repeat(160);
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
            let _ = render_home_latest_detail_content(f, Rect::new(0, 0, 200, 40), &item, true, 0);
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
            desc_region.chars().count() <= 601,
            "description column budget is 600 + ellipsis, got {} chars",
            desc_region.chars().count()
        );
    }

    /// The generic hero measure sizes the hero to its text content, not the
    /// available area: a text-only Feed measures a few rows (title + meta
    /// row + separator), and an ABS item with a show name adds its subtitle
    /// row. The narrow Home layout builds the hero height from this measure
    /// (plus the 16:9 cover slot for ABS), so a bare Feed no longer fills
    /// the whole content area.
    #[test]
    fn generic_detail_measure_sizes_to_text_content() {
        let feed = feed_item("1");
        let text = home_latest_detail_text(&feed, 100, 100);
        assert_eq!(text.meta_height, 3, "Feed hero = title + meta + separator");

        let abs = abs_item("1", None, None);
        let text = home_latest_detail_text(&abs, 100, 100);
        assert_eq!(
            text.meta_height, 4,
            "ABS hero = title + show name + meta + separator"
        );
        assert_eq!(text.show_name, "Podcast");
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
        let item = feed_item("2");
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let _ = render_home_latest_detail_content(f, Rect::new(0, 0, 40, 6), &item, true, 0);
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
