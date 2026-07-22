use super::ui_util::fmt_duration;
use super::{palette, App, LibEvent, PAGE_SIZE};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use std::io::Read as IoRead;
use std::time::Duration;
use textwrap::wrap;

pub(super) const NAV_IMAGE_FETCH_IDLE_DELAY: Duration = Duration::from_millis(150);

const MAX_IMAGE_FETCHES: usize = 6;
const MAX_ALBUM_ARTIST_FETCHES: usize = 6;

/// Cache key under which the bundled Power View card placeholder is stored in
/// `card_image_states`. Never touches `card_image_loading`, so it never triggers
/// the transient "Loading…" treatment — it is decoded synchronously from the
/// bundled bytes the first time it's needed and then just sits in the cache.
pub(super) const POWER_CARD_PLACEHOLDER_KEY: &str = "__power_card_placeholder__";

/// Fixed steady-state placeholder shown in the Power View queue card when no
/// queue-card artwork is available.
static POWER_CARD_PLACEHOLDER_BYTES: &[u8] =
    include_bytes!("../../assets/power-card-placeholder.webp");

/// A pending card-image fetch, queued when the in-flight limit is reached.
pub(super) struct ImageFetchReq {
    pub cache_key: String,
    pub item_id: String,
    pub series_id: String,
    pub types: Vec<String>,
    /// When true, the decoded image is center-cropped to a square before it is
    /// handed to the protocol. Used by the artist-header collage so its tiles
    /// are uniform squares regardless of the cover's native aspect ratio.
    pub square_crop: bool,
}

impl App {
    pub(super) fn fetch_album_year(&mut self, album_id: String) {
        if self.album_year_loading.contains(&album_id)
            || self.album_year_cache.contains_key(&album_id)
        {
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
            let year: u32 = ureq::get(&url)
                .call()
                .ok()
                .and_then(|r| r.into_json::<serde_json::Value>().ok())
                .and_then(|v| v["Items"].get(0).cloned())
                .and_then(|item| {
                    item["ProductionYear"]
                        .as_u64()
                        .or_else(|| item["Year"].as_u64())
                })
                .unwrap_or(0) as u32;
            let _ = tx.send(LibEvent::AlbumYearFetched { album_id, year });
        });
    }

    /// Proactively fetches the full track list for `album_id` so the Power
    /// View inline album detail pane (#145) can render it without the user
    /// drilling in first. Mirrors `fetch_album_year`'s simple one-shot
    /// fetch (no throttle queue) — only one album is ever highlighted at a
    /// time, so there is no fan-out to bound.
    pub(super) fn fetch_album_tracks(&mut self, album_id: String) {
        if self.album_tracks_loading.contains(&album_id)
            || self.album_tracks_cache.contains_key(&album_id)
        {
            return;
        }
        self.album_tracks_loading.insert(album_id.clone());
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let tracks = client
                .get_items_sorted(
                    &album_id,
                    None,
                    false,
                    0,
                    PAGE_SIZE,
                    "ParentIndexNumber,IndexNumber",
                    "Ascending",
                )
                .map(|(items, _total)| items)
                .unwrap_or_default();
            let _ = tx.send(LibEvent::AlbumTracksFetched { album_id, tracks });
        });
    }

    pub(super) fn fetch_album_artist(&mut self, album_id: String) {
        if self.album_artist_loading.contains(&album_id)
            || self.album_artist_cache.contains_key(&album_id)
        {
            return;
        }
        self.album_artist_loading.insert(album_id.clone());
        if self.album_artist_fetches_active >= MAX_ALBUM_ARTIST_FETCHES {
            // Queue instead of dropping: a slot will pick it up on completion.
            self.pending_album_artist_fetches.push_back(album_id);
            return;
        }
        self.spawn_album_artist_fetch(album_id);
    }

    /// Spawn queued album-artist fetches until the in-flight limit is reached.
    /// Called whenever an in-flight fetch completes and frees a slot (see the
    /// `LibEvent::AlbumArtistFetched` handler in `actions.rs`).
    pub(super) fn drain_album_artist_fetches(&mut self) {
        while self.album_artist_fetches_active < MAX_ALBUM_ARTIST_FETCHES {
            let Some(album_id) = self.pending_album_artist_fetches.pop_front() else {
                break;
            };
            self.spawn_album_artist_fetch(album_id);
        }
    }

    fn spawn_album_artist_fetch(&mut self, album_id: String) {
        self.album_artist_fetches_active += 1;
        let (server_url, token) = {
            let c = self.client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let url = format!(
                "{}/Items?ParentId={}&IncludeItemTypes=Audio&Limit=5&SortBy=ParentIndexNumber,IndexNumber&SortOrder=Ascending&Fields=AlbumArtist,Artists&api_key={}",
                server_url, album_id, token
            );
            let items: Vec<serde_json::Value> = ureq::get(&url)
                .call()
                .ok()
                .and_then(|r| r.into_json::<serde_json::Value>().ok())
                .and_then(|v| v["Items"].as_array().cloned())
                .unwrap_or_default();

            // Majority vote over up to 5 tracks' AlbumArtist (falling back to
            // Artists[0] per-track), so one outlier/mistagged track can't poison
            // the whole album's displayed artist.
            let mut counts: Vec<(String, usize)> = Vec::new();
            for item in &items {
                let candidate = item["AlbumArtist"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| {
                        item["Artists"]
                            .get(0)
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
                if candidate.is_empty() {
                    continue;
                }
                match counts.iter_mut().find(|(c, _)| c == &candidate) {
                    Some(entry) => entry.1 += 1,
                    None => counts.push((candidate, 1)),
                }
            }
            // `max_by_key` breaks ties by keeping the *last* max; we want the
            // *first*-seen artist to win ties, since it corresponds to the
            // earliest track in the sample (closest to "read the first track").
            let artist = counts
                .into_iter()
                .enumerate()
                .max_by_key(|(i, (_, n))| (*n, std::cmp::Reverse(*i)))
                .map(|(_, (c, _))| c)
                .unwrap_or_default();

            let _ = tx.send(LibEvent::AlbumArtistFetched { album_id, artist });
        });
    }

    pub(super) fn fetch_card_image(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        self.queue_card_image_fetch(cache_key, item_id, series_id, types, false);
    }

    /// Like [`fetch_card_image`], but the decoded image is center-cropped to a
    /// square. Use a cache key distinct from the standalone image (e.g. a
    /// `:sq` suffix) so the un-cropped variant is not clobbered.
    pub(super) fn fetch_card_image_square(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        self.queue_card_image_fetch(cache_key, item_id, series_id, types, true);
    }

    fn queue_card_image_fetch(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
        square_crop: bool,
    ) {
        if self.card_image_loading.contains(&cache_key)
            || self.card_image_states.contains_key(&cache_key)
        {
            return;
        }
        // Reserve the key immediately so duplicate (and queued) requests dedupe.
        self.card_image_loading.insert(cache_key.clone());
        let req = ImageFetchReq {
            cache_key,
            item_id,
            series_id,
            types: types.iter().map(|s| s.to_string()).collect(),
            square_crop,
        };
        if self.image_fetches_active >= MAX_IMAGE_FETCHES {
            // Queue instead of dropping: a slot will pick it up on completion.
            self.pending_image_fetches.push_back(req);
            return;
        }
        self.spawn_image_fetch(req);
    }

    pub(in crate::app) fn list_image_fetches_allowed(&self) -> bool {
        self.last_nav_at.elapsed() >= NAV_IMAGE_FETCH_IDLE_DELAY
    }

    pub(super) fn list_image_renders_allowed(&self) -> bool {
        self.list_image_fetches_allowed()
    }

    pub(super) fn fetch_list_card_image_when_idle(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        if !self.list_image_fetches_allowed() {
            return;
        }
        self.fetch_card_image(cache_key, item_id, series_id, types);
    }

    pub(super) fn ensure_placeholder_card_image(&mut self) {
        if self
            .card_image_states
            .contains_key(POWER_CARD_PLACEHOLDER_KEY)
        {
            return;
        }
        let Some(picker) = self.image_picker.clone() else {
            return;
        };
        let state = image::load_from_memory(POWER_CARD_PLACEHOLDER_BYTES)
            .ok()
            .map(|img| self.new_thread_protocol(&picker, img, POWER_CARD_PLACEHOLDER_KEY));
        self.card_image_states
            .insert(POWER_CARD_PLACEHOLDER_KEY.to_string(), state);
    }

    /// Builds a [`ratatui_image::thread::ThreadProtocol`] for `cache_key`,
    /// registering a dedicated request channel with the resize worker thread
    /// (see `spawn_resize_worker` in `mod.rs`) so responses can be routed
    /// back to the right `card_image_states` entry. The expensive
    /// resize+encode step (`StatefulProtocol::resize_encode`) then runs off
    /// the render thread on first draw, instead of blocking it (#164).
    pub(super) fn new_thread_protocol(
        &self,
        picker: &Picker,
        img: image::DynamicImage,
        cache_key: &str,
    ) -> ratatui_image::thread::ThreadProtocol {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<ratatui_image::thread::ResizeRequest>();
        let _ = self
            .resize_register_tx
            .send((cache_key.to_string(), req_rx));
        ratatui_image::thread::ThreadProtocol::new(req_tx, Some(picker.new_resize_protocol(img)))
    }

    /// Spawn queued image fetches until the in-flight limit is reached. Called
    /// whenever an in-flight fetch completes and frees a slot (see the card-image
    /// receiver in `mod.rs`).
    pub(super) fn drain_image_fetches(&mut self) {
        while self.image_fetches_active < MAX_IMAGE_FETCHES {
            let Some(req) = self.pending_image_fetches.pop_front() else {
                break;
            };
            self.spawn_image_fetch(req);
        }
    }

    fn spawn_image_fetch(&mut self, req: ImageFetchReq) {
        self.image_fetches_active += 1;
        let (server_url, token) = {
            let c = self.client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let tx = self.card_image_tx.clone();
        let ImageFetchReq {
            cache_key,
            item_id,
            series_id,
            types,
            square_crop,
        } = req;
        std::thread::spawn(move || {
            // catch_unwind so a panic during fetch/decode still reports a result,
            // freeing the in-flight slot and the loading reservation (H9). Exactly
            // one message is sent per spawn, so the receiver can balance the count.
            let cache_key_outer = cache_key.clone();
            let tx_outer = tx.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let bytes: Option<Vec<u8>> = if let Some(cached) =
                    crate::config::read_image_disk_cache(&cache_key)
                {
                    Some(cached)
                } else {
                    let fetch_url = |url: &str| -> Option<Vec<u8>> {
                        let agent = ureq::AgentBuilder::new()
                            .timeout(std::time::Duration::from_secs(10))
                            .build();
                        agent.get(url).call().ok().and_then(|r| {
                            let mut buf = Vec::new();
                            r.into_reader()
                                .take(10 * 1024 * 1024)
                                .read_to_end(&mut buf)
                                .ok()?;
                            Some(buf)
                        })
                    };
                    let fetched = types.iter().find_map(|t| {
                        if t == "AudioChild" {
                            let child_url = format!(
                                "{}/Items?ParentId={}&IncludeItemTypes=Audio&Limit=1&api_key={}",
                                server_url, item_id, token
                            );
                            let child_id: Option<String> = fetch_url(&child_url)
                                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                                .and_then(|v| {
                                    v["Items"]
                                        .get(0)
                                        .and_then(|i| i["Id"].as_str().map(|s| s.to_string()))
                                });
                            let child_id = child_id?;
                            let url = format!(
                                "{}/Items/{}/Images/Primary?maxHeight=400&quality=80&api_key={}",
                                server_url, child_id, token
                            );
                            return fetch_url(&url);
                        }
                        let src = match t.as_str() {
                            "Logo" | "Backdrop" if !series_id.is_empty() => &series_id,
                            _ => &item_id,
                        };
                        let url = match t.as_str() {
                            "Backdrop" => format!(
                                "{}/Items/{}/Images/Backdrop/0?maxHeight=400&quality=80&api_key={}",
                                server_url, src, token
                            ),
                            "Logo" => format!(
                                "{}/Items/{}/Images/Logo?maxHeight=400&quality=80&api_key={}",
                                server_url, src, token
                            ),
                            _ => format!(
                                "{}/Items/{}/Images/Primary?maxHeight=400&quality=80&api_key={}",
                                server_url, src, token
                            ),
                        };
                        fetch_url(&url)
                    });
                    // Cache the original server bytes as-is. Emby already sized them
                    // (maxHeight=400&quality=80); no client-side re-encode, so quality
                    // is unchanged and the cache stays small for fast decode.
                    if let Some(ref b) = fetched {
                        crate::config::write_image_disk_cache(&cache_key, b);
                    }
                    fetched
                };
                // Decode off the UI thread; the main loop only builds the protocol.
                let img = bytes.and_then(|b| image::load_from_memory(&b).ok()).map(|img| {
                    if square_crop {
                        // Center-crop to a square so collage tiles are uniform
                        // regardless of the cover's native aspect ratio.
                        let side = img.width().min(img.height());
                        let x = (img.width() - side) / 2;
                        let y = (img.height() - side) / 2;
                        img.crop_imm(x, y, side, side)
                    } else {
                        img
                    }
                });
                let _ = tx.send((cache_key, img));
            }));
            if result.is_err() {
                let _ = tx_outer.send((cache_key_outer, None));
            }
        });
    }

    pub(super) fn images_enabled(&self) -> bool {
        self.image_protocol_enabled
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
            let border_fg = if selected {
                palette::IRIS
            } else {
                palette::WHITE
            };
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_fg));
            if let Some(title) = section_title {
                block = block
                    .title(Span::styled(
                        format!(" {} ", title),
                        Style::default()
                            .fg(palette::IRIS)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .title_alignment(Alignment::Center);
            } else if now_playing {
                block = block
                    .title(Span::styled(
                        " Now Playing ",
                        Style::default()
                            .fg(palette::GREEN)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .title_alignment(Alignment::Center);
            }
            if let Some(label) = count_label {
                block = block.title_bottom(
                    Line::from(Span::styled(
                        format!(" {} ", label),
                        Style::default().fg(palette::SUBTLE),
                    ))
                    .centered(),
                );
            }
            let inner = block.inner(card_rect);
            f.render_widget(block, card_rect);
            inner
        };

        if inner.height < 2 || inner.width == 0 {
            return None;
        }

        let trunc = |s: &str| -> String { super::ui_util::trunc_str(s, inner.width as usize) };

        let put = |f: &mut Frame, y: u16, para: Paragraph| {
            if y < inner.bottom() {
                f.render_widget(
                    para,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: 1,
                    },
                );
            }
        };

        let fmt_m = |t: i64| -> String {
            let s = t / TICKS_PER_SECOND;
            if s >= 3600 {
                format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
            } else {
                format!("{}m", s / 60)
            }
        };
        let text_rows = if inner.height >= 8 {
            if times_inline {
                4u16
            } else {
                5u16
            }
        } else if inner.height >= 5 {
            3
        } else {
            1
        };
        let show_seekbar = text_rows >= 5 || (times_inline && text_rows >= 4);
        let img_top = inner.y;
        let img_bottom = inner.bottom().saturating_sub(text_rows);
        let img_h = img_bottom.saturating_sub(img_top);

        let mut actual_img_h: u16 = 0;
        if img_h >= 2 {
            if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
                type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                if is_center {
                    let avail = ratatui::layout::Size {
                        width: inner.width.saturating_sub(2),
                        height: img_h,
                    };
                    // `size_for` is `None` while resize+encode is in-flight on
                    // the worker thread; skip drawing for this frame and try
                    // again once the response arrives.
                    if let Some(actual) = state.size_for(
                        ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)),
                        avail,
                    ) {
                        let img_x = inner.x + 1 + (avail.width.saturating_sub(actual.width)) / 2;
                        let img_rect = Rect {
                            x: img_x,
                            y: img_top,
                            width: actual.width,
                            height: actual.height,
                        };
                        f.render_stateful_widget(
                            SImg::default().resize(ratatui_image::Resize::Scale(Some(
                                ratatui_image::FilterType::Lanczos3,
                            ))),
                            img_rect,
                            state,
                        );
                        actual_img_h = actual.height;
                    }
                } else {
                    let w = (inner.width as u32 * 36 / 100) as u16;
                    let avail = ratatui::layout::Size {
                        width: w,
                        height: img_h,
                    };
                    if let Some(actual) = state.size_for(
                        ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)),
                        avail,
                    ) {
                        let img_x = inner.x + (inner.width.saturating_sub(actual.width)) / 2;
                        let img_y = img_top + (img_h.saturating_sub(actual.height)) / 2;
                        let img_rect = Rect {
                            x: img_x,
                            y: img_y,
                            width: actual.width,
                            height: actual.height,
                        };
                        f.render_stateful_widget(
                            SImg::default().resize(ratatui_image::Resize::Fit(Some(
                                ratatui_image::FilterType::Lanczos3,
                            ))),
                            img_rect,
                            state,
                        );
                        actual_img_h = actual.height;
                    }
                }
            }
        }
        let mut text_y = if text_top_aligned {
            img_top + actual_img_h
        } else {
            img_bottom
        };

        {
            let title_fg = if selected {
                palette::WHITE
            } else {
                palette::TEXT
            };
            let title_mod = if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            };
            let dur_suffix = if runtime > 0 {
                format!(" ({})", fmt_m(runtime))
            } else {
                String::new()
            };
            let w = inner.width as usize;
            let name_chars: Vec<char> = name.chars().collect();
            let name_len = name_chars.len();
            let suffix_len = dur_suffix.chars().count();
            if name_len + suffix_len <= w {
                let mut spans = vec![Span::styled(
                    name.to_string(),
                    Style::default().fg(title_fg).add_modifier(title_mod),
                )];
                if !dur_suffix.is_empty() {
                    spans.push(Span::styled(
                        dur_suffix,
                        Style::default().fg(palette::SUBTLE),
                    ));
                }
                put(
                    f,
                    text_y,
                    Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
                );
                text_y += 1;
            } else {
                let wrapped = wrap(name, w);
                let line1: String = wrapped.first().map(|s| s.to_string()).unwrap_or_default();
                let skip = line1.chars().count();
                let line2: String = name
                    .chars()
                    .skip(skip)
                    .collect::<String>()
                    .trim_start()
                    .chars()
                    .take(w)
                    .collect();
                put(
                    f,
                    text_y,
                    Paragraph::new(Line::from(Span::styled(
                        line1,
                        Style::default().fg(title_fg).add_modifier(title_mod),
                    )))
                    .alignment(Alignment::Center),
                );
                text_y += 1;
                let mut spans = vec![Span::styled(
                    line2,
                    Style::default().fg(title_fg).add_modifier(title_mod),
                )];
                if !dur_suffix.is_empty() {
                    spans.push(Span::styled(
                        dur_suffix,
                        Style::default().fg(palette::SUBTLE),
                    ));
                }
                put(
                    f,
                    text_y,
                    Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
                );
                text_y += 1;
            }
        }

        if stack_subtitles && !series.is_empty() {
            put(
                f,
                text_y,
                Paragraph::new(Line::from(Span::styled(
                    trunc(series),
                    Style::default().fg(palette::SUBTLE),
                )))
                .alignment(Alignment::Center),
            );
            text_y += 1;
            if !ep_tag.is_empty() {
                put(
                    f,
                    text_y,
                    Paragraph::new(Line::from(Span::styled(
                        ep_tag.to_string(),
                        Style::default().fg(palette::SUBTLE),
                    )))
                    .alignment(Alignment::Center),
                );
                text_y += 1;
            }
        } else if text_rows >= 3 && (!series.is_empty() || !ep_tag.is_empty()) {
            let line = if !series.is_empty() && !ep_tag.is_empty() {
                Line::from(vec![
                    Span::styled(trunc(series), Style::default().fg(palette::SUBTLE)),
                    Span::styled(" • ", Style::default().fg(palette::IRIS)),
                    Span::styled(ep_tag.to_string(), Style::default().fg(palette::SUBTLE)),
                ])
            } else if !series.is_empty() {
                Line::from(Span::styled(
                    trunc(series),
                    Style::default().fg(palette::SUBTLE),
                ))
            } else {
                Line::from(Span::styled(
                    ep_tag.to_string(),
                    Style::default().fg(palette::SUBTLE),
                ))
            };
            put(f, text_y, Paragraph::new(line).alignment(Alignment::Center));
            text_y += 1;
        }

        if show_seekbar && pos_ticks > 0 && rt_ticks > 0 {
            let full_w = inner.width as usize;
            let bar_w = (full_w as u32 * 3 / 5) as usize;
            let pad = (full_w.saturating_sub(bar_w)) / 2;
            let fraction = (pos_ticks as f64 / rt_ticks as f64).clamp(0.0, 1.0);
            let filled = ((fraction * bar_w as f64).round() as usize).min(bar_w);
            let seekbar_y = text_y;
            put(
                f,
                seekbar_y,
                Paragraph::new(Line::from(vec![
                    Span::raw(" ".repeat(pad)),
                    Span::styled(
                        "━".repeat(filled),
                        Style::default().fg(if now_playing {
                            palette::IRIS
                        } else {
                            palette::GREEN
                        }),
                    ),
                    Span::styled(
                        "─".repeat(bar_w - filled),
                        Style::default().fg(if now_playing {
                            palette::IRIS_DIM
                        } else {
                            Color::Rgb(0, 80, 128)
                        }),
                    ),
                ])),
            );
            let time_style = Style::default().fg(palette::SUBTLE);
            let elapsed_str = fmt_duration(pos_ticks / TICKS_PER_SECOND);
            let total_str = fmt_duration(rt_ticks / TICKS_PER_SECOND);
            let elapsed_w = elapsed_str.chars().count() as u16;
            let total_w = total_str.chars().count() as u16;
            let bar_x = inner.x + pad as u16;
            let bar_end_x = bar_x + bar_w as u16;
            if times_inline {
                if seekbar_y < inner.bottom() {
                    let elapsed_x = bar_x.saturating_sub(elapsed_w + 1).max(inner.x);
                    f.render_widget(
                        Paragraph::new(Span::styled(elapsed_str, time_style)),
                        Rect {
                            x: elapsed_x,
                            y: seekbar_y,
                            width: elapsed_w.min(bar_x.saturating_sub(elapsed_x + 1)),
                            height: 1,
                        },
                    );
                    let total_x = bar_end_x + 1;
                    let total_avail = (inner.x + inner.width).saturating_sub(total_x);
                    if total_x < inner.x + inner.width {
                        f.render_widget(
                            Paragraph::new(Span::styled(total_str, time_style)),
                            Rect {
                                x: total_x,
                                y: seekbar_y,
                                width: total_w.min(total_avail),
                                height: 1,
                            },
                        );
                    }
                }
                return Some(seekbar_y);
            } else {
                text_y += 1;
                if now_playing && text_y < inner.bottom() {
                    f.render_widget(
                        Paragraph::new(Span::styled(elapsed_str, time_style)),
                        Rect {
                            x: bar_x,
                            y: text_y,
                            width: elapsed_w.min(bar_w as u16),
                            height: 1,
                        },
                    );
                    let total_x = bar_end_x.saturating_sub(total_w);
                    f.render_widget(
                        Paragraph::new(Span::styled(total_str, time_style)),
                        Rect {
                            x: total_x,
                            y: text_y,
                            width: total_w.min(bar_w as u16),
                            height: 1,
                        },
                    );
                } else {
                    put(
                        f,
                        text_y,
                        Paragraph::new(format!("{} / {}", fmt_m(pos_ticks), fmt_m(rt_ticks)))
                            .style(Style::default().fg(palette::SUBTLE))
                            .alignment(Alignment::Center),
                    );
                }
            }
        } else if show_seekbar && rt_ticks > 0 {
            let status_str = if played { "Played" } else { "Unplayed" };
            let length_str = fmt_m(rt_ticks);
            put(
                f,
                text_y,
                Paragraph::new(Line::from(vec![
                    Span::styled(status_str, Style::default().fg(palette::SUBTLE)),
                    Span::styled(
                        " • ",
                        Style::default()
                            .fg(palette::IRIS)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(length_str, Style::default().fg(palette::SUBTLE)),
                ]))
                .alignment(Alignment::Center),
            );
            text_y += 1;
        }
        Some(text_y)
    }
}

#[cfg(test)]
mod tests {
    use super::NAV_IMAGE_FETCH_IDLE_DELAY;
    use crate::app::tests::make_app_stub;
    use std::time::{Duration, Instant};

    #[test]
    fn recent_navigation_blocks_list_card_image_fetch() {
        let mut app = make_app_stub();
        app.last_nav_at = Instant::now();

        app.fetch_list_card_image_when_idle(
            "recent-nav:P".into(),
            "recent-nav".into(),
            String::new(),
            &["Primary"],
        );

        assert!(!app.card_image_loading.contains("recent-nav:P"));
        assert!(!app.card_image_states.contains_key("recent-nav:P"));
    }

    #[test]
    fn idle_navigation_allows_list_card_image_fetch() {
        let mut app = make_app_stub();
        app.last_nav_at = Instant::now() - NAV_IMAGE_FETCH_IDLE_DELAY - Duration::from_millis(1);

        app.fetch_list_card_image_when_idle(
            "idle-nav:P".into(),
            "idle-nav".into(),
            String::new(),
            &["Primary"],
        );

        assert!(
            app.card_image_loading.contains("idle-nav:P")
                || app.card_image_states.contains_key("idle-nav:P")
        );
    }

    #[test]
    fn recent_navigation_blocks_list_image_render() {
        let mut app = make_app_stub();
        app.last_nav_at = Instant::now();

        assert!(!app.list_image_renders_allowed());
    }

    #[test]
    fn idle_navigation_allows_list_image_render() {
        let mut app = make_app_stub();
        app.last_nav_at = Instant::now() - NAV_IMAGE_FETCH_IDLE_DELAY - Duration::from_millis(1);

        assert!(app.list_image_renders_allowed());
    }
}
