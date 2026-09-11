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

/// The queue visual slot's image projection (task 3.4, design D9): the queue
/// projection issues every fetch for the now-playing item and projects the
/// slot the painter consumes; painting reads it only — no fetch, no source
/// resolution. The shell's image-cache authority resolves the slot's
/// protocol handle for the painter (the same seam `paint_home_image` uses);
/// the painter itself takes plain data only.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct QueueCardProjection {
    /// The artwork slot's cache key; `None` paints the bundled placeholder
    /// (empty queue, a source without artwork, or a resolved-empty fetch).
    pub(in crate::app) cache_key: Option<String>,
    /// Terminal images are off: the slot reserves its last painted geometry
    /// and paints nothing.
    pub(in crate::app) images_enabled: bool,
    /// The visualizer is selected: painting stays App-coupled (it reads the
    /// shell's sample window and panel focus) until the visual slot moves
    /// into the Queue playback panel (task 3.5).
    pub(in crate::app) visualizer: bool,
}

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

/// The rectangle the queue card reserves for artwork: the last rendered
/// image/visualizer size, or the full reserved slot (capped like the artwork
/// height) before anything has rendered. Used by both the blank reservations
/// and the visualizer so `v` never moves the queue list.
fn card_reserved_rect(
    last_card: (u16, u16),
    terminal_height: u16,
    area: Rect,
    left_align: bool,
) -> Rect {
    let (last_height, last_width) = last_card;
    let max_h = area.height.min(if terminal_height <= 30 { 12 } else { 24 });
    let height = if last_height == 0 {
        // Keep the queue title separator and one queue row visible when
        // the card area is shorter than its normal image cap.
        if max_h == area.height {
            max_h.saturating_sub(2)
        } else {
            max_h
        }
    } else {
        last_height
    };
    let width = if last_width == 0 {
        if left_align {
            height.saturating_mul(2).min(area.width)
        } else {
            area.width
        }
    } else {
        last_width
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

type CardImageProtocol = ratatui_image::thread::ThreadProtocol;

/// The queue visual slot's painter (task 3.4, design D9): a free function
/// over projected state with no `App` access. `image` is the shell-resolved
/// protocol handle for the slot's key (`None` while not ready); `loading`
/// reserves the loading rectangle for an in-flight fetch. Returns
/// `(rows_used, cols_used, image_loading)`.
pub(in crate::app) fn render_card_painting(
    f: &mut Frame,
    area: Rect,
    left_align: bool,
    projection: &QueueCardProjection,
    // `true` while a fetch for the slot's key is in flight: the painter
    // reserves the loading rectangle instead of leaving the slot blank.
    loading: bool,
    image: Option<&mut CardImageProtocol>,
    last_card: (u16, u16),
    terminal_height: u16,
) -> (u16, u16, bool) {
    // The visualizer never reaches this painter (the `App` adapter paints it
    // from the shell's sample window); degenerate input reserves geometry.
    if projection.visualizer || !projection.images_enabled {
        let rect = card_reserved_rect(last_card, terminal_height, area, left_align);
        return (rect.height, rect.width, false);
    }
    // The bundled placeholder slot caps its height at 24 rows like the
    // compact banner's poster placeholder.
    let placeholder_slot = projection.cache_key.is_none();
    let cap = if terminal_height <= 30 { 12 } else { 24 };
    let max_h = if placeholder_slot {
        area.height.min(24)
    } else {
        area.height
    }
    .min(cap);
    let mut image = image;
    let actual_size = image.as_deref_mut().and_then(|state| {
        let avail = ratatui::layout::Size {
            width: area.width,
            height: max_h,
        };
        // `size_for` returns `None` while the resize+encode is still
        // in-flight on the worker thread (ThreadProtocol has taken its
        // inner protocol to send it off). Fall through to the
        // loading/placeholder path below for that frame; the next
        // frame after the response arrives will have a size again.
        state
            .size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            .map(|actual| (actual.height, actual.width, actual))
    });
    if let Some((height, width, _actual)) = actual_size {
        let img_x = if left_align {
            area.x
        } else {
            area.x + (area.width.saturating_sub(width)) / 2
        };
        let img_rect = Rect {
            x: img_x,
            y: area.y,
            width,
            height,
        };
        f.render_stateful_widget(
            ratatui_image::StatefulImage::default()
                .resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
            img_rect,
            image.expect("image handle present"),
        );
        return (height, width, false);
    }
    // No image loaded yet. If a fetch is in-flight and we have never rendered
    // a card before, reserve the full height cap so the queue panel doesn't
    // expand then collapse when the first image arrives; otherwise the last
    // painted geometry holds the slot steady.
    let (last_height, last_width) = last_card;
    let reservation = if last_height == 0 && loading {
        card_reserved_rect(last_card, terminal_height, area, left_align)
    } else {
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: last_height,
        }
    };
    let placeholder_w = if last_width == 0 && loading {
        reservation.width
    } else {
        last_width
    };
    // The reserved area above was otherwise left visually blank while
    // loading -- paint a dim block over it instead, matching the
    // compact movie banner's own poster placeholder. Image aspect
    // ratios vary too widely here (backdrop, poster, album art,
    // thumbnail) to estimate a tighter width the way the banner does
    // for posters specifically, so this fills the full reserved area.
    if loading && reservation.height > 0 {
        f.render_widget(
            Block::default().style(Style::default().bg(
                palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false).fill,
            )),
            reservation,
        );
    }
    if placeholder_slot && reservation.height == 0 && last_width == 0 && !loading {
        // An empty queue with no previous artwork geometry still reserves its
        // fallback rectangle so toggling `v` never moves the queue list.
        let rect = card_reserved_rect(last_card, terminal_height, area, left_align);
        return (rect.height, rect.width, false);
    }
    (reservation.height, placeholder_w, loading)
}

impl App {
    fn render_card_visualizer(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        let rect = card_reserved_rect(
            (self.last_card_height, self.last_card_width),
            self.terminal_height,
            area,
            left_align,
        );
        let bg = palette::surface_colors(
            palette::Surface::QueueCardVisualizer,
            matches!(self.effective_panel_focus(), PanelFocus::Queue),
        )
        .fill;
        self.render_visualizer(f, rect, bg);
        (rect.height, rect.width, false)
    }

    /// Renders the visual slot and returns `(rows_used, cols_used,
    /// image_loading)`. The projection owns every fetch (task 3.4, D9);
    /// painting reads the projected slot plus the shell-resolved image
    /// protocol handle — no fetch, no source resolution.
    pub(in crate::app) fn render_card(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        let mut projection = self.queue_card_projection.clone();
        if projection.visualizer {
            return self.render_card_visualizer(f, area, left_align);
        }
        if !projection.images_enabled {
            let rect = card_reserved_rect(
                (self.last_card_height, self.last_card_width),
                self.terminal_height,
                area,
                left_align,
            );
            return (rect.height, rect.width, false);
        }
        // The slot's key: the projected artwork key, or the bundled
        // placeholder when the fetch resolved empty or the slot is the
        // placeholder itself. Resolved from the image cache's authority —
        // a read, never a fetch.
        let artwork_key = projection.cache_key.clone().filter(|key| {
            !self
                .card_image_states
                .get(key)
                .is_some_and(|entry| entry.img.is_none())
        });
        let placeholder_slot = artwork_key.is_none();
        if placeholder_slot {
            self.ensure_placeholder_card_image();
            // The painter derives `placeholder_slot` from
            // `projection.cache_key`; hand it the resolved-empty fact (a
            // `Some` key whose fetch resolved empty means the placeholder)
            // so the painter's fallback reservation matches the adapter's
            // derivation (review of tasks 3.1-3.4).
            projection.cache_key = None;
        }
        let key = artwork_key.as_deref().unwrap_or(QUEUE_CARD_PLACEHOLDER_KEY);
        let loading = !placeholder_slot && self.card_image_loading.contains(key);
        let last_card = (self.last_card_height, self.last_card_width);
        let terminal_height = self.terminal_height;
        let image = self.cached_image_protocol_mut(key);
        let (height, width, loading) = render_card_painting(
            f,
            area,
            left_align,
            &projection,
            loading,
            image,
            last_card,
            terminal_height,
        );
        self.last_card_height = height;
        self.last_card_width = width;
        (height, width, loading)
    }

    /// The queue visual slot's image projection (task 3.4, design D9): the
    /// queue projection — not the painter — issues every fetch for the
    /// now-playing item and projects the slot to paint. Active-first, then
    /// the viewed queue's selection, exactly as the painter-side source
    /// resolution used to derive it.
    pub(in crate::app) fn refresh_queue_card_image(&mut self) {
        let mut projection = QueueCardProjection {
            cache_key: None,
            images_enabled: self.images_enabled(),
            visualizer: self.visualizer_enabled,
        };
        if projection.visualizer || !projection.images_enabled {
            self.queue_card_projection = projection;
            return;
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
            let cover_id = match raw_item {
                Some(QueueItem::Audiobookshelf(ep)) => Some((ep.library_item_id, false)),
                Some(QueueItem::AudiobookshelfBook(book)) => Some((book.library_item_id, true)),
                _ => None,
            };
            let Some((item_id, is_book)) = cover_id else {
                self.queue_card_projection = projection;
                return;
            };
            let Some(server_url) = self
                .config
                .lock()
                .unwrap()
                .audiobookshelf_setup
                .as_ref()
                .map(|setup| setup.server_url.clone())
            else {
                self.queue_card_projection = projection;
                return;
            };
            if is_book {
                self.fetch_audiobookshelf_book_cover(server_url.clone(), item_id.clone());
            } else {
                self.fetch_audiobookshelf_cover(server_url.clone(), item_id.clone());
            }
            let cache_key = if is_book {
                audiobookshelf_book_cover_cache_key(
                    &server_url,
                    &item_id,
                    self.current_protocol_suffix(),
                )
            } else {
                audiobookshelf_cover_cache_key(
                    &server_url,
                    &item_id,
                    self.current_protocol_suffix(),
                )
            };
            projection.cache_key = Some(cache_key);
            self.queue_card_projection = projection;
            return;
        };

        let img_types = card_image_types(&item.item_type);
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        let cache_key = card_cache_key(&item);
        self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);

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
        projection.cache_key = Some(cache_key);
        self.queue_card_projection = projection;
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
        // The projection owns every fetch (task 3.4); drive it the way the
        // queue projection does, then render from the projected state.
        app.refresh_queue_card_image();
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

        app.refresh_queue_card_image();
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

    /// A now-playing item whose fetch resolves empty must reserve the card
    /// rect (the bundled placeholder) even with no prior card geometry, not
    /// collapse to (0, 0) and hand its rows back to the queue panel (review
    /// of tasks 3.1-3.4).
    #[test]
    fn now_playing_resolved_empty_art_reserves_the_placeholder_slot() {
        let mut app = make_queue_app(3, 0);
        app.image_protocol_enabled = true;
        app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
        app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
        set_playback(&mut app, 1, false);
        // The now-playing item's fetch resolved empty; nothing has been
        // painted yet, so there is no prior card geometry.
        app.card_image_states
            .insert("id1:P".into(), CachedImage::empty());

        let (h, w, loading) = render_card(&mut app);

        assert!(
            h > 0 && w > 0,
            "a resolved-empty now-playing fetch must reserve the card rect, got ({h},{w})"
        );
        assert!(!loading);
        assert!(app
            .card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
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

/// Task 3.4 (design D9): the visual slot's painter is a free function over
/// projected state — this buffer test constructs the projection and the
/// geometry facts by hand and never touches `App`.
#[cfg(test)]
mod painter_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn card_painting_paints_from_projected_state_without_app_access() {
        let mut term = Terminal::new(TestBackend::new(20, 10)).unwrap();
        let area = Rect::new(0, 0, 20, 10);

        // A projected artwork slot with a fetch in flight and no protocol
        // state yet: the painter reserves the loading rectangle and paints
        // the dim loading block.
        let projection = QueueCardProjection {
            cache_key: Some("now-playing:P".into()),
            images_enabled: true,
            visualizer: false,
        };
        let mut result = (0u16, 0u16, false);
        term.draw(|f| {
            result = render_card_painting(f, area, false, &projection, true, None, (0, 0), 40);
        })
        .unwrap();
        let (height, width, loading) = result;
        assert!(loading, "an in-flight fetch reports image_loading");
        assert!(
            height > 0 && width > 0,
            "the loading reservation reserves rows"
        );
        let buf = term.backend().buffer();
        let loading_fill =
            crate::app::palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false)
                .fill;
        assert_eq!(
            buf[(0, 0)].style().bg,
            Some(loading_fill),
            "the projected loading slot paints the dim reservation block"
        );

        // Terminal images off: the slot reserves its last geometry and paints
        // nothing — no fetch, no placeholder state.
        let off = QueueCardProjection {
            cache_key: Some("now-playing:P".into()),
            images_enabled: false,
            visualizer: false,
        };
        let mut term = Terminal::new(TestBackend::new(20, 10)).unwrap();
        let mut result = (0u16, 0u16, false);
        term.draw(|f| {
            result = render_card_painting(f, area, false, &off, false, None, (4, 6), 40);
        })
        .unwrap();
        let (height, width, loading) = result;
        assert!(!loading);
        assert_eq!(
            (height, width),
            (4, 6),
            "the images-off slot reports the last painted geometry"
        );
        assert_eq!(
            term.backend().buffer()[(0, 0)].style().bg,
            Some(ratatui::style::Color::Reset),
            "images-off paints nothing (untouched cell)"
        );

        // The placeholder slot with no cached state: the fallback rectangle
        // (the bundled placeholder's reservation) with no loading block.
        let placeholder = QueueCardProjection {
            cache_key: None,
            images_enabled: true,
            visualizer: false,
        };
        let mut result = (0u16, 0u16, false);
        term.draw(|f| {
            result = render_card_painting(f, area, false, &placeholder, false, None, (0, 0), 40);
        })
        .unwrap();
        let (height, width, loading) = result;
        assert!(!loading);
        assert!(
            height > 0 && width > 0,
            "the empty-queue placeholder still reserves its fallback rectangle, got ({height},{width})"
        );
        assert_eq!(
            term.backend().buffer()[(0, 0)].style().bg,
            Some(ratatui::style::Color::Reset),
            "a not-loading placeholder paints no dim block"
        );
    }
}
