use crate::app::{App, PowerFocus};
use ratatui::layout::Rect;
use ratatui::Frame;

impl App {
    fn render_card_image(
        &mut self,
        f: &mut Frame,
        area: Rect,
        cache_key: &str,
        max_h: u16,
    ) -> (u16, bool) {
        // On short terminals (<= 30 rows) cap the card image at 12 rows so the queue
        // list keeps adequate space; taller terminals cap at 18 rows.
        let max_h = max_h.min(if self.terminal_height <= 30 { 12 } else { 18 });
        let image_loading = self.card_image_loading.contains(cache_key);
        if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::protocol::StatefulProtocol>;
            let avail = ratatui::layout::Size {
                width: area.width,
                height: max_h.saturating_sub(1),
            };
            let actual = state.size_for(
                ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)),
                avail,
            );
            let img_x = area.x + (area.width.saturating_sub(actual.width)) / 2;
            let img_rect = Rect {
                x: img_x,
                y: area.y,
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
            self.last_card_height = actual.height + 1;
            (actual.height + 1, false)
        } else {
            // No image loaded yet — if a fetch is in-flight and we have never
            // rendered a card before, reserve the full height cap so the queue
            // panel doesn't expand then collapse when the first image arrives.
            let placeholder = if self.last_card_height == 0 && image_loading {
                max_h
            } else {
                self.last_card_height
            };
            (placeholder, image_loading)
        }
    }

    /// Renders the card image and returns `(rows_used, image_loading)`.
    /// `rows_used` is 0 if the queue is empty or the image is not yet ready.
    /// `image_loading` is true when a fetch is in-flight (caller should defer
    /// rendering the rest of the view until the image arrives).
    pub(super) fn render_power_card(&mut self, f: &mut Frame, area: Rect) -> (u16, bool) {
        // If a movie detail is pinned, show that item's image instead of the queue cursor item.
        // Only show library-driven images when the library panel has focus; switch back to
        // the queue selection image when the queue panel is focused.
        let lib_focused = matches!(self.power_focus, PowerFocus::Left);
        let power_detail_pinned = lib_focused
            && self.power_left_tab > 0
            && self.libs[self.power_left_tab - 1]
                .power_detail_item
                .is_some();
        if power_detail_pinned {
            // (handled below)
        } else if lib_focused
            && self.power_left_tab > 0
            && self.is_album_level(self.power_left_tab - 1)
        {
            // When browsing a music album's tracks, show the album art in the card slot.
            let lib_idx = self.power_left_tab - 1;
            let (album_id, fallback_id) = {
                let lib = &self.libs[lib_idx];
                let lvl = match lib.nav_stack.last() {
                    Some(l) => l,
                    None => return (0, false),
                };
                let fid = lvl.items.first().map(|t| t.id.clone()).unwrap_or_default();
                (lvl.parent_id.clone(), fid)
            };
            let fetch_id = if !album_id.is_empty() {
                album_id.clone()
            } else {
                fallback_id
            };
            let cache_key = format!("{}:pwr_al", album_id);
            self.fetch_card_image(
                cache_key.clone(),
                fetch_id,
                String::new(),
                &["AudioChild", "Primary"],
            );
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        } else if lib_focused
            && self.power_left_tab > 0
            && self.is_series_view(self.power_left_tab - 1)
        {
            // Series view: show the selected episode's image when at episode level,
            // or the current season's poster when still loading.
            let lib_idx = self.power_left_tab - 1;
            let stack_len = self.libs[lib_idx].nav_stack.len();
            let at_episodes = self.libs[lib_idx]
                .nav_stack
                .last()
                .and_then(|l| l.items.first())
                .map(|i| i.item_type == "Episode")
                .unwrap_or(false);
            let (cache_key, item_id, series_id) = if at_episodes {
                let lib = &self.libs[lib_idx];
                let lvl = lib.nav_stack.last().unwrap();
                match lvl.items.get(lvl.cursor) {
                    Some(ep) => (
                        format!("{}:pwr_ep", ep.id),
                        ep.id.clone(),
                        ep.series_id.clone(),
                    ),
                    None => return (0, false),
                }
            } else {
                // Transitional loading state (switch_season in flight): episodes
                // haven't arrived yet. Return blank placeholder rows so neither
                // the season poster nor the queue image flashes during the gap.
                let is_switch_loading = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.loading && l.items.is_empty())
                    .unwrap_or(false);
                if is_switch_loading {
                    return (self.last_card_height, false);
                }
                // At-season level (before any drill-in): use the season's own image.
                let lib = &self.libs[lib_idx];
                let season_lvl = if stack_len >= 2 {
                    &lib.nav_stack[stack_len - 2]
                } else {
                    lib.nav_stack.last().unwrap()
                };
                match season_lvl.items.get(season_lvl.cursor) {
                    Some(s) => (format!("{}:pwr_ep", s.id), s.id.clone(), String::new()),
                    None => return (0, false),
                }
            };
            self.fetch_card_image(
                cache_key.clone(),
                item_id,
                series_id,
                &["Primary", "Backdrop"],
            );
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        } else if lib_focused
            && self.power_left_tab > 0
            && self.is_home_video_view(self.power_left_tab - 1)
        {
            // Home video / feed library: show the selected item's thumbnail.
            let lib_idx = self.power_left_tab - 1;
            let (item_id, series_id) = {
                let lib = &self.libs[lib_idx];
                let lvl = match lib.nav_stack.last() {
                    Some(l) => l,
                    None => return (0, false),
                };
                match lvl.items.get(lvl.cursor) {
                    Some(item) => (item.id.clone(), item.series_id.clone()),
                    None => return (0, false),
                }
            };
            let cache_key = format!("{}:pwr_hv", item_id);
            self.fetch_card_image(
                cache_key.clone(),
                item_id,
                series_id,
                &["Primary", "Backdrop"],
            );
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        }

        if power_detail_pinned {
            let (detail_id, series_id) = {
                let lib_idx = self.power_left_tab - 1;
                let d = self.libs[lib_idx].power_detail_item.as_ref().unwrap();
                (d.id.clone(), d.series_id.clone())
            };
            let img_types: &[&str] = &["Backdrop", "Primary", "Logo"];
            let cache_key = format!("{}:P", detail_id);
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), detail_id, series_id, img_types);
            }
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        }

        let cursor = self.player_tab.playlist_cursor;
        let n = self.player_tab.items.len();
        if n == 0 {
            return (0, false);
        }
        let item = &self.player_tab.items[cursor];
        let img_types: &[&str] = match item.item_type.as_str() {
            "MusicAlbum" => &["AudioChild"],
            "Audio" => &["Primary"],
            "Movie" => &["Backdrop", "Primary", "Logo"],
            _ => &["Primary", "Backdrop", "Logo"],
        };
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        // For audio tracks, key by album_id so all tracks on the same album share
        // one cached image. Fetch still uses the track ID (proven URL), but the
        // result is stored under the album key so the second track hits the cache.
        let cache_key = if item.item_type == "Audio" && !item.album_id.is_empty() {
            format!("{}:P", item.album_id)
        } else {
            format!("{}:P", item_id)
        };
        let is_music_item = matches!(img_types, &["Primary"] | &["AudioChild"]);
        if self.images_enabled() || is_music_item {
            self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
        }

        // Prefetch images for nearby items so they are ready before the cursor reaches them.
        // Collect data first (releasing the borrow on items) then call fetch (&mut self).
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        let start = cursor.saturating_sub(PREFETCH_BEHIND);
        let end = (cursor + PREFETCH_AHEAD + 1).min(n);
        let prefetch: Vec<(String, String, String, String)> = self.player_tab.items[start..end]
            .iter()
            .enumerate()
            .filter(|(i, _)| start + i != cursor)
            .map(|(_, p)| {
                let key = if p.item_type == "Audio" && !p.album_id.is_empty() {
                    format!("{}:P", p.album_id)
                } else {
                    format!("{}:P", p.id)
                };
                (key, p.id.clone(), p.series_id.clone(), p.item_type.clone())
            })
            .collect();
        for (pkey, pid, psid, ptype) in prefetch {
            let ptypes: &[&str] = match ptype.as_str() {
                "MusicAlbum" => &["AudioChild"],
                "Audio" => &["Primary"],
                "Movie" => &["Backdrop", "Primary", "Logo"],
                _ => &["Primary", "Backdrop", "Logo"],
            };
            let is_music = matches!(ptypes, &["Primary"] | &["AudioChild"]);
            if self.images_enabled() || is_music {
                self.fetch_card_image(pkey, pid, psid, ptypes);
            }
        }
        self.render_card_image(f, area, &cache_key, area.height)
    }
}
