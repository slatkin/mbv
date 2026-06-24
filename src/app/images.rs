use std::io::Read as IoRead;
use textwrap::wrap;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use crate::api::TICKS_PER_SECOND;
use super::{App, palette, LibEvent};
use super::ui_util::fmt_duration;

impl App {
    pub(super) fn fetch_album_year(&mut self, album_id: String) {
        if self.album_year_loading.contains(&album_id) || self.album_year_cache.contains_key(&album_id) {
            return;
        }
        self.album_year_loading.insert(album_id.clone());
        let (server_url, token) = {
            let c = self.client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let url = format!("{}/Items?ParentId={}&IncludeItemTypes=Audio&Limit=1&Fields=ProductionYear,Year&api_key={}",
                server_url, album_id, token);
            let year: u32 = ureq::get(&url).call().ok()
                .and_then(|r| r.into_json::<serde_json::Value>().ok())
                .and_then(|v| v["Items"].get(0).cloned())
                .and_then(|item| {
                    item["ProductionYear"].as_u64()
                        .or_else(|| item["Year"].as_u64())
                })
                .unwrap_or(0) as u32;
            let _ = tx.send(LibEvent::AlbumYearFetched { album_id, year });
        });
    }

    pub(super) fn fetch_card_image(&mut self, cache_key: String, item_id: String, series_id: String, types: &[&str]) {
        if self.card_image_loading.contains(&cache_key) || self.card_image_states.contains_key(&cache_key) {
            return;
        }
        self.card_image_loading.insert(cache_key.clone());
        let (server_url, token) = {
            let c = self.client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let types_owned: Vec<String> = types.iter().map(|s| s.to_string()).collect();
        let tx = self.card_image_tx.clone();
        std::thread::spawn(move || {
            if let Some(cached) = crate::config::read_image_disk_cache(&cache_key) {
                let _ = tx.send((cache_key, Some(cached)));
                return;
            }
            let fetch_url = |url: &str| -> Option<Vec<u8>> {
                ureq::get(url).call().ok().and_then(|r| {
                    let mut buf = Vec::new();
                    r.into_reader().read_to_end(&mut buf).ok()?;
                    Some(buf)
                })
            };
            let bytes = types_owned.iter().find_map(|t| {
                if t == "AudioChild" {
                    let child_url = format!("{}/Items?ParentId={}&IncludeItemTypes=Audio&Limit=1&api_key={}",
                        server_url, item_id, token);
                    let child_id: Option<String> = fetch_url(&child_url)
                        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                        .and_then(|v| v["Items"].get(0).and_then(|i| i["Id"].as_str().map(|s| s.to_string())));
                    let child_id = child_id?;
                    let url = format!("{}/Items/{}/Images/Primary?maxHeight=400&quality=80&api_key={}",
                        server_url, child_id, token);
                    return fetch_url(&url);
                }
                let src = match t.as_str() {
                    "Logo" | "Backdrop" if !series_id.is_empty() => &series_id,
                    _ => &item_id,
                };
                let url = match t.as_str() {
                    "Backdrop" => format!("{}/Items/{}/Images/Backdrop/0?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                    "Logo"     => format!("{}/Items/{}/Images/Logo?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                    _          => format!("{}/Items/{}/Images/Primary?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                };
                fetch_url(&url)
            });
            let bytes = bytes.map(|b| {
                match magick_resize(&b) {
                    Some(resized) => resized,
                    None => {
                        log::warn!(target: "img", "magick_resize failed for {cache_key}, using raw bytes");
                        b
                    }
                }
            });
            if let Some(ref b) = bytes {
                crate::config::write_image_disk_cache(&cache_key, b);
            }
            let _ = tx.send((cache_key, bytes));
        });
    }

    pub(super) fn images_enabled(&self) -> bool {
        self.image_protocol_enabled
    }

    pub(super) fn evict_card_images(&mut self) {
        let mut valid: std::collections::HashSet<String> = self.player_tab.items.iter()
            .flat_map(|item| [format!("{}:A", item.id), format!("{}:S", item.id)])
            .collect();
        // Preserve the current power-view card image so switching back doesn't re-fetch.
        if let Some(item) = self.player_tab.items.get(self.player_tab.playlist_cursor) {
            valid.insert(format!("{}:P", item.id));
        }
        for lib in &self.libs {
            if let Some(lvl) = lib.nav_stack.last() {
                if let Some(item) = lvl.items.get(lvl.cursor) {
                    valid.insert(format!("{}:lib", item.id));
                }
            }
        }
        self.card_image_states.retain(|k, _| valid.contains(k));
        self.card_image_loading.retain(|k| valid.contains(k));
        // Keep image_lru in sync so stale entries don't inflate the LRU and
        // cause premature eviction of valid images.
        self.image_lru.retain(|k| valid.contains(k));
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_card_slot(
        &mut self,
        f: &mut Frame,
        card_rect: Rect,
        is_center: bool,
        selected: bool,
        now_playing: bool,
        no_border: bool,
        text_top_aligned: bool,
        times_inline: bool,
        cache_key: &str,
        name: &str,
        series: &str,
        ep_tag: &str,
        runtime: i64,
        pos_ticks: i64,
        rt_ticks: i64,
        played: bool,
        count_label: Option<&str>,
        section_title: Option<&str>,
        stack_subtitles: bool,
    ) -> Option<u16> {
        let inner = if no_border {
            card_rect
        } else {
            let border_fg = if selected { palette::IRIS } else { palette::WHITE };
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_fg));
            if let Some(title) = section_title {
                block = block
                    .title(Span::styled(format!(" {} ", title), Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)))
                    .title_alignment(Alignment::Center);
            } else if now_playing {
                block = block
                    .title(Span::styled(" Now Playing ", Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)))
                    .title_alignment(Alignment::Center);
            }
            if let Some(label) = count_label {
                block = block.title_bottom(
                    Line::from(Span::styled(format!(" {} ", label), Style::default().fg(palette::SUBTLE)))
                        .centered()
                );
            }
            let inner = block.inner(card_rect);
            f.render_widget(block, card_rect);
            inner
        };

        if inner.height < 2 || inner.width == 0 { return None; }

        let trunc = |s: &str| -> String {
            let w = inner.width as usize;
            if s.chars().count() > w {
                format!("{}…", &s[..s.char_indices()
                    .nth(w.saturating_sub(1))
                    .map(|(b, _)| b)
                    .unwrap_or(s.len())])
            } else { s.to_string() }
        };

        let put = |f: &mut Frame, y: u16, para: Paragraph| {
            if y < inner.bottom() {
                f.render_widget(para, Rect { x: inner.x, y, width: inner.width, height: 1 });
            }
        };

        let fmt_m = |t: i64| -> String {
            let s = t / TICKS_PER_SECOND;
            if s >= 3600 { format!("{}h{:02}m", s/3600, (s%3600)/60) }
            else         { format!("{}m", s/60) }
        };
        let text_rows = if inner.height >= 8 { if times_inline { 4u16 } else { 5u16 } }
                        else if inner.height >= 5 { 3 }
                        else { 1 };
        let show_seekbar = text_rows >= 5 || (times_inline && text_rows >= 4);
        let img_top    = inner.y;
        let img_bottom = inner.bottom().saturating_sub(text_rows);
        let img_h      = img_bottom.saturating_sub(img_top);

        let mut actual_img_h: u16 = 0;
        if img_h >= 2 {
            if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                if is_center {
                    let avail = ratatui::layout::Size { width: inner.width.saturating_sub(2), height: img_h };
                    let actual = state.size_for(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)), avail);
                    let img_x = inner.x + 1 + (avail.width.saturating_sub(actual.width)) / 2;
                    let img_rect = Rect { x: img_x, y: img_top, width: actual.width, height: actual.height };
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3))),
                        img_rect, state,
                    );
                    actual_img_h = actual.height;
                } else {
                    let w     = (inner.width as u32 * 36 / 100) as u16;
                    let avail = ratatui::layout::Size { width: w, height: img_h };
                    let actual = state.size_for(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)), avail);
                    let img_x = inner.x + (inner.width.saturating_sub(actual.width)) / 2;
                    let img_y = img_top + (img_h.saturating_sub(actual.height)) / 2;
                    let img_rect = Rect { x: img_x, y: img_y, width: actual.width, height: actual.height };
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3))),
                        img_rect, state,
                    );
                    actual_img_h = actual.height;
                }
            }
        }
        let mut text_y = if text_top_aligned {
            img_top + actual_img_h
        } else {
            img_bottom
        };

        {
            let title_fg  = if selected { palette::WHITE } else { palette::TEXT };
            let title_mod = if selected { Modifier::BOLD } else { Modifier::empty() };
            let dur_suffix = if runtime > 0 { format!(" ({})", fmt_m(runtime)) } else { String::new() };
            let w           = inner.width as usize;
            let name_chars: Vec<char> = name.chars().collect();
            let name_len    = name_chars.len();
            let suffix_len  = dur_suffix.chars().count();
            if name_len + suffix_len <= w {
                let mut spans = vec![Span::styled(name.to_string(), Style::default().fg(title_fg).add_modifier(title_mod))];
                if !dur_suffix.is_empty() {
                    spans.push(Span::styled(dur_suffix, Style::default().fg(palette::SUBTLE)));
                }
                put(f, text_y, Paragraph::new(Line::from(spans)).alignment(Alignment::Center));
                text_y += 1;
            } else {
                let wrapped = wrap(name, w);
                let line1: String = wrapped.first().map(|s| s.to_string()).unwrap_or_default();
                let skip = line1.chars().count();
                let line2: String = name.chars().skip(skip).collect::<String>()
                    .trim_start().chars().take(w).collect();
                put(f, text_y, Paragraph::new(Line::from(
                    Span::styled(line1, Style::default().fg(title_fg).add_modifier(title_mod))
                )).alignment(Alignment::Center));
                text_y += 1;
                let mut spans = vec![Span::styled(line2, Style::default().fg(title_fg).add_modifier(title_mod))];
                if !dur_suffix.is_empty() {
                    spans.push(Span::styled(dur_suffix, Style::default().fg(palette::SUBTLE)));
                }
                put(f, text_y, Paragraph::new(Line::from(spans)).alignment(Alignment::Center));
                text_y += 1;
            }
        }

        if stack_subtitles && !series.is_empty() {
            put(f, text_y, Paragraph::new(Line::from(
                Span::styled(trunc(series), Style::default().fg(palette::SUBTLE))
            )).alignment(Alignment::Center));
            text_y += 1;
            if !ep_tag.is_empty() {
                put(f, text_y, Paragraph::new(Line::from(
                    Span::styled(ep_tag.to_string(), Style::default().fg(palette::SUBTLE))
                )).alignment(Alignment::Center));
                text_y += 1;
            }
        } else if text_rows >= 3 && (!series.is_empty() || !ep_tag.is_empty()) {
            let line = if !series.is_empty() && !ep_tag.is_empty() {
                Line::from(vec![
                    Span::styled(trunc(series), Style::default().fg(palette::SUBTLE)),
                    Span::styled(" • ",         Style::default().fg(palette::IRIS)),
                    Span::styled(ep_tag.to_string(), Style::default().fg(palette::SUBTLE)),
                ])
            } else if !series.is_empty() {
                Line::from(Span::styled(trunc(series), Style::default().fg(palette::SUBTLE)))
            } else {
                Line::from(Span::styled(ep_tag.to_string(), Style::default().fg(palette::SUBTLE)))
            };
            put(f, text_y, Paragraph::new(line).alignment(Alignment::Center));
            text_y += 1;
        }

        if show_seekbar && pos_ticks > 0 && rt_ticks > 0 {
            let full_w = inner.width as usize;
            let bar_w  = (full_w as u32 * 3 / 5) as usize;
            let pad    = (full_w.saturating_sub(bar_w)) / 2;
            let fraction = (pos_ticks as f64 / rt_ticks as f64).clamp(0.0, 1.0);
            let filled = ((fraction * bar_w as f64).round() as usize).min(bar_w);
            let seekbar_y = text_y;
            put(f, seekbar_y, Paragraph::new(Line::from(vec![
                Span::raw(" ".repeat(pad)),
                Span::styled("━".repeat(filled),         Style::default().fg(if now_playing { palette::IRIS } else { palette::FOAM })),
                Span::styled("─".repeat(bar_w - filled), Style::default().fg(if now_playing { palette::IRIS_DIM } else { Color::Rgb(0, 80, 128) })),
            ])));
            let time_style = Style::default().fg(palette::SUBTLE);
            let elapsed_str = fmt_duration(pos_ticks / TICKS_PER_SECOND);
            let total_str   = fmt_duration(rt_ticks / TICKS_PER_SECOND);
            let elapsed_w   = elapsed_str.chars().count() as u16;
            let total_w     = total_str.chars().count() as u16;
            let bar_x       = inner.x + pad as u16;
            let bar_end_x   = bar_x + bar_w as u16;
            if times_inline {
                if seekbar_y < inner.bottom() {
                    let elapsed_x = bar_x.saturating_sub(elapsed_w + 1).max(inner.x);
                    f.render_widget(
                        Paragraph::new(Span::styled(elapsed_str, time_style)),
                        Rect { x: elapsed_x, y: seekbar_y, width: elapsed_w.min(bar_x.saturating_sub(elapsed_x + 1)), height: 1 },
                    );
                    let total_x = bar_end_x + 1;
                    let total_avail = (inner.x + inner.width).saturating_sub(total_x);
                    if total_x < inner.x + inner.width {
                        f.render_widget(
                            Paragraph::new(Span::styled(total_str, time_style)),
                            Rect { x: total_x, y: seekbar_y, width: total_w.min(total_avail), height: 1 },
                        );
                    }
                }
                return Some(seekbar_y);
            } else {
                text_y += 1;
                if now_playing && text_y < inner.bottom() {
                    f.render_widget(
                        Paragraph::new(Span::styled(elapsed_str, time_style)),
                        Rect { x: bar_x, y: text_y, width: elapsed_w.min(bar_w as u16), height: 1 },
                    );
                    let total_x = bar_end_x.saturating_sub(total_w);
                    f.render_widget(
                        Paragraph::new(Span::styled(total_str, time_style)),
                        Rect { x: total_x, y: text_y, width: total_w.min(bar_w as u16), height: 1 },
                    );
                } else {
                    put(f, text_y, Paragraph::new(format!("{} / {}", fmt_m(pos_ticks), fmt_m(rt_ticks)))
                        .style(Style::default().fg(palette::SUBTLE))
                        .alignment(Alignment::Center));
                }
            }
        } else if show_seekbar && rt_ticks > 0 {
            let status_str = if played { "Played" } else { "Unplayed" };
            let length_str = fmt_m(rt_ticks);
            put(f, text_y, Paragraph::new(Line::from(vec![
                Span::styled(status_str, Style::default().fg(palette::SUBTLE)),
                Span::styled(" • ", Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                Span::styled(length_str, Style::default().fg(palette::SUBTLE)),
            ])).alignment(Alignment::Center));
            text_y += 1;
        }
        Some(text_y)
    }
}

pub(super) fn magick_resize(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let args: &[&[&str]] = &[
        &["magick", "convert"],
        &["convert"],
    ];
    for a in args {
        let (cmd, extra) = (a[0], &a[1..]);
        let Ok(mut child) = Command::new(cmd)
            .args(extra)
            .args(["-", "-filter", "Lanczos", "-resize", "400x400>", "-quality", "85", "png:-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn() else { continue };
        let Some(mut stdin) = child.stdin.take() else { continue };
        if stdin.write_all(bytes).is_err() { continue; }
        drop(stdin); // close pipe so magick knows EOF
        let Ok(out) = child.wait_with_output() else { continue };
        if out.status.success() && !out.stdout.is_empty() {
            return Some(out.stdout);
        }
    }
    None
}
