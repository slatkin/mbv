use super::widgets::RENDER_FILTER;
use crate::app::images::{
    audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key, QUEUE_CARD_PLACEHOLDER_KEY,
};
use crate::app::{palette, App, PanelFocus};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

fn card_image_types(item_type: &str) -> &'static [&'static str] {
    match item_type {
        "MusicAlbum" => super::widgets::MUSIC_ALBUM_IMAGE_TYPES,
        "Audio" => &["Primary"],
        "Movie" => &["Backdrop", "Primary", "Logo"],
        _ => &["Primary", "Backdrop", "Logo"],
    }
}

fn card_cache_key(item: &EmbyItem) -> String {
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
        left_align: bool,
    ) -> (u16, u16, bool) {
        // On short terminals (<= 30 rows) cap the card image at 12 rows so the queue
        // list keeps adequate space; taller terminals cap at 24 rows.
        let max_h = max_h.min(if self.terminal_height <= 30 { 12 } else { 24 });
        let image_loading = self.card_image_loading.contains(cache_key);
        let actual_size = if let Some(state) = self.cached_image_protocol_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = ratatui::layout::Size {
                width: area.width,
                height: max_h,
            };
            // `size_for` returns `None` while the resize+encode is still
            // in-flight on the worker thread (ThreadProtocol has taken its
            // inner protocol to send it off). Fall through to the
            // loading/placeholder path below for that frame; the next
            // frame after the response arrives will have a size again.
            if let Some(actual) =
                state.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            {
                let img_x = if left_align {
                    area.x
                } else {
                    area.x + (area.width.saturating_sub(actual.width)) / 2
                };
                let img_rect = Rect {
                    x: img_x,
                    y: area.y,
                    width: actual.width,
                    height: actual.height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                    img_rect,
                    state,
                );
                Some((actual.height, actual.width))
            } else {
                None
            }
        } else {
            None
        };
        if let Some((height, width)) = actual_size {
            self.last_card_height = height;
            self.last_card_width = width;
            return (height, width, false);
        }
        // No image loaded yet — if a fetch is in-flight and we have never
        // rendered a card before, reserve the full height cap so the queue
        // panel doesn't expand then collapse when the first image arrives.
        let reservation = if self.last_card_height == 0 && image_loading {
            self.card_reserved_rect(area, left_align)
        } else {
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: self.last_card_height,
            }
        };
        let placeholder_w = if self.last_card_width == 0 && image_loading {
            reservation.width
        } else {
            self.last_card_width
        };
        // The reserved area above was otherwise left visually blank while
        // loading -- paint a dim block over it instead, matching the
        // compact movie banner's own poster placeholder. Image aspect
        // ratios vary too widely here (backdrop, poster, album art,
        // thumbnail) to estimate a tighter width the way the banner does
        // for posters specifically, so this fills the full reserved area.
        if image_loading && reservation.height > 0 {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                reservation,
            );
        }
        (reservation.height, placeholder_w, image_loading)
    }

    /// Renders artwork for the active/selected queue item when it isn't an
    /// `EmbyItem` (Audiobookshelf episode or book). Feed entries carry no
    /// artwork (`QueueItem::artwork_url` is always `None` for them) and fall
    /// straight through to the placeholder, same as an empty queue.
    fn render_non_emby_card(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
        raw_item: Option<QueueItem>,
    ) -> (u16, u16, bool) {
        let cover_id = match raw_item {
            Some(QueueItem::Audiobookshelf(ep)) => Some((ep.library_item_id, false)),
            Some(QueueItem::AudiobookshelfBook(book)) => Some((book.library_item_id, true)),
            _ => None,
        };
        let Some((item_id, is_book)) = cover_id else {
            return self.render_card_placeholder(f, area, left_align);
        };
        let Some(server_url) = self
            .config
            .lock()
            .unwrap()
            .audiobookshelf_setup
            .as_ref()
            .map(|setup| setup.server_url.clone())
        else {
            return self.render_card_placeholder(f, area, left_align);
        };

        if is_book {
            self.fetch_audiobookshelf_book_cover(server_url.clone(), item_id.clone());
        } else {
            self.fetch_audiobookshelf_cover(server_url.clone(), item_id.clone());
        }
        let suffix = self.current_protocol_suffix();
        let cache_key = if is_book {
            audiobookshelf_book_cover_cache_key(&server_url, &item_id, suffix)
        } else {
            audiobookshelf_cover_cache_key(&server_url, &item_id, suffix)
        };

        let use_placeholder = self
            .card_image_states
            .get(&cache_key)
            .is_some_and(|e| e.img.is_none());
        if use_placeholder {
            return self.render_card_placeholder(f, area, left_align);
        }
        self.render_card_image(f, area, &cache_key, area.height, left_align)
    }

    fn render_card_placeholder(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        self.ensure_placeholder_card_image();
        let rendered = self.render_card_image(
            f,
            area,
            QUEUE_CARD_PLACEHOLDER_KEY,
            area.height.min(24),
            left_align,
        );
        if rendered == (0, 0, false) {
            let rect = self.card_reserved_rect(area, left_align);
            (rect.height, rect.width, false)
        } else {
            rendered
        }
    }

    /// The rectangle the queue card reserves for artwork: the last rendered
    /// image/visualizer size, or the full reserved slot (capped like the
    /// artwork height) before anything has rendered. Used by both the blank
    /// reservations and the visualizer so `v` never moves the queue list.
    fn card_reserved_rect(&self, area: Rect, left_align: bool) -> Rect {
        let max_h = area
            .height
            .min(if self.terminal_height <= 30 { 12 } else { 24 });
        let height = if self.last_card_height == 0 {
            // Keep the queue title separator and one queue row visible when
            // the card area is shorter than its normal image cap.
            if max_h == area.height {
                max_h.saturating_sub(2)
            } else {
                max_h
            }
        } else {
            self.last_card_height
        };
        let width = if self.last_card_width == 0 {
            if left_align {
                height.saturating_mul(2).min(area.width)
            } else {
                area.width
            }
        } else {
            self.last_card_width
        };
        let x = if left_align {
            area.x
        } else {
            area.x + (area.width.saturating_sub(width)) / 2
        };
        Rect {
            x,
            y: area.y,
            width: width.min(area.width),
            height: height.min(area.height),
        }
    }

    /// Renders the selected visualizer into the queue card's reserved artwork
    /// rectangle and returns the geometry the artwork/placeholder path would
    /// return, so toggling `v` never moves the queue list. Capture eligibility
    /// is a separate concern (`sync_visualizer`); here an empty sample window
    /// just paints the cleared vectorscope background.
    fn render_card_visualizer(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        let rect = self.card_reserved_rect(area, left_align);
        let bg = palette::resolve_surface_focus(matches!(
            self.effective_panel_focus(),
            PanelFocus::Queue
        ));
        self.render_visualizer(f, rect, bg);
        (rect.height, rect.width, false)
    }

    /// Renders the card image and returns `(rows_used, cols_used, image_loading)`.
    /// `rows_used`/`cols_used` are 0 if the queue is empty or the image is not
    /// yet ready. `image_loading` is true when a fetch is in-flight (caller
    /// should defer rendering the rest of the view until the image arrives).
    pub(in crate::app) fn render_card(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        if self.visualizer_enabled {
            return self.render_card_visualizer(f, area, left_align);
        }
        if !self.images_enabled() {
            let rect = self.card_reserved_rect(area, left_align);
            return (rect.height, rect.width, false);
        }
        let playback = self.effective_playback_state();
        let active_source = if playback.active {
            let queue = self.playback_queue();
            queue
                .emby_item_at(playback.active_idx)
                .cloned()
                .map(|item| (playback.active_idx, item))
        } else {
            None
        };
        let selected_source = || {
            let queue = self.displayed_queue();
            queue
                .clone_emby_item_at(queue.queue_cursor)
                .map(|item| (queue.queue_cursor, item))
        };
        let Some((cursor, item)) = active_source.or_else(selected_source) else {
            // The active/selected slot holds a non-Emby item (or the queue is
            // empty) -- resolve the raw `QueueItem` the same active-first,
            // then-selected way so Audiobookshelf artwork still renders here.
            let raw_item = if playback.active {
                self.playback_queue().item_at(playback.active_idx).cloned()
            } else {
                None
            }
            .or_else(|| {
                let queue = self.displayed_queue();
                queue.item_at(queue.queue_cursor).cloned()
            });
            return self.render_non_emby_card(f, area, left_align, raw_item);
        };

        let img_types = card_image_types(&item.item_type);
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        let cache_key = card_cache_key(&item);
        self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
        let use_placeholder = self
            .card_image_states
            .get(&cache_key)
            .is_some_and(|e| e.img.is_none());

        // Prefetch images for nearby items so they are ready before the cursor reaches them.
        // Collect data first (releasing the borrow on queue) then call fetch (&mut self).
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        let queue_ref = self.playback_queue();
        let n = queue_ref.total_queue_len();
        let start = cursor.saturating_sub(PREFETCH_BEHIND).min(n);
        let end = (cursor + PREFETCH_AHEAD + 1).min(n);
        let prefetch: Vec<(String, String, String, String)> = queue_ref.slots()[start..end]
            .iter()
            .enumerate()
            .filter(|(i, _)| start + i != cursor)
            .filter_map(|(_, slot)| slot.item.as_emby())
            .map(|p| {
                (
                    card_cache_key(p),
                    p.id.clone(),
                    p.series_id.clone(),
                    p.item_type.clone(),
                )
            })
            .collect();
        for (pkey, pid, psid, ptype) in prefetch {
            let ptypes = card_image_types(&ptype);
            self.fetch_list_card_image_when_idle(pkey, pid, psid, ptypes);
        }
        if use_placeholder {
            return self.render_card_placeholder(f, area, left_align);
        }
        self.render_card_image(f, area, &cache_key, area.height, left_align)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::images::{CachedImage, QUEUE_CARD_PLACEHOLDER_KEY};
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use crate::app::{App, BrowseLevel, LibraryTab, PanelFocus, QueueScope, TabSelection};
    use crate::config::Config;
    use mbv_core::api::EmbyClient;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    fn make_queue_app(n: usize, cursor: usize) -> App {
        let mut app = make_app_stub();
        app.player_tab.set_items(make_items(n), cursor);
        app
    }

    fn make_drilled_library_app() -> App {
        let mut app = make_app_stub();
        app.panel_focus = PanelFocus::Library;
        app.tab = TabSelection::EmbyLibrary(0);

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-focused".into();

        app.libs.push(LibraryTab {
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![movie],
                total_count: 1,
                resting: crate::app::types_browse::BrowseResting::new(0, 0),
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            }],
            ..LibraryTab::new(library)
        });

        app
    }

    fn render_card(app: &mut App) -> (u16, u16, bool) {
        let backend = TestBackend::new(30, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut result = (0u16, 0u16, false);
        term.draw(|f| {
            result = app.render_card(f, Rect::new(0, 0, 30, 20), false);
        })
        .unwrap();
        result
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
        local_items: Vec<mbv_core::api::EmbyItem>,
        remote_items: Vec<mbv_core::api::EmbyItem>,
    ) -> App {
        let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        let mut app = App::new_remote(
            EmbyClient::new(Config::default()),
            remote,
            player_rx,
            mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        );
        app.player_tab.set_items(local_items, 0);
        app
    }

    #[test]
    fn active_playback_overrides_visible_cursor() {
        let mut app = make_queue_app(4, 0);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 2, false);

        render_card(&mut app);

        assert!(fetch_triggered(&app, "id2:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn paused_playback_keeps_active_priority() {
        let mut app = make_queue_app(3, 0);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 1, true);

        render_card(&mut app);

        assert!(fetch_triggered(&app, "id1:P"));
    }

    #[test]
    fn stopped_playback_uses_visible_queue_selection() {
        let mut app = make_queue_app(4, 2);
        app.image_protocol_enabled = true;

        render_card(&mut app);

        assert!(fetch_triggered(&app, "id2:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn stopped_playback_with_audiobookshelf_selection_fetches_cover_not_placeholder() {
        let mut app = make_app_stub();
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.config.lock().unwrap().audiobookshelf_setup = Some(
            mbv_core::config::AudiobookshelfSetup::new("https://books.example"),
        );
        mbv_core::config::save_service_secret(
            mbv_core::config::ServiceKind::Audiobookshelf,
            "book-secret",
        )
        .unwrap();

        let episode = mbv_core::playback_queue::QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfQueueItem {
                library_item_id: "show-1".into(),
                episode_id: "ep-1".into(),
                title: "Episode".into(),
                show_title: None,
                author: None,
                description: None,
                duration_ticks: None,
                position_ticks: 0,
                played: false,
                pub_date_secs: None,
                is_finished: false,
                cover_path: None,
            },
        );
        app.player_tab.queue.append(episode);

        render_card(&mut app);

        assert!(
            !app.card_image_states
                .contains_key(QUEUE_CARD_PLACEHOLDER_KEY),
            "a selected Audiobookshelf queue item must not fall back to the bundled placeholder"
        );
        let cache_key = crate::app::images::audiobookshelf_cover_cache_key(
            "https://books.example",
            "show-1",
            app.current_protocol_suffix(),
        );
        assert!(
            app.card_image_loading.contains(&cache_key)
                || app.card_image_states.contains_key(&cache_key),
            "expected the Audiobookshelf cover fetch to be triggered"
        );
    }

    #[test]
    fn stopped_local_selection_with_empty_remote_playback_queue_does_not_panic() {
        let local_items = make_items(3);
        let mut app = make_direct_remote_app(local_items, Vec::new());
        app.set_queue_scope(QueueScope::Local);
        app.player_tab.set_items(make_items(3), 2);
        app.image_protocol_enabled = true;
        app.player.status.lock().unwrap().active = false;

        render_card(&mut app);

        assert!(fetch_triggered(&app, "id2:P"));
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

        render_card(&mut app);

        assert!(fetch_triggered(&app, "remote-1:P"));
        assert!(!fetch_triggered(&app, "local-0:P"));
    }

    #[test]
    fn library_focus_and_depth_do_not_affect_card_source() {
        let mut app = make_drilled_library_app();
        app.player_tab.set_items(make_items(2), 1);
        app.image_protocol_enabled = true;

        render_card(&mut app);

        assert!(fetch_triggered(&app, "id1:P"));
        assert!(!fetch_triggered(&app, "movie-focused:P"));
    }

    #[test]
    fn stale_active_index_falls_back_to_visible_selection() {
        let mut app = make_queue_app(3, 1);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 99, false);

        render_card(&mut app);

        assert!(fetch_triggered(&app, "id1:P"));
    }

    #[test]
    fn completed_no_art_uses_card_placeholder() {
        let mut app = make_queue_app(6, 2);
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.card_image_states
            .insert("id2:P".into(), CachedImage::empty());

        render_card(&mut app);

        assert!(app
            .card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
        assert!(!app.card_image_loading.contains("id2:P"));
    }

    #[test]
    fn completed_no_art_prefetch_centers_on_active_source() {
        let mut app = make_queue_app(6, 0);
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.card_image_states
            .insert("id3:P".into(), CachedImage::empty());
        set_playback(&mut app, 3, false);

        render_card(&mut app);

        assert!(app
            .card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
        assert!(!app.card_image_loading.contains("id3:P"));
    }

    #[test]
    fn prefetch_centers_on_active_source_while_playing() {
        let mut app = make_queue_app(6, 0);
        app.image_protocol_enabled = true;
        set_playback(&mut app, 3, false);

        render_card(&mut app);

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

        render_card(&mut app);

        assert!(fetch_triggered(&app, "id2:P"));
        assert!(fetch_triggered(&app, "id1:P"));
        assert!(fetch_triggered(&app, "id3:P"));
        assert!(fetch_triggered(&app, "id4:P"));
        assert!(fetch_triggered(&app, "id5:P"));
        assert!(!fetch_triggered(&app, "id0:P"));
    }

    #[test]
    fn selected_visualizer_reserves_geometry_without_fetching_artwork() {
        let mut app = make_queue_app(3, 2);
        app.image_protocol_enabled = true;
        app.visualizer_enabled = true;

        let backend = TestBackend::new(30, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut geometry = (0, 0, false);
        term.draw(|f| {
            geometry = app.render_card(f, Rect::new(0, 0, 30, 20), false);
        })
        .unwrap();
        let (h, w, loading) = geometry;

        assert!(
            h > 0 && w > 0,
            "the selected visualizer must reserve the card rectangle, got ({h},{w})"
        );
        assert!(!loading);
        assert!(
            !fetch_triggered(&app, "id2:P"),
            "selecting the visualizer must not fetch artwork"
        );
        assert!(!app
            .card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
        assert_eq!(
            term.backend().buffer()[(0, 0)].style().bg,
            Some(crate::app::palette::resolve_surface_focus(false)),
            "an empty selected visualizer must still paint its reserved card"
        );
    }

    #[test]
    fn selected_visualizer_ignores_confirmed_missing_artwork() {
        let mut app = make_queue_app(6, 2);
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.card_image_states
            .insert("id2:P".into(), CachedImage::empty());
        app.visualizer_enabled = true;

        let (h, w, loading) = render_card(&mut app);

        assert!(h > 0 && w > 0);
        assert!(!loading);
        assert!(
            !app.card_image_states
                .contains_key(QUEUE_CARD_PLACEHOLDER_KEY),
            "visualizer selection must not fall back to the bundled placeholder"
        );
    }

    #[test]
    fn pending_artwork_keeps_loading_reservation() {
        let mut app = make_queue_app(3, 1);
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());

        let (h, w, loading) = render_card(&mut app);

        assert!(
            loading,
            "pending artwork must report the in-flight loading reservation"
        );
        assert!(
            h > 0 && w > 0,
            "the loading reservation must reserve card rows, got ({h},{w})"
        );
    }

    #[test]
    fn images_off_artwork_and_visualizer_return_identical_geometry_without_fetch() {
        // Terminal images are disabled by default in the stub.
        let mut app = make_queue_app(3, 2);
        assert!(!app.images_enabled());

        let artwork = render_card(&mut app);
        assert!(!fetch_triggered(&app, "id2:P"));
        assert!(!app
            .card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));

        app.visualizer_enabled = true;
        let visualizer = render_card(&mut app);

        assert!(
            artwork.0 > 0 && artwork.1 > 0,
            "artwork selection must keep a blank reservation, got ({},{})",
            artwork.0,
            artwork.1
        );
        assert_eq!(
            artwork, visualizer,
            "images-off artwork and visualizer must keep the same fallback rectangle"
        );
        assert!(
            !fetch_triggered(&app, "id2:P"),
            "images-off must not fetch artwork"
        );
        assert!(app.card_image_states.is_empty());
    }
}
