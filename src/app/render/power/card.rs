use super::POWER_RENDER_FILTER;
use crate::app::images::POWER_CARD_PLACEHOLDER_KEY;
use crate::app::{App, PowerFocus};
use ratatui::layout::Rect;
use ratatui::Frame;

/// Which source drives the Power View media card's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PowerCardSource {
    /// The queue-cursor item's artwork.
    QueueCursor,
    /// The focused library item's artwork (library browser drilled below the
    /// top level).
    LibraryFocused,
    /// A fixed steady-state placeholder (never the transient "Loading…" state).
    Placeholder,
}

/// Pure derivation of the Power View media card's content, re-derived every
/// frame from current Power View state — no persisted "last shown" stickiness:
///
/// - Queue (left column) focused, queue non-empty -> queue-cursor item's art.
/// - Library browser drilled below the top level -> focused library item's art.
/// - Top level / startup, queue non-empty -> queue-cursor item's art (default).
/// - Top level / startup, queue empty -> a fixed steady-state placeholder.
///
/// `library_drilled_below_top` takes priority over queue focus being merely
/// "possible but empty": an empty queue never has a cursor item to show, so
/// that combination falls through to the placeholder rather than indexing
/// into an empty queue.
pub(super) fn power_card_source(
    focus: PowerFocus,
    library_drilled_below_top: bool,
    queue_len: usize,
) -> PowerCardSource {
    if focus == PowerFocus::Queue && queue_len > 0 {
        return PowerCardSource::QueueCursor;
    }
    if library_drilled_below_top {
        return PowerCardSource::LibraryFocused;
    }
    if queue_len > 0 {
        PowerCardSource::QueueCursor
    } else {
        PowerCardSource::Placeholder
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
        (placeholder, image_loading)
    }

    /// Renders the card image and returns `(rows_used, image_loading)`.
    /// `rows_used` is 0 if the queue is empty or the image is not yet ready.
    /// `image_loading` is true when a fetch is in-flight (caller should defer
    /// rendering the rest of the view until the image arrives).
    pub(super) fn render_power_card(&mut self, f: &mut Frame, area: Rect) -> (u16, bool) {
        // If a leaf movie is selected, show that item's image instead of the queue
        // cursor item (the compact detail banner is showing for it). Only show
        // library-driven images when the library panel has focus; switch back to
        // the queue selection image when the queue panel is focused.
        let lib_focused = matches!(self.power_focus, PowerFocus::Left);
        let compact_banner_active = lib_focused
            && self.power_left_tab > 0
            && self
                .power_selected_movie_item(self.power_left_tab - 1)
                .is_some();
        if compact_banner_active {
            // (handled below)
        } else if lib_focused
            && self.power_left_tab > 0
            && self.is_viewing_album_folders(self.power_left_tab - 1)
        {
            // Inline album view: while the selected album's tracks are shown
            // under the album row, keep the card slot on that album's artwork.
            let lib_idx = self.power_left_tab - 1;
            let Some(album) = self.selected_album_item(lib_idx) else {
                return (0, false);
            };
            let cache_key = format!(
                "{}:{}",
                album.id,
                crate::config::IMAGE_CACHE_SUFFIX_POWER_ALBUM
            );
            self.fetch_card_image(
                cache_key.clone(),
                album.id,
                String::new(),
                &["AudioChild", "Primary"],
            );
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
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
            let cache_key = format!(
                "{}:{}",
                album_id,
                crate::config::IMAGE_CACHE_SUFFIX_POWER_ALBUM
            );
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
            let item = if self.is_feed_home_video_group_view(lib_idx) {
                self.selected_feed_home_video_item(lib_idx)
            } else {
                let lib = &self.libs[lib_idx];
                let lvl = match lib.nav_stack.last() {
                    Some(l) => l,
                    None => return (0, false),
                };
                lvl.items.get(lvl.cursor).cloned()
            };
            let Some(item) = item else {
                return (0, false);
            };
            let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
            let cache_key = format!("{}:pwr_hv", item_id);
            self.fetch_card_image(
                cache_key.clone(),
                item_id,
                series_id,
                &["Primary", "Backdrop"],
            );
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        }

        if compact_banner_active {
            if let Some(item) = self.power_selected_movie_item(self.power_left_tab - 1) {
                let img_types: &[&str] = &["Backdrop", "Primary", "Logo"];
                let cache_key = format!("{}:P", item.id);
                if self.images_enabled() {
                    self.fetch_card_image(
                        cache_key.clone(),
                        item.id.clone(),
                        item.series_id.clone(),
                        img_types,
                    );
                }
                let has_no_image = matches!(self.card_image_states.get(&cache_key), Some(None));
                if !has_no_image {
                    return self.render_card_image(f, area, &cache_key, area.height.min(18));
                }
                // Movie has no Backdrop/Primary/Logo image at all (fetch completed,
                // nothing found) — fall through to the default queue-cursor-art
                // code below instead of returning a blank card.
            }
        }

        let (cursor, items) = {
            let queue = self.playback_queue();
            (queue.queue_cursor, queue.items.clone())
        };
        let n = items.len();
        // By this point every "library browser drilled below top level" branch
        // above has already returned, so this is always the default derivation:
        // queue-cursor art when the queue has items, else the steady placeholder.
        if matches!(
            power_card_source(self.power_focus, false, n),
            PowerCardSource::Placeholder
        ) {
            self.ensure_placeholder_card_image();
            return self.render_card_image(
                f,
                area,
                POWER_CARD_PLACEHOLDER_KEY,
                area.height.min(18),
            );
        }
        let item = &items[cursor];
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
        let prefetch: Vec<(String, String, String, String)> = items[start..end]
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
                self.fetch_list_card_image_when_idle(pkey, pid, psid, ptypes);
            }
        }
        self.render_card_image(f, area, &cache_key, area.height)
    }
}

#[cfg(test)]
mod tests {
    use super::{power_card_source, PowerCardSource};
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{App, BrowseLevel, LibraryTab, PowerFocus, ViewMode};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;
    use std::time::Instant;

    /// Builds an App with a "movies" library whose only leaf item is a selected
    /// movie (compact banner active: lib focused, no pinned detail).
    fn make_compact_banner_movie_app() -> App {
        let mut app = make_app_stub();
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
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
        });

        app
    }

    fn make_inline_album_app() -> App {
        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.view_mode = ViewMode::Power;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;
        app.music_levels = vec!["group".into(), "album".into()];

        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();

        let mut group = make_item("Alpha", "MusicArtist");
        group.id = "group-0".into();
        group.is_folder = true;

        let mut album = make_item("First Album", "MusicAlbum");
        album.id = "album-1".into();
        album.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![
                BrowseLevel {
                    parent_id: "lib-music".into(),
                    title: "Music".into(),
                    items: vec![group],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                },
                BrowseLevel {
                    parent_id: "group-0".into(),
                    title: "Alpha".into(),
                    items: vec![album],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                },
            ],
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
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

    #[test]
    fn queue_focused_with_items_shows_queue_cursor() {
        assert_eq!(
            power_card_source(PowerFocus::Queue, false, 3),
            PowerCardSource::QueueCursor
        );
        // Drilled-in library state is irrelevant once the queue itself is focused.
        assert_eq!(
            power_card_source(PowerFocus::Queue, true, 3),
            PowerCardSource::QueueCursor
        );
    }

    #[test]
    fn queue_focused_but_empty_falls_back_to_placeholder() {
        // Should not happen in practice (UI shouldn't let you focus an empty
        // queue), but the derivation must stay safe rather than indexing into
        // an empty queue.
        assert_eq!(
            power_card_source(PowerFocus::Queue, false, 0),
            PowerCardSource::Placeholder
        );
    }

    #[test]
    fn library_drilled_below_top_shows_focused_library_item() {
        assert_eq!(
            power_card_source(PowerFocus::Left, true, 0),
            PowerCardSource::LibraryFocused
        );
        // Still true with a non-empty queue: drilled-in library wins over the
        // queue default.
        assert_eq!(
            power_card_source(PowerFocus::Left, true, 5),
            PowerCardSource::LibraryFocused
        );
    }

    #[test]
    fn top_level_with_nonempty_queue_defaults_to_queue_cursor() {
        assert_eq!(
            power_card_source(PowerFocus::Left, false, 1),
            PowerCardSource::QueueCursor
        );
    }

    #[test]
    fn top_level_with_empty_queue_shows_placeholder() {
        assert_eq!(
            power_card_source(PowerFocus::Left, false, 0),
            PowerCardSource::Placeholder
        );
    }

    #[test]
    fn compact_banner_active_fetches_movie_image_and_does_not_fall_through_to_queue() {
        let mut app = make_compact_banner_movie_app();
        app.image_protocol_enabled = true;
        // No queue item set up: if the render fell through to the default
        // queue-cursor-art path it would index into an empty queue and panic
        // (or at least never populate the movie's cache key), so a clean
        // render here proves the compact-banner branch handled it directly.
        render_power_card(&mut app);

        let cache_key = "movie-focused:P".to_string();
        assert!(
            app.card_image_loading.contains(&cache_key)
                || app.card_image_states.contains_key(&cache_key),
            "expected the selected movie's image fetch to be requested under its own cache key"
        );
        // The fetch is still in flight (never completed in this test), so the
        // cache key must not have been marked as "no image" — that only
        // happens once the fetch resolves to nothing.
        assert!(!matches!(app.card_image_states.get(&cache_key), Some(None)));
    }

    #[test]
    fn inline_selected_album_fetches_album_art() {
        let mut app = make_inline_album_app();
        app.image_protocol_enabled = true;
        assert!(app.is_viewing_album_folders(0));
        assert!(!app.is_album_level(0));

        render_power_card(&mut app);

        let cache_key = "album-1:pwr_al".to_string();
        assert!(
            app.card_image_loading.contains(&cache_key)
                || app.card_image_states.contains_key(&cache_key),
            "expected the selected inline album's artwork fetch to be requested"
        );
    }

    #[test]
    fn compact_banner_recent_navigation_still_fetches_focused_movie_image() {
        let mut app = make_compact_banner_movie_app();
        app.image_protocol_enabled = true;
        app.last_nav_at = Instant::now();

        render_power_card(&mut app);

        let cache_key = "movie-focused:P".to_string();
        assert!(
            app.card_image_loading.contains(&cache_key)
                || app.card_image_states.contains_key(&cache_key),
            "the focused movie image should start loading immediately even during rapid navigation"
        );
    }

    #[test]
    fn compact_banner_active_with_no_movie_image_falls_back_to_queue_art() {
        use crate::app::tests::make_items;

        let mut app = make_compact_banner_movie_app();
        app.image_protocol_enabled = true;
        // Simulate the fetch having already completed with no image found.
        app.card_image_states.insert("movie-focused:P".into(), None);

        // Give the queue an item so the fallback path has something to render.
        let queue_items = make_items(1);
        let queue_cache_key = format!("{}:P", queue_items[0].id);
        app.player_tab.set_items(queue_items, 0);

        render_power_card(&mut app);

        // The compact-banner branch fell through to the default queue-cursor-art
        // code, which fetches/renders the queue item under its own cache key
        // rather than returning early on the movie's (imageless) cache key.
        assert!(
            app.card_image_loading.contains(&queue_cache_key)
                || app.card_image_states.contains_key(&queue_cache_key),
            "expected fallback to fetch the queue-cursor item's image"
        );
    }
}
