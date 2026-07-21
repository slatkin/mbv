use super::super::super::super::palette;
use super::super::super::super::ui_util::trunc_overview;
use super::super::super::super::App;
use super::meta::{
    library_folder_count, library_is_audio, library_is_episode_like, library_is_generic_folder,
    library_meta_line, library_title_line,
};
use super::{LibraryTableContext, LIB_AUDIO_IMG_W, LIB_EPISODE_IMG_W, LIB_SELECTED_IMG_W};
use mbv_core::api::MediaItem;
use ratatui::layout::{Constraint, Layout, Rect, Size};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use textwrap::wrap;

impl App {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_library_table_row(
        &mut self,
        f: &mut Frame,
        area: Rect,
        row_y: u16,
        row_h: u16,
        sep_w: u16,
        item: &MediaItem,
        abs_idx: usize,
        cursor: usize,
        ctx: &LibraryTableContext,
    ) {
        let selected = abs_idx == cursor;
        let is_audio = library_is_audio(item);
        let is_album_folder = ctx.at_album_folders && item.is_folder;
        let show_img = selected && (ctx.images_enabled || is_audio || is_album_folder);
        let row_w = if sep_w < area.width {
            area.width.saturating_sub(1)
        } else {
            area.width
        };
        let row_rect = Rect {
            x: area.x,
            y: row_y,
            width: row_w,
            height: row_h,
        };

        let content_area = Rect {
            height: row_h.saturating_sub(1),
            ..row_rect
        };
        let cache_key = format!("{}:{}", item.id, crate::config::IMAGE_CACHE_SUFFIX_LIBRARY);
        let img_actual = self.library_row_image_actual(item, show_img, content_area, ctx);
        let (ind_rect, text_rect, img_rect_opt) =
            self.library_row_rects(content_area, img_actual, is_audio, is_album_folder);
        let content_w = text_rect.width as usize;
        let is_episode_like = library_is_episode_like(item, ctx.is_feed_lib);

        if selected
            && !is_album_folder
            && !matches!(
                item.item_type.as_str(),
                "Movie" | "Series" | "Season" | "Episode"
            )
            && !is_episode_like
        {
            let bar: Vec<Line> = (0..ind_rect.height)
                .map(|_| Line::from(Span::styled("▌", Style::default().fg(palette::PINE))))
                .collect();
            f.render_widget(Paragraph::new(bar), ind_rect);
        }

        if let Some(img_rect) = img_rect_opt {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Fit(Some(
                        ratatui_image::FilterType::Lanczos3,
                    ))),
                    img_rect,
                    state,
                );
            }
        }

        let text_color = if ctx.now_playing_id.as_deref() == Some(item.id.as_str()) {
            palette::GREEN
        } else if selected {
            palette::IRIS
        } else {
            palette::TEXT
        };
        let artist_line = if is_audio && !item.artist.is_empty() {
            Some(item.artist.clone())
        } else {
            None
        };
        let meta_line =
            library_meta_line(self, item, is_audio, is_album_folder, is_episode_like, ctx);
        let overview_lines = library_overview_lines(item, selected, content_w, is_audio, ctx);
        let tech_line = library_tech_line(item, selected, is_audio);
        let artist_extra: usize = usize::from(artist_line.is_some());
        let tech_lines: usize = usize::from(!tech_line.is_empty());
        let base_lines = library_base_lines(item, is_album_folder, artist_extra, tech_lines);
        let dir_lines: usize = if selected && item.item_type == "Movie" && !item.director.is_empty()
        {
            2
        } else {
            0
        };
        let line_count =
            (base_lines + overview_lines.len() + dir_lines).min(text_rect.height as usize);
        if line_count == 0 {
            return;
        }

        let v_offset = if is_audio && selected {
            (text_rect.height as usize).saturating_sub(line_count) / 2
        } else {
            0
        };
        let centered_text_rect = Rect {
            y: text_rect.y + v_offset as u16,
            height: text_rect.height.saturating_sub(v_offset as u16),
            ..text_rect
        };
        let constraints: Vec<Constraint> = (0..line_count).map(|_| Constraint::Length(1)).collect();
        let line_rects = Layout::vertical(constraints).split(centered_text_rect);

        self.render_library_title(
            f,
            line_rects[0],
            item,
            content_w,
            text_color,
            selected,
            ctx.at_music_groups,
        );
        if let Some(ref a) = artist_line {
            if line_count >= 2 {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        a.as_str(),
                        Style::default().fg(palette::SUBTLE),
                    )),
                    line_rects[1],
                );
            }
            if line_count >= 3 {
                f.render_widget(Paragraph::new(meta_line), line_rects[2]);
            }
        } else if line_count >= 2 {
            f.render_widget(Paragraph::new(meta_line), line_rects[1]);
        }
        if tech_lines > 0 {
            let ti = 2 + artist_extra;
            if ti < line_count {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        tech_line.as_str(),
                        Style::default().fg(palette::SUBTLE),
                    )),
                    line_rects[ti],
                );
            }
        }
        for (j, ov_line) in overview_lines.iter().enumerate() {
            let idx = base_lines + j;
            if idx >= line_count {
                break;
            }
            f.render_widget(
                Paragraph::new(Span::styled(
                    ov_line.as_str(),
                    Style::default().fg(palette::WHITE),
                )),
                line_rects[idx],
            );
        }
        if dir_lines > 0 {
            let dir_idx = base_lines + overview_lines.len() + 1;
            if dir_idx < line_count {
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("Director: ", Style::default().fg(palette::SUBTLE)),
                        Span::styled(item.director.clone(), Style::default().fg(palette::TEXT)),
                    ])),
                    line_rects[dir_idx],
                );
            }
        }
        render_library_separator(f, area, row_y, row_h, sep_w);
    }

    fn render_library_title(
        &self,
        f: &mut Frame,
        rect: Rect,
        item: &MediaItem,
        content_w: usize,
        text_color: ratatui::style::Color,
        selected: bool,
        at_music_groups: bool,
    ) {
        let title_style = {
            let s = Style::default().fg(text_color);
            if selected {
                s.add_modifier(Modifier::BOLD)
            } else {
                s
            }
        };
        let title_display = wrap(&library_title_line(item), content_w.max(1))
            .into_iter()
            .next()
            .map(|c| c.into_owned())
            .unwrap_or_default();
        let title_line_widget = if let Some(count) = library_folder_count(item) {
            let count_style = Style::default()
                .fg(palette::IRIS)
                .add_modifier(Modifier::BOLD);
            let label_style = Style::default().fg(palette::YELLOW);
            let count_label = if at_music_groups { " albums" } else { " items" };
            Line::from(vec![
                Span::styled(title_display, title_style),
                Span::styled(format!(" · {count}"), count_style),
                Span::styled(count_label, label_style),
            ])
        } else {
            Line::from(Span::styled(title_display, title_style))
        };
        f.render_widget(Paragraph::new(title_line_widget), rect);
    }

    fn library_row_image_actual(
        &mut self,
        item: &MediaItem,
        show_img: bool,
        padded_area: Rect,
        ctx: &LibraryTableContext,
    ) -> Option<Size> {
        if !show_img {
            return None;
        }
        let cache_key = format!("{}:{}", item.id, crate::config::IMAGE_CACHE_SUFFIX_LIBRARY);
        let state = self.card_image_states.get_mut(&cache_key)?.as_mut()?;
        let (img_w, img_h) = if library_is_audio(item) || (ctx.at_album_folders && item.is_folder) {
            (LIB_AUDIO_IMG_W, ctx.audio_img_h)
        } else if library_is_episode_like(item, ctx.is_feed_lib) {
            (LIB_EPISODE_IMG_W, ctx.episode_img_h)
        } else {
            (LIB_SELECTED_IMG_W, ctx.selected_img_h)
        };
        // `size_for` is `None` while resize+encode is in-flight on the
        // worker thread (ThreadProtocol); treat that the same as no image
        // yet, same as the `?` above for an absent/unfetched entry.
        state.size_for(
            ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)),
            Size {
                width: img_w,
                height: img_h.min(padded_area.height),
            },
        )
    }

    fn library_row_rects(
        &self,
        padded_area: Rect,
        img_actual: Option<Size>,
        is_audio: bool,
        is_album_folder: bool,
    ) -> (Rect, Rect, Option<Rect>) {
        if is_audio || is_album_folder {
            if let Some(actual) = img_actual {
                let [a, b, _, c] = Layout::horizontal([
                    Constraint::Length(1),
                    Constraint::Length(actual.width),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .areas(padded_area);
                let img_h = actual.height.min(b.height);
                let v_off = b.height.saturating_sub(img_h) / 2;
                let img_rect = Rect {
                    y: b.y + v_off,
                    height: img_h,
                    ..b
                };
                (a, c, Some(img_rect))
            } else {
                let [a, c] = Layout::horizontal([Constraint::Length(1), Constraint::Min(0)])
                    .areas(padded_area);
                (a, c, None)
            }
        } else if let Some(actual) = img_actual {
            let [a, b, _, c] = Layout::horizontal([
                Constraint::Length(1),
                Constraint::Length(actual.width),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .areas(padded_area);
            let img_rect = Rect {
                height: actual.height.min(b.height),
                ..b
            };
            (a, c, Some(img_rect))
        } else {
            let [a, c] =
                Layout::horizontal([Constraint::Length(1), Constraint::Min(0)]).areas(padded_area);
            (a, c, None)
        }
    }
}

fn library_overview_lines(
    item: &MediaItem,
    selected: bool,
    content_w: usize,
    is_audio: bool,
    ctx: &LibraryTableContext,
) -> Vec<String> {
    if !is_audio && selected && ctx.images_enabled && !item.overview.is_empty() {
        wrap(&trunc_overview(&item.overview), content_w.max(1))
            .into_iter()
            .map(|s| s.into_owned())
            .collect()
    } else {
        Vec::new()
    }
}

fn library_tech_line(item: &MediaItem, selected: bool, is_audio: bool) -> String {
    if selected && !is_audio && !library_is_generic_folder(item) {
        match (item.video_info.is_empty(), item.audio_info.is_empty()) {
            (false, false) => format!("{}  {}", item.video_info, item.audio_info),
            (false, true) => item.video_info.clone(),
            (true, false) => item.audio_info.clone(),
            (true, true) => String::new(),
        }
    } else {
        String::new()
    }
}

fn library_base_lines(
    item: &MediaItem,
    is_album_folder: bool,
    artist_extra: usize,
    tech_lines: usize,
) -> usize {
    if is_album_folder {
        2
    } else if library_is_generic_folder(item) {
        1
    } else {
        2 + artist_extra + tech_lines
    }
}

fn render_library_separator(f: &mut Frame, area: Rect, row_y: u16, row_h: u16, sep_w: u16) {
    let sep_y = row_y + row_h - 1;
    if sep_y < area.y + area.height {
        let sep_rect = Rect {
            x: area.x,
            y: sep_y,
            width: sep_w,
            height: 1,
        };
        let sep_str: String = "─".repeat(sep_w as usize);
        f.render_widget(
            Paragraph::new(Span::styled(sep_str, Style::default().fg(palette::MUTED))),
            sep_rect,
        );
    }
}
