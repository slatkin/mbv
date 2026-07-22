use super::POWER_RENDER_FILTER;
use crate::app::images::POWER_CARD_PLACEHOLDER_KEY;
use crate::app::{palette, App};
use mbv_core::api::MediaItem;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

fn power_card_image_types(item_type: &str) -> &'static [&'static str] {
    match item_type {
        "MusicAlbum" => &["AudioChild"],
        "Audio" => &["Primary"],
        "Movie" => &["Backdrop", "Primary", "Logo"],
        _ => &["Primary", "Backdrop", "Logo"],
    }
}

fn power_card_cache_key(item: &MediaItem) -> String {
    if item.item_type == "Audio" && !item.album_id.is_empty() {
        format!("{}:P", item.album_id)
    } else {
        format!("{}:P", item.id)
    }
}

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
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = ratatui::layout::Size {
                width: area.width,
                height: max_h.saturating_sub(1),
            };
            // `size_for` returns `None` while the resize+encode is still
            // in-flight on the worker thread (ThreadProtocol has taken its
            // inner protocol to send it off). Fall through to the
            // loading/placeholder path below for that frame; the next
            // frame after the response arrives will have a size again.
            if let Some(actual) = state.size_for(
                ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)),
                avail,
            ) {
                let img_x = area.x + (area.width.saturating_sub(actual.width)) / 2;
                let img_rect = Rect {
                    x: img_x,
                    y: area.y,
                    width: actual.width,
                    height: actual.height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER))),
                    img_rect,
                    state,
                );
                self.last_card_height = actual.height + 1;
                return (actual.height + 1, false);
            }
        }
        // No image loaded yet — if a fetch is in-flight and we have never
        // rendered a card before, reserve the full height cap so the queue
        // panel doesn't expand then collapse when the first image arrives.
        let placeholder = if self.last_card_height == 0 && image_loading {
            max_h
        } else {
            self.last_card_height
        };
        // The reserved area above was otherwise left visually blank while
        // loading -- paint a dim block over it instead, matching the
        // compact movie banner's own poster placeholder. Image aspect
        // ratios vary too widely here (backdrop, poster, album art,
        // thumbnail) to estimate a tighter width the way the banner does
        // for posters specifically, so this fills the full reserved area.
        if image_loading && placeholder > 0 {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::OVERLAY)),
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: placeholder,
                },
            );
        }
        (placeholder, image_loading)
    }

    fn render_power_card_placeholder(&mut self, f: &mut Frame, area: Rect) -> (u16, bool) {
        self.ensure_placeholder_card_image();
        let rendered =
            self.render_card_image(f, area, POWER_CARD_PLACEHOLDER_KEY, area.height.min(18));
        if rendered == (0, false) {
            (
                area.height
                    .min(if self.terminal_height <= 30 { 12 } else { 18 }),
                false,
            )
        } else {
            rendered
        }
    }

    /// Renders the card image and returns `(rows_used, image_loading)`.
    /// `rows_used` is 0 if the queue is empty or the image is not yet ready.
    /// `image_loading` is true when a fetch is in-flight (caller should defer
    /// rendering the rest of the view until the image arrives).
    pub(super) fn render_power_card(&mut self, f: &mut Frame, area: Rect) -> (u16, bool) {
        let playback = self.effective_playback_state();
        let active_source = if playback.active {
            let queue = self.playback_queue();
            (playback.active_idx < queue.items.len())
                .then(|| (playback.active_idx, queue.items.clone()))
        } else {
            None
        };
        let selected_source = || {
            let queue = self.displayed_queue();
            (queue.queue_cursor < queue.items.len())
                .then(|| (queue.queue_cursor, queue.items.clone()))
        };
        let Some((cursor, items)) = active_source.or_else(selected_source) else {
            return self.render_power_card_placeholder(f, area);
        };

        let item = &items[cursor];
        let img_types = power_card_image_types(&item.item_type);
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        let cache_key = power_card_cache_key(item);
        let is_music_item = matches!(img_types, &["Primary"] | &["AudioChild"]);
        if self.images_enabled() || is_music_item {
            self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
        }
        let use_placeholder = matches!(self.card_image_states.get(&cache_key), Some(None));

        // Prefetch images for nearby items so they are ready before the cursor reaches them.
        // Collect data first (releasing the borrow on items) then call fetch (&mut self).
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        let n = items.len();
        let start = cursor.saturating_sub(PREFETCH_BEHIND);
        let end = (cursor + PREFETCH_AHEAD + 1).min(n);
        let prefetch: Vec<(String, String, String, String)> = items[start..end]
            .iter()
            .enumerate()
            .filter(|(i, _)| start + i != cursor)
            .map(|(_, p)| {
                (
                    power_card_cache_key(p),
                    p.id.clone(),
                    p.series_id.clone(),
                    p.item_type.clone(),
                )
            })
            .collect();
        for (pkey, pid, psid, ptype) in prefetch {
            let ptypes = power_card_image_types(&ptype);
            let is_music = matches!(ptypes, &["Primary"] | &["AudioChild"]);
            if self.images_enabled() || is_music {
                self.fetch_list_card_image_when_idle(pkey, pid, psid, ptypes);
            }
        }
        if use_placeholder {
            return self.render_power_card_placeholder(f, area);
        }
        self.render_card_image(f, area, &cache_key, area.height)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::images::POWER_CARD_PLACEHOLDER_KEY;
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use crate::app::{palette, App, BrowseLevel, LibraryTab, PowerFocus, QueueScope, ViewMode};
    use crate::config::Config;
    use mbv_core::api::EmbyClient;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Style;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;

    fn make_queue_app(n: usize, cursor: usize) -> App {
        let mut app = make_app_stub();
        app.player_tab.set_items(make_items(n), cursor);
        app
    }

    fn make_drilled_library_app() -> App {
        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.view_mode = ViewMode::Power;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-focused".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![movie],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            library_total: None,
        });

        app
    }

    fn render_power_card(app: &mut App) -> (u16, bool) {
        let backend = TestBackend::new(30, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut result = (0u16, false);
        term.draw(|f| {
            result = app.render_power_card(f, Rect::new(0, 0, 30, 20));
        })
        .unwrap();
        result
    }

    fn render_power_card_to_string(app: &mut App) -> String {
        let backend = TestBackend::new(30, 20);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_card(f, Rect::new(0, 0, 30, 20));
        })
        .unwrap();
        format!("{:?}", term.backend().buffer())
    }

    fn set_playback(app: &mut App, active_idx: usize, paused: bool) {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = active_idx;
        status.paused = paused;
    }

    fn fetch_triggered(app: &App, key: &str) -> bool {
        app.card_image_loading.contains(key) || app.card_image_states.contains_key(key)
    }

    fn make_direct_remote_app(
        local_items: Vec<mbv_core::api::MediaItem>,
        remote_items: Vec<mbv_core::api::MediaItem>,
    ) -> App {
        let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
        app.player_tab.set_items(local_items, 0);
        app
    }

    #[test]
    fn active_playback_overrides_visible_cursor() {
        let mut app = make_queue_app(4, 0);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 2, false);

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "id2:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn paused_playback_keeps_active_priority() {
        let mut app = make_queue_app(3, 0);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 1, true);

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "id1:P"));
    }

    #[test]
    fn stopped_playback_uses_visible_queue_selection() {
        let mut app = make_queue_app(4, 2);
        app.image_protocol_enabled = true;

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "id2:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn active_remote_overrides_visible_local_queue() {
        let mut local_items = make_items(2);
        local_items[0].id = "local-0".into();
        local_items[1].id = "local-1".into();
        let mut remote_items = make_items(3);
        remote_items[0].id = "remote-0".into();
        remote_items[1].id = "remote-1".into();
        remote_items[2].id = "remote-2".into();
        let mut app = make_direct_remote_app(local_items, remote_items);
        app.set_queue_scope(QueueScope::Local);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 1, false);

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "remote-1:P"));
        assert!(!fetch_triggered(&app, "local-0:P"));
    }

    #[test]
    fn stopped_empty_visible_queue_ignores_hidden_nonempty_queue() {
        let local_items = Vec::new();
        let mut remote_items = make_items(1);
        remote_items[0].id = "remote-hidden".into();
        let mut app = make_direct_remote_app(local_items, remote_items);
        app.set_queue_scope(QueueScope::Local);
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.player.status.lock().unwrap().active = false;

        render_power_card(&mut app);

        assert!(app
            .card_image_states
            .contains_key(POWER_CARD_PLACEHOLDER_KEY));
        assert!(!fetch_triggered(&app, "remote-hidden:P"));
    }

    #[test]
    fn library_focus_and_depth_do_not_affect_card_source() {
        let mut app = make_drilled_library_app();
        app.player_tab.set_items(make_items(2), 1);
        app.image_protocol_enabled = true;

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "id1:P"));
        assert!(!fetch_triggered(&app, "movie-focused:P"));
    }

    #[test]
    fn stale_active_index_falls_back_to_visible_selection() {
        let mut app = make_queue_app(3, 1);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 99, false);

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "id1:P"));
    }

    #[test]
    fn completed_no_art_uses_power_card_placeholder() {
        let mut app = make_queue_app(6, 2);
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.card_image_states.insert("id2:P".into(), None);

        render_power_card(&mut app);

        assert!(app
            .card_image_states
            .contains_key(POWER_CARD_PLACEHOLDER_KEY));
        assert!(!app.card_image_loading.contains("id2:P"));
        assert!(fetch_triggered(&app, "id1:P"));
        assert!(fetch_triggered(&app, "id3:P"));
        assert!(fetch_triggered(&app, "id4:P"));
        assert!(fetch_triggered(&app, "id5:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn completed_no_art_prefetch_centers_on_active_source() {
        let mut app = make_queue_app(6, 0);
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.card_image_states.insert("id3:P".into(), None);
        set_playback(&mut app, 3, false);

        render_power_card(&mut app);

        assert!(app
            .card_image_states
            .contains_key(POWER_CARD_PLACEHOLDER_KEY));
        assert!(!app.card_image_loading.contains("id3:P"));
        assert!(fetch_triggered(&app, "id2:P"));
        assert!(fetch_triggered(&app, "id4:P"));
        assert!(fetch_triggered(&app, "id5:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn loading_image_paints_dim_reserved_block() {
        let mut app = make_queue_app(1, 0);
        app.image_protocol_enabled = true;

        let rendered = render_power_card_to_string(&mut app);

        assert!(app.card_image_loading.contains("id0:P"));
        assert!(rendered.contains(&format!("bg: {:?}", palette::OVERLAY)));
    }

    #[test]
    fn loading_image_overwrites_prior_card_area_with_dim_block() {
        let backend = TestBackend::new(30, 20);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            f.render_widget(
                Paragraph::new("STALE ART").style(Style::default().bg(palette::IRIS)),
                Rect::new(0, 0, 30, 6),
            );
        })
        .unwrap();

        let mut app = make_queue_app(1, 0);
        app.image_protocol_enabled = true;
        app.last_card_height = 6;
        term.draw(|f| {
            app.render_power_card(f, Rect::new(0, 0, 30, 20));
        })
        .unwrap();
        let rendered = format!("{:?}", term.backend().buffer());

        assert!(app.card_image_loading.contains("id0:P"));
        assert!(rendered.contains(&format!("bg: {:?}", palette::OVERLAY)));
        assert!(!rendered.contains("STALE"));
    }

    #[test]
    fn prefetch_centers_on_active_source_while_playing() {
        let mut app = make_queue_app(6, 0);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 3, false);

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "id3:P"));
        assert!(fetch_triggered(&app, "id2:P"));
        assert!(fetch_triggered(&app, "id4:P"));
        assert!(fetch_triggered(&app, "id5:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn prefetch_centers_on_selected_source_when_stopped() {
        let mut app = make_queue_app(6, 2);
        app.image_protocol_enabled = true;

        render_power_card(&mut app);

        assert!(fetch_triggered(&app, "id2:P"));
        assert!(fetch_triggered(&app, "id1:P"));
        assert!(fetch_triggered(&app, "id3:P"));
        assert!(fetch_triggered(&app, "id4:P"));
        assert!(fetch_triggered(&app, "id5:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }
}
