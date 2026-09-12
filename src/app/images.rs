use super::{App, LibEvent, PAGE_SIZE};
use crate::app::palette;
use crate::app::render::components::widgets::RENDER_FILTER;
use crate::app::render::{PANE_PAD_X, PANE_PAD_Y};
use ratatui_image::picker::Picker;
use std::io::Read as IoRead;
use std::time::{Duration, Instant};

pub(super) const NAV_IMAGE_FETCH_IDLE_DELAY: Duration = Duration::from_millis(150);

pub(super) fn mem_key(cache_key: &str, suffix: &str) -> String {
    format!("{cache_key}@{suffix}")
}

/// Prefix shared by every Audiobookshelf-sourced cache key, used to filter
/// or clear Audiobookshelf entries from the image caches.
pub(super) const AUDIOBOOKSHELF_CACHE_KEY_PREFIX: &str = "audiobookshelf:";

/// Cache key for an Audiobookshelf cover under `server`, keyed by the
/// library item's `id` and the active protocol `suffix`.
pub(super) fn audiobookshelf_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:cover:{id}:{suffix}")
}

/// Cache key for an Audiobookshelf book cover. Distinct from the podcast
/// cover key (`:book:`, not `:cover:`) so a book and a podcast sharing an
/// id never share artwork state (book-browsing spec).
pub(super) fn audiobookshelf_book_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:bookcover:{id}:{suffix}")
}

/// The infix opening a Series artwork key: `{id}{SERIES_IMAGE_CACHE_KEY_INFIX}{types}`.
/// No other cache-key namespace uses it, which is what lets the image-completion
/// gate recognise the whole Series family from the key alone.
pub(in crate::app) const SERIES_IMAGE_CACHE_KEY_INFIX: &str = ":ser:";

/// Cache key for Series artwork under the `{id}:ser:{types}` scheme.
/// Shared by the `paint_home_image` Series arm and the shell-side
/// prefetch/loading lookups so the two can never format the key
/// differently and silently miss each other's cache entries. Formats only;
/// chain ownership stays with the callers.
pub(in crate::app) fn series_image_cache_key(item_id: &str, image_types: &[&str]) -> String {
    format!(
        "{item_id}{SERIES_IMAGE_CACHE_KEY_INFIX}{}",
        image_types.join(",")
    )
}

const MAX_IMAGE_FETCHES: usize = 6;
const MAX_ALBUM_ARTIST_FETCHES: usize = 6;

/// Cache key under which the bundled queue card placeholder is stored in
/// `card_image_states`. Never touches `card_image_loading`, so it never triggers
/// the transient "Loading…" treatment — it is decoded synchronously from the
/// bundled bytes the first time it's needed and then just sits in the cache.
pub(super) const QUEUE_CARD_PLACEHOLDER_KEY: &str = "__power_card_placeholder__";

/// Fixed steady-state placeholder shown in the queue card when no
/// queue-card artwork is available.
static QUEUE_CARD_PLACEHOLDER_BYTES: &[u8] =
    include_bytes!("../../assets/power-card-placeholder.webp");

/// One `card_image_states` cache entry: the decoded source image (retained so
/// it can be re-encoded with a different protocol picker without refetching —
/// e.g. the halfblock picker used while a backdrop is dimmed, #451) plus one
/// encoded `ThreadProtocol` per protocol suffix (e.g. `sixel`, `halfblock`).
/// The active suffix's protocol is built on fetch; the others are created
/// lazily on first render under that suffix.
pub(super) struct CachedImage {
    /// `None` marks a fetch that resolved without artwork.
    pub img: Option<image::DynamicImage>,
    pub protocols: std::collections::HashMap<&'static str, ratatui_image::thread::ThreadProtocol>,
    /// The hero artwork box (cells) the current protocols were built from
    /// (task 5.10, design D5's cover fit): the panel hero projection
    /// `resize_to_fill`s the source to the box's pixel size before encoding,
    /// keyed by the box, so a box change rebuilds the protocol at the new
    /// size. `None` for every non-hero cache entry (plain `Resize::Scale`).
    pub cover_box: Option<(u16, u16)>,
}

impl CachedImage {
    /// An entry for a fetch that resolved with no image.
    #[cfg(test)]
    pub(super) fn empty() -> Self {
        Self {
            img: None,
            protocols: std::collections::HashMap::new(),
            cover_box: None,
        }
    }
}

/// A pending card-image fetch, queued when the in-flight limit is reached.
pub(super) struct ImageFetchReq {
    pub cache_key: String,
    pub item_id: String,
    pub series_id: String,
    pub types: Vec<String>,
    pub source: ImageSource,
}

#[derive(Debug, Clone)]
pub(super) enum ImageSource {
    Emby,
    Audiobookshelf { server_url: String, api_key: String },
}

impl App {
    /// Proactively fetches the full track list for `album_id` so the view's
    /// inline album detail pane (#145) can render it without the user
    /// drilling in first. A simple one-shot fetch (no throttle queue) —
    /// only one album is ever highlighted at a time, so there is no fan-out
    /// to bound.
    pub(super) fn fetch_album_tracks(&mut self, album_id: String) {
        if self.album_tracks_loading.contains(&album_id)
            || self.album_tracks_cache.contains_key(&album_id)
        {
            return;
        }
        self.album_tracks_loading.insert(album_id.clone());
        let Some(client) = self.emby_snapshot() else {
            self.album_tracks_loading.remove(&album_id);
            return;
        };
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

    /// Proactively fetches TV series detail (seasons + episodes) so the
    /// Inline series detail pane can render without the user
    /// drilling in first.
    pub(super) fn fetch_series_detail(&mut self, series_id: String) {
        if series_id.is_empty() {
            return;
        }
        if self.series_detail_loading.contains(&series_id)
            || self.series_detail_cache.contains_key(&series_id)
        {
            return;
        }
        self.series_detail_loading.insert(series_id.clone());
        let Some(client) = self.emby_snapshot() else {
            self.series_detail_loading.remove(&series_id);
            return;
        };
        let tx = self.lib_tx.clone();
        let sid = series_id.clone();
        std::thread::spawn(move || {
            let (seasons, episodes) = client
                .get_items_sorted(&sid, None, false, 0, PAGE_SIZE, "SortName", "Ascending")
                .map(|(items, _total)| items)
                .map(|seasons| (seasons, std::collections::HashMap::new()))
                .unwrap_or_default();
            let _ = tx.send(LibEvent::SeriesDetailFetched {
                series_id: sid,
                seasons,
                episodes,
            });
        });
    }

    /// Fetches one season only after the complete ordered Series detail is in
    /// the cache. The detail event handler calls this for every uncached pill.
    pub(super) fn fetch_series_season_episodes(&mut self, series_id: String, season_id: String) {
        let key = (series_id.clone(), season_id.clone());
        let Some(detail) = self.series_detail_cache.get(&series_id) else {
            return;
        };
        if !detail.seasons.iter().any(|season| season.id == season_id)
            || detail.episodes.contains_key(&season_id)
            || self.series_season_loading.contains(&key)
        {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        self.series_detail_loading.insert(series_id.clone());
        self.series_season_loading.insert(key);
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let episodes = client
                .get_items_sorted(
                    &season_id,
                    None,
                    false,
                    0,
                    PAGE_SIZE,
                    "IndexNumber",
                    "Ascending",
                )
                .map(|(items, _)| items)
                .unwrap_or_default();
            let _ = tx.send(LibEvent::SeriesSeasonEpisodesFetched {
                series_id,
                season_id,
                episodes,
            });
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
            let Some(client) = self.emby_client() else {
                self.album_artist_loading.remove(&album_id);
                self.album_artist_fetches_active =
                    self.album_artist_fetches_active.saturating_sub(1);
                return;
            };
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let url = format!(
                "{}/Items?ParentId={}&IncludeItemTypes=Audio&Limit=5&SortBy=ParentIndexNumber,IndexNumber&SortOrder=Ascending&Fields=AlbumArtist,Artists&api_key={}",
                server_url, album_id, token
            );
            let items: Vec<serde_json::Value> = super::feed_parse::tls_agent(None)
                .get(&url)
                .call()
                .ok()
                .and_then(|mut r| r.body_mut().read_json::<serde_json::Value>().ok())
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
        self.queue_card_image_fetch(cache_key, item_id, series_id, types);
    }

    fn queue_card_image_fetch(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        if self.card_image_loading.contains(&cache_key)
            || self.card_image_states.contains_key(&cache_key)
        {
            return;
        }
        // Test-only instrumentation: counts every reservation that proceeds
        // past the dedup guard above, so a broken guard is visible even when
        // the fixture has no Emby client (`spawn_image_fetch` balances
        // `image_fetches_active` back to its prior value synchronously in
        // that case, hiding a redundant reservation from the other counters).
        // `project_hero_image` calls `fetch_card_image` unconditionally on
        // every sync pass, so counting entry into this function (rather than
        // past this guard) would also increment on every legitimate repaint.
        #[cfg(test)]
        {
            self.card_image_fetch_calls += 1;
        }
        // Reserve the key immediately so duplicate (and queued) requests dedupe.
        self.card_image_loading.insert(cache_key.clone());
        let req = ImageFetchReq {
            cache_key,
            item_id,
            series_id,
            types: types.iter().map(|s| s.to_string()).collect(),
            source: ImageSource::Emby,
        };
        if self.image_fetches_active >= MAX_IMAGE_FETCHES {
            // Queue instead of dropping: a slot will pick it up on completion.
            self.pending_image_fetches.push_back(req);
            return;
        }
        self.spawn_image_fetch(req);
    }

    pub(super) fn fetch_audiobookshelf_cover(&mut self, server_url: String, item_id: String) {
        let cache_key =
            audiobookshelf_cover_cache_key(&server_url, &item_id, self.current_protocol_suffix());
        self.fetch_audiobookshelf_image(cache_key, server_url, item_id);
    }

    /// Book-shaped sibling of `fetch_audiobookshelf_cover` using the isolated
    /// `:bookcover:` cache key.
    pub(super) fn fetch_audiobookshelf_book_cover(&mut self, server_url: String, item_id: String) {
        let cache_key = audiobookshelf_book_cover_cache_key(
            &server_url,
            &item_id,
            self.current_protocol_suffix(),
        );
        self.fetch_audiobookshelf_image(cache_key, server_url, item_id);
    }

    fn fetch_audiobookshelf_image(
        &mut self,
        cache_key: String,
        server_url: String,
        item_id: String,
    ) {
        if !self.image_protocol_enabled {
            return;
        }
        if self.card_image_loading.contains(&cache_key)
            || self.card_image_states.contains_key(&cache_key)
        {
            return;
        }
        let Some(api_key) =
            mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
        else {
            return;
        };
        let req = ImageFetchReq {
            cache_key: cache_key.clone(),
            item_id,
            series_id: String::new(),
            types: Vec::new(),
            source: ImageSource::Audiobookshelf {
                server_url,
                api_key,
            },
        };
        self.card_image_loading.insert(cache_key);
        if self.image_fetches_active >= MAX_IMAGE_FETCHES {
            self.pending_image_fetches.push_back(req);
        } else {
            self.spawn_image_fetch(req);
        }
    }

    pub(in crate::app) fn list_image_fetches_allowed(&self) -> bool {
        self.last_nav_at.elapsed() >= NAV_IMAGE_FETCH_IDLE_DELAY
    }

    pub(super) fn right_panel_image_renders_allowed(&self) -> bool {
        self.last_library_nav_at.elapsed() >= NAV_IMAGE_FETCH_IDLE_DELAY
    }

    pub(super) fn mark_library_navigation(&mut self, at: Instant) {
        self.last_library_nav_at = at;
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
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY)
        {
            return;
        }
        if self.picker_and_suffix().is_none() {
            return;
        }
        let Ok(img) = image::load_from_memory(QUEUE_CARD_PLACEHOLDER_BYTES) else {
            return;
        };
        let entry = self.build_cached_image(QUEUE_CARD_PLACEHOLDER_KEY, Some(img));
        self.card_image_states
            .insert(QUEUE_CARD_PLACEHOLDER_KEY.to_string(), entry);
    }

    fn picker_and_suffix(&self) -> Option<(&Picker, &'static str)> {
        let use_halfblock = self.dim_backdrop_active
            && self.image_protocol_enabled
            && !self.is_halfblock_configured();
        if use_halfblock {
            self.halfblock_picker.as_ref().map(|p| (p, "halfblock"))
        } else {
            self.image_picker
                .as_ref()
                .map(|p| (p, self.configured_protocol_name()))
        }
    }

    /// The suffix of the protocol currently active: the halfblock picker's
    /// while a dimmed backdrop is up, else the configured picker's.
    pub(super) fn current_protocol_suffix(&self) -> &'static str {
        self.picker_and_suffix()
            .map(|(_, s)| s)
            .unwrap_or("halfblock")
    }

    /// The picker that encodes the given protocol suffix.
    fn picker_for_suffix(&self, suffix: &'static str) -> Option<&Picker> {
        if suffix == "halfblock" {
            self.halfblock_picker
                .as_ref()
                .or(self.image_picker.as_ref())
        } else {
            self.image_picker.as_ref()
        }
    }

    /// Builds a fresh cache entry for a just-fetched image: keeps the decoded
    /// source so it can be re-encoded with a different protocol picker later
    /// (#451), and encodes the protocol for the currently active suffix.
    /// `img: None` records a resolved-but-empty fetch (the "no art" marker
    /// renderers branch on).
    pub(super) fn build_cached_image(
        &self,
        bare_key: &str,
        img: Option<image::DynamicImage>,
    ) -> CachedImage {
        let mut entry = CachedImage {
            img,
            protocols: std::collections::HashMap::new(),
            cover_box: None,
        };
        if let Some(img) = entry.img.clone() {
            let suffix = self.current_protocol_suffix();
            if let Some(picker) = self.picker_for_suffix(suffix) {
                let proto = self.build_protocol(bare_key, suffix, picker, img);
                entry.protocols.insert(suffix, proto);
            }
        }
        entry
    }

    /// Returns the protocol to render `bare_key` with under the currently
    /// active suffix, lazily re-encoding the retained source image when that
    /// suffix's protocol isn't cached yet (#451). The re-encode runs off the
    /// render thread (via the resize worker), so the first frame after a
    /// protocol switch still shows the placeholder while it completes.
    pub(super) fn cached_image_protocol_mut(
        &mut self,
        bare_key: &str,
    ) -> Option<&mut ratatui_image::thread::ThreadProtocol> {
        let suffix = self.current_protocol_suffix();
        let picker = self.picker_for_suffix(suffix)?;
        let reencode = self
            .card_image_states
            .get(bare_key)
            .is_some_and(|e| e.img.is_some() && !e.protocols.contains_key(suffix));
        if reencode {
            let (img, cover_box) = self
                .card_image_states
                .get(bare_key)
                .and_then(|e| e.img.clone().map(|img| (img, e.cover_box)))
                .expect("img present, just checked");
            // A hero entry's protocols carry the cover-fit crop (task 5.10,
            // design D5): rebuild from the source through the same cover step
            // so a suffix switch keeps the cropped aspect.
            let img = match cover_box {
                Some((w, h)) => {
                    let (px_w, px_h) = self.hero_box_pixels(w, h);
                    super::images::cover_fill_hero_box(&img, px_w, px_h)
                }
                None => img,
            };
            let proto = self.build_protocol(bare_key, suffix, picker, img);
            if let Some(entry) = self.card_image_states.get_mut(bare_key) {
                entry.protocols.insert(suffix, proto);
            }
        }
        self.card_image_states
            .get_mut(bare_key)?
            .protocols
            .get_mut(suffix)
    }

    fn build_protocol(
        &self,
        bare_key: &str,
        suffix: &'static str,
        picker: &Picker,
        img: image::DynamicImage,
    ) -> ratatui_image::thread::ThreadProtocol {
        let mem_key = mem_key(bare_key, suffix);
        let (req_tx, req_rx) = std::sync::mpsc::channel::<ratatui_image::thread::ResizeRequest>();
        let _ = self.resize_register_tx.send((mem_key, req_rx));
        ratatui_image::thread::ThreadProtocol::new(req_tx, Some(picker.new_resize_protocol(img)))
    }

    pub(super) fn is_halfblock_configured(&self) -> bool {
        self.image_protocol
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("halfblocks"))
            .unwrap_or(false)
            || self
                .image_picker
                .as_ref()
                .map(|p| p.protocol_type() == ratatui_image::picker::ProtocolType::Halfblocks)
                .unwrap_or(false)
    }

    pub(super) fn configured_protocol_name(&self) -> &'static str {
        use ratatui_image::picker::ProtocolType;
        match self.image_picker.as_ref().map(|p| p.protocol_type()) {
            Some(ProtocolType::Sixel) => "sixel",
            Some(ProtocolType::Kitty) => "kitty",
            Some(ProtocolType::Iterm2) => "iterm2",
            Some(ProtocolType::Halfblocks) | None => "halfblock",
        }
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
        let (server_url, token) = if matches!(req.source, ImageSource::Emby) {
            let Some(client) = self.emby_client() else {
                self.image_fetches_active = self.image_fetches_active.saturating_sub(1);
                let _ = self.card_image_tx.send((req.cache_key, None));
                return;
            };
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        } else {
            (String::new(), String::new())
        };
        let tx = self.card_image_tx.clone();
        let ImageFetchReq {
            cache_key,
            item_id,
            series_id,
            types,
            source,
        } = req;
        std::thread::spawn(move || {
            // catch_unwind so a panic during fetch/decode still reports a result,
            // freeing the in-flight slot and the loading reservation (H9). Exactly
            // one message is sent per spawn, so the receiver can balance the count.
            let cache_key_outer = cache_key.clone();
            let tx_outer = tx.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let bytes: Option<Vec<u8>> = if let ImageSource::Audiobookshelf {
                    server_url,
                    api_key,
                } = source
                {
                    if let Some(cached) = crate::config::read_image_disk_cache(&cache_key) {
                        Some(cached)
                    } else {
                        let client =
                            mbv_core::audiobookshelf::AudiobookshelfClient::new(&server_url).ok();
                        let result = client.and_then(|client| {
                            client
                                .cover_bounded(
                                    &api_key,
                                    &item_id,
                                    mbv_core::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
                                )
                                .ok()
                        });
                        if let Some(ref bytes) = result {
                            crate::config::write_image_disk_cache(&cache_key, bytes);
                        }
                        result
                    }
                } else if let Some(cached) = crate::config::read_image_disk_cache(&cache_key) {
                    // Mem-cache miss satisfied from the on-disk source bytes
                    // (no network). The protocol-specific re-encode then runs
                    // off-thread via the resize worker, so this is the
                    // local-only path that powers dim-then-undim cycles for
                    // a dimmed modal opening on a warm cache.
                    log::debug!(target: "images", "image disk cache hit for {cache_key}");
                    Some(cached)
                } else {
                    let fetch_url = |url: &str| -> Option<Vec<u8>> {
                        let agent =
                            super::feed_parse::tls_agent(Some(std::time::Duration::from_secs(10)));
                        agent.get(url).call().ok().and_then(|r| {
                            let mut buf = Vec::new();
                            r.into_body()
                                .into_reader()
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
                            "Logo" | "Backdrop" | "Thumb" if !series_id.is_empty() => &series_id,
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
                            "Thumb" => format!(
                                "{}/Items/{}/Images/Thumb?maxHeight=400&quality=80&api_key={}",
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
                let img = bytes.and_then(|b| image::load_from_memory(&b).ok());
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

    /// The active picker's terminal font size — the cell-to-pixel ratio the
    /// hero cover fit's box pixels derive from. Falls back to the halfblock
    /// picker and then the picker constructor's default.
    pub(in crate::app) fn image_font_size(&self) -> ratatui_image::FontSize {
        self.picker_and_suffix()
            .map(|(picker, _)| picker.font_size())
            .unwrap_or(ratatui_image::FontSize::new(10, 20))
    }

    /// One hero artwork box's pixel size from its cell size (task 5.10,
    /// design D5's cover-fit input).
    pub(in crate::app) fn hero_box_pixels(&self, box_w: u16, box_h: u16) -> (u32, u32) {
        let font = self.image_font_size();
        (
            u32::from(box_w) * u32::from(font.width.max(1)),
            u32::from(box_h) * u32::from(font.height.max(1)),
        )
    }

    /// The decoded source image's pixel size cached under `cache_key`, when
    /// one is (the Narrow inline hero's decoded-size arm, design D7).
    pub(in crate::app) fn decoded_image_size(&self, cache_key: &str) -> Option<(u32, u32)> {
        use image::GenericImageView;
        self.card_image_states
            .get(cache_key)
            .and_then(|entry| entry.img.as_ref())
            .map(|img| img.dimensions())
    }

    /// Ensure the hero cover-fit protocol for `cache_key` matches
    /// `box_cells` (task 5.10, design D5): the protocol is rebuilt from the
    /// decoded source through `cover_fill_hero_box` at the box's pixel size
    /// whenever the box changed (resize, split drag, Workspace shrink — the
    /// next sync pass's "re-encode request keyed by the new box size"), and
    /// the painters show the placeholder for at most that one frame.
    /// Returns whether a ready protocol is available.
    pub(in crate::app) fn ensure_hero_cover_protocol(
        &mut self,
        cache_key: &str,
        box_cells: (u16, u16),
    ) -> bool {
        let Some(entry) = self.card_image_states.get(cache_key) else {
            return false;
        };
        let Some(source) = entry.img.clone() else {
            return false;
        };
        if entry.cover_box == Some(box_cells) && !entry.protocols.is_empty() {
            return true;
        }
        let Some((suffix, picker)) = self
            .picker_and_suffix()
            .map(|(picker, suffix)| (suffix, picker.clone()))
        else {
            return false;
        };
        let (px_w, px_h) = self.hero_box_pixels(box_cells.0, box_cells.1);
        let cropped = cover_fill_hero_box(&source, px_w, px_h);
        let bare_key = cache_key.to_string();
        let proto = self.build_protocol(&bare_key, suffix, &picker, cropped);
        if let Some(entry) = self.card_image_states.get_mut(cache_key) {
            entry.protocols.clear();
            entry.protocols.insert(suffix, proto);
            entry.cover_box = Some(box_cells);
        }
        true
    }

    /// Paints the panel's retained hero image paint (task 5.10, design D9):
    /// the cached protocol rendered into the projected box (cover-fit keyed
    /// by the box at the projection), or — while that one frame's encode is
    /// still completing — the shared loading placeholder in the same box, so
    /// the placeholder never shows for more than the one frame the spec
    /// allows.
    pub(in crate::app) fn paint_panel_hero_image(
        &mut self,
        f: &mut ratatui::Frame,
        paint: &crate::app::components::library_panel::PanelHeroImagePaint,
    ) {
        if paint.area.width == 0 || paint.area.height == 0 {
            return;
        }
        if let Some(state) = self.cached_image_protocol_mut(&paint.cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = ratatui::layout::Size {
                width: paint.area.width,
                height: paint.area.height,
            };
            if let Some(actual) =
                state.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            {
                let img_rect = ratatui::layout::Rect {
                    x: paint.area.x + paint.area.width.saturating_sub(actual.width) / 2,
                    y: paint.area.y,
                    width: actual.width,
                    height: actual.height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                    img_rect,
                    state,
                );
                return;
            }
        }
        f.render_widget(
            ratatui::widgets::Block::default().style(ratatui::style::Style::default().bg(
                palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false).fill,
            )),
            paint.area,
        );
    }
}

/// Cover fit for a hero artwork box (design D5, task 5.4): the decoded
/// source image is scaled to cover the box's pixel size and centre-cropped,
/// so the box shows no margin; painting then uses the existing
/// `Resize::Scale`. The box's size comes from the panel's paint-free
/// `hero_artwork_box` (task 5.5); the shell projection that calls this is
/// keyed by the box (task 5.10, `App::ensure_hero_cover_protocol`).
pub(in crate::app) fn cover_fill_hero_box(
    source: &image::DynamicImage,
    box_w: u32,
    box_h: u32,
) -> image::DynamicImage {
    let (w, h) = (box_w.max(1), box_h.max(1));
    source.resize_to_fill(w, h, image::imageops::FilterType::Lanczos3)
}

#[cfg(test)]
mod tests {
    use super::{cover_fill_hero_box, series_image_cache_key, NAV_IMAGE_FETCH_IDLE_DELAY};
    use crate::app::tests::make_app_stub;
    use std::time::{Duration, Instant};

    /// A 4:3 source filled into a 16:9 box is cropped top and bottom (design
    /// D5: the artwork fills its box; the excess is cropped, centred). The
    /// source has white bands in its top and bottom eighths so a squashed or
    /// letterboxed fit would show white at the box edges; only the cover
    /// crop removes them.
    #[test]
    fn cover_fill_crops_a_4_3_source_into_a_16_9_box() {
        let (w, h) = (800u32, 600u32);
        let mut img = image::DynamicImage::new_rgb8(w, h);
        for (_x, y, pixel) in img.as_mut_rgb8().unwrap().enumerate_pixels_mut() {
            let white = y < h / 8 || y >= h * 7 / 8;
            *pixel = if white {
                image::Rgb([255, 255, 255])
            } else {
                image::Rgb([0, 0, 0])
            };
        }
        let filled = cover_fill_hero_box(&img, 160, 90);
        // The box is filled exactly: no letterbox margin remains.
        use image::GenericImageView;
        assert_eq!(filled.dimensions(), (160, 90));
        let brightness = |pixel: &image::Rgb<u8>| {
            let [r, g, b] = pixel.0;
            (u16::from(r) + u16::from(g) + u16::from(b)) / 3
        };
        let rgb = filled.as_rgb8().unwrap();
        // Cropped at the top: the white top band is gone (the visible top
        // row maps inside the source's black middle; Lanczos ringing may
        // bleed a little, so compare against the white band's brightness).
        assert!(brightness(rgb.get_pixel(80, 0)) < 64, "top band cropped");
        // Cropped at the bottom likewise.
        assert!(
            brightness(rgb.get_pixel(80, 89)) < 64,
            "bottom band cropped"
        );
    }

    #[test]
    fn series_image_cache_key_pins_both_live_chains() {
        assert_eq!(
            series_image_cache_key("abc", &["Primary"]),
            "abc:ser:Primary"
        );
        assert_eq!(
            series_image_cache_key("abc", &["Thumb", "Primary", "Backdrop", "Logo"]),
            "abc:ser:Thumb,Primary,Backdrop,Logo"
        );
    }

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
}

impl App {
    /// One panel hero's projected image state (task 5.10, design D9): the
    /// projection — never the painter — issues every fetch, then projects the
    /// state painting reads. `panel_area` is the Library panel's `RootFrame`
    /// content area; the Wide header's cover-fit box is keyed by the box
    /// `hero_artwork_box` derives from it, so a resize, split drag, or
    /// Workspace shrink re-encodes at the new box size on the next sync pass
    /// and the placeholder shows for at most that one frame. The Narrow
    /// inline hero is not cropped: it paints `Resize::Scale` into its own
    /// box, sized from the projected decoded aspect (design D7).
    pub(in crate::app) fn project_hero_image(
        &mut self,
        facts: &crate::app::components::library_panel::HeroFacts,
        workspace_present: bool,
        panel_area: ratatui::layout::Rect,
        list_pane_width: Option<u16>,
    ) -> crate::app::components::library_panel::HeroImageState {
        use crate::app::components::library_panel::content::ArtworkSource;
        use crate::app::components::library_panel::content::HeroImageState as State;
        let artwork = &facts.artwork;
        let Some(source) = &artwork.source else {
            return State::None;
        };
        if !self.images_enabled() {
            return State::None;
        }
        // The one fetch per key (fetch dedupes on its own reservation set).
        let cache_key: Option<String> = match source {
            ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
                ..
            } => {
                let types: Vec<&str> = image_types.iter().map(|s| s.as_str()).collect();
                self.fetch_card_image(
                    cache_key.clone(),
                    item_id.clone(),
                    series_id.clone(),
                    &types,
                );
                Some(cache_key.clone())
            }
            ArtworkSource::AudiobookshelfCover {
                library_item_id,
                book,
            } => match *book {
                true => self.audiobookshelf_book_cover_key(library_item_id),
                false => self.audiobookshelf_cover_key(library_item_id),
            },
        };
        let Some(cache_key) = cache_key else {
            return State::None;
        };
        if self.card_image_loading.contains(&cache_key) {
            return State::Loading;
        }
        let Some(entry) = self.card_image_states.get(&cache_key) else {
            return State::Loading;
        };
        let Some(source_img) = entry.img.as_ref() else {
            // Resolved-empty fetch: no artwork exists; the placeholder is
            // final.
            return State::None;
        };
        let decoded = {
            use image::GenericImageView;
            Some(source_img.dimensions())
        };
        // The Wide header is cover-fit: re-encode keyed by the box size.
        if crate::app::render::wide_hero_fits(panel_area) {
            if let Some(panes) = crate::app::render::arrangements::library::wide_library_panes(
                panel_area,
                PANE_PAD_X,
                PANE_PAD_Y,
                list_pane_width,
            ) {
                let box_cells =
                    crate::app::components::library_panel::hero_header::hero_artwork_box(
                        panes.hero_area,
                        facts,
                        workspace_present,
                    );
                if !self.ensure_hero_cover_protocol(&cache_key, (box_cells.width, box_cells.height))
                {
                    return State::Loading;
                }
            }
        }
        State::Ready { cache_key, decoded }
    }
}

impl App {
    /// Triggers the Audiobookshelf cover fetch for `library_item_id` and
    /// returns its image cache key, or `None` with no server configured.
    /// The hero projection (task 5.10) and the un-migrated painters'
    /// `paint_home_image` Audiobookshelf arm both resolve the key here, so
    /// the fetch dedupes on the one reservation.
    pub(in crate::app) fn audiobookshelf_cover_key(
        &mut self,
        library_item_id: &str,
    ) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        if self.images_enabled() {
            self.fetch_audiobookshelf_cover(setup.server_url.clone(), library_item_id.to_string());
        }
        Some(audiobookshelf_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.current_protocol_suffix(),
        ))
    }

    /// Triggers the Audiobookshelf book-cover fetch for `library_item_id` and
    /// returns its isolated image cache key, or `None` with no server
    /// configured. The book-browsing spec requires book artwork to remain
    /// isolated from podcast artwork (line 124).
    pub(in crate::app) fn audiobookshelf_book_cover_key(
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
        Some(audiobookshelf_book_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.current_protocol_suffix(),
        ))
    }
}
