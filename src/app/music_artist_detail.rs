//! Shell-owned Grouped Music artist detail projection and request identity
//! (design D7, tasks 6.1–6.3).
//!
//! Artist roots are a presentation of the settled album catalog, not a second
//! provider-side library. This module keeps the asynchronous artist-track and
//! artist-artwork caches keyed by the source identity that produced them —
//! Library destination, Service setup generation, stable `ArtistItems` artist
//! ID, and settled catalog revision — and builds the one canonical Workspace
//! snapshot from that cache (or from the existing per-album caches when the
//! root only has a fallback artist key). Every request and completion crosses
//! a typed boundary variant with an exhaustive shell dispatch arm; a
//! completion that no longer matches its source identity is rejected here
//! rather than filtered downstream.

use std::collections::HashSet;

use mbv_core::api::EmbyItem;
use mbv_core::config::ServiceKind;
use mbv_core::service_runtime::SetupGeneration;

use crate::app::components::library_panel::content::HeroImageState;
use crate::app::components::library_panel::{LibraryKey, LibraryKind};
use crate::app::components::msg::MusicArtistTarget;
use crate::app::music_grouping::ArtistKey;
use crate::app::render::MusicWideRenderCtx;
use crate::app::ui_util::sort_audio_tracks;
use crate::app::{App, LibEvent};

/// At most this many fallback artist per-album track fetches run at once. A
/// fallback root's scope can be the whole settled catalog, so arming every
/// album on focus would issue one request per album in one burst; the queue
/// arms the bound and each arrival frees a slot for the next album.
const MAX_ARTIST_FALLBACK_TRACK_FETCHES: usize = 6;

/// The cache identity of one artist-detail query. Every axis is part of the
/// identity so a late response from a replaced browse snapshot, a re-settled
/// catalog, or a Service re-setup can never become the Workspace for the new
/// source.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub(super) struct ArtistDetailKey {
    pub(super) destination: LibraryKey,
    pub(super) generation: u64,
    pub(super) artist_id: String,
    pub(super) revision: u64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ArtistDetailCacheEntry {
    pub(super) tracks: Vec<EmbyItem>,
    /// A terminal query failure (the Service rejected or does not support the
    /// `ArtistIds` query). Projection switches to the documented per-album
    /// aggregation fallback, and repeated pushes do not turn the Service
    /// error into an unbounded request loop.
    pub(super) failed: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ArtistArtworkStatus {
    Loading,
    Ready,
    None,
}

/// Immediate facts for the focused artist. These derive from the settled
/// in-scope album set, so they are available while tracks/artwork are still
/// loading. Read by the task-6.4 Hero wiring.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ArtistSummary {
    pub(super) name: String,
    pub(super) album_count: usize,
    pub(super) year_start: Option<u32>,
    pub(super) year_end: Option<u32>,
}

impl ArtistSummary {
    pub(super) fn year_span(&self) -> Option<String> {
        match (self.year_start, self.year_end) {
            (Some(start), Some(end)) if start != end => Some(format!("{start}\u{2013}{end}")),
            (Some(year), _) | (_, Some(year)) => Some(year.to_string()),
            (None, None) => None,
        }
    }
}

/// One settled album's slice of the artist Workspace snapshot (task 6.3):
/// tracks of that album only, in disc/track order.
#[derive(Clone)]
pub(super) struct ArtistTrackGroup {
    pub(super) album_id: String,
    pub(super) album_title: String,
    pub(super) tracks: Vec<EmbyItem>,
}

/// The shell-owned projection for one focused artist root. Its summary facts
/// and artwork feed the Music Hero's artist arm and its grouped rows the
/// canonical track Workspace (`MusicContent`, task 6.4).
#[derive(Clone)]
pub(super) struct ArtistDetailProjection {
    pub(super) target: MusicArtistTarget,
    /// Immediate artist facts (task 6.2): the Hero's artist name, in-scope
    /// album count, and year span.
    pub(super) summary: ArtistSummary,
    pub(super) track_groups: Vec<ArtistTrackGroup>,
    /// Projected artwork state (task 6.2) painted by the Hero's artist arm.
    pub(super) artwork: HeroImageState,
    /// The artist's typed image cache key; with a stable artist ID it builds
    /// the Hero's artwork source (task 6.4), and a fallback root has none.
    pub(super) artwork_cache_key: Option<String>,
}

pub(super) fn artist_artwork_cache_key(
    destination: &LibraryKey,
    generation: u64,
    artist_id: &str,
) -> String {
    let destination_id = match destination {
        LibraryKey::Service { library_id, .. } => library_id.as_str(),
        LibraryKey::Home => "home",
        LibraryKey::Feeds => "feeds",
    };
    format!("artist:{generation}:{destination_id}:{artist_id}:Primary")
}

/// The Service album identity a tree target addresses. Duplicate album rows
/// (equal `EmbyItem` ids) are addressed by the tree's opaque `id\0index`
/// target; both track caches key on the Emby id before that local occurrence
/// suffix.
fn target_album_id(target: &str) -> &str {
    target.split('\0').next().unwrap_or(target)
}

fn parse_year(year: &str) -> Option<u32> {
    year.trim().parse().ok().filter(|year: &u32| *year > 0)
}

fn artist_destination_index(app: &App, destination: &LibraryKey) -> Option<usize> {
    let LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id,
        kind: LibraryKind::Music,
    } = destination
    else {
        return None;
    };
    app.libs
        .iter()
        .position(|lib| lib.library.id == *library_id && lib.library.collection_type == "music")
}

/// The settled in-scope album IDs of one artist under the identity that
/// requested it. Empty unless the destination still exists, the settled
/// catalog is still that revision, and the artist root is still part of it —
/// the guard a completion must pass before its result is kept.
fn scoped_album_ids(
    app: &App,
    destination: &LibraryKey,
    revision: u64,
    artist_id: &str,
) -> Vec<String> {
    let Some(index) = artist_destination_index(app, destination) else {
        return Vec::new();
    };
    let Some(level) = app.libs[index].nav_stack.last() else {
        return Vec::new();
    };
    let Some(catalog) = level
        .music_grouping
        .as_ref()
        .and_then(|state| state.settled.as_ref())
    else {
        return Vec::new();
    };
    if catalog.revision != revision {
        return Vec::new();
    }
    catalog
        .entries
        .iter()
        .filter(|entry| entry.artist_key == ArtistKey::Service(artist_id.to_string()))
        .map(|entry| entry.album_id.clone())
        .collect()
}

fn current_artist_scope(
    app: &App,
    destination: &LibraryKey,
    revision: u64,
    artist_id: &str,
) -> bool {
    !scoped_album_ids(app, destination, revision, artist_id).is_empty()
}

/// The albums the focused root's settled leaves address, in settled order:
/// the raw item index (the `album_info` row), the leaf's row target, the
/// Service album ID both track caches key on, and the display title.
fn current_album_ids(
    context: &MusicWideRenderCtx,
    target: &MusicArtistTarget,
) -> Vec<(usize, String, String, String)> {
    let scope: HashSet<&str> = target.album_targets.iter().map(String::as_str).collect();
    context
        .album_order
        .iter()
        .filter_map(|&index| {
            let row_target = context.album_targets.get(index)?;
            if !scope.contains(row_target.as_str()) {
                return None;
            }
            let item = context.list.items.get(index)?;
            let title = context
                .album_info
                .get(index)
                .map(|(_, _, title)| title.clone())
                .unwrap_or_else(|| item.display_name());
            Some((index, row_target.clone(), item.id.clone(), title))
        })
        .collect()
}

/// Whether one artist-track result belongs to the settled album carrying
/// `album_id`. Only the Service album ID may match: the `ArtistIds` Audio
/// query this cache holds is user-scoped and never requests `AlbumId`, so a
/// track with no album ID is a live payload shape, and a title match would
/// admit another same-titled album's tracks as playable rows. A track the
/// Service omitted an album ID for can still reach the Workspace through the
/// fallback path's per-album fetches, which carry real album IDs.
pub(super) fn track_matches_album(track: &EmbyItem, album_id: &str) -> bool {
    !album_id.is_empty() && track.album_id == album_id
}

impl App {
    pub(super) fn artist_detail_key(
        &self,
        destination: &LibraryKey,
        target: &MusicArtistTarget,
    ) -> Option<ArtistDetailKey> {
        Some(ArtistDetailKey {
            destination: destination.clone(),
            generation: self.emby_runtime.generation().value(),
            artist_id: target.artist_id.clone()?,
            revision: target.revision,
        })
    }

    /// Start the verified `ArtistIds` Audio query (task 1.4's client
    /// operation). A fallback artist has no provider identity, so this arm
    /// deliberately starts no artist query and instead arms the already-
    /// supported per-album fetches the fallback aggregates from, through the
    /// bounded fallback queue rather than one request per in-scope album.
    pub(super) fn request_artist_tracks(
        &mut self,
        destination: LibraryKey,
        target: MusicArtistTarget,
    ) {
        let Some(artist_id) = target.artist_id.clone() else {
            let album_ids = target
                .album_targets
                .iter()
                .map(|album_target| target_album_id(album_target).to_string())
                .collect();
            self.enqueue_artist_album_tracks(album_ids);
            return;
        };

        let key = ArtistDetailKey {
            destination,
            generation: self.emby_runtime.generation().value(),
            artist_id,
            revision: target.revision,
        };
        if self.artist_detail_cache.contains_key(&key)
            || !self.artist_detail_loading.insert(key.clone())
        {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            self.artist_detail_loading.remove(&key);
            self.artist_detail_cache.insert(
                key,
                ArtistDetailCacheEntry {
                    tracks: Vec::new(),
                    failed: true,
                },
            );
            return;
        };
        let tx = self.lib_tx.clone();
        let destination = key.destination.clone();
        let generation = SetupGeneration::new(key.generation);
        let revision = key.revision;
        let artist_id = key.artist_id.clone();
        std::thread::spawn(move || {
            let result = client.get_artist_audio_tracks(&artist_id);
            let _ = tx.send(LibEvent::ArtistTracksFetched {
                destination,
                generation,
                artist_id,
                revision,
                result,
            });
        });
    }

    /// Queue the fallback aggregation's per-album fetches (design D7). The
    /// aggregation source stays the existing per-album cache, but a fallback
    /// root's scope can cover the whole settled catalog, so arming every album
    /// at once would flood the Service. Albums already cached or already armed
    /// are skipped, repeats dedupe against the pending queue, and the drain
    /// runs at most `MAX_ARTIST_FALLBACK_TRACK_FETCHES` at a time; each
    /// arrival arms the next album, so rows appear progressively from cache.
    pub(super) fn enqueue_artist_album_tracks(&mut self, album_ids: Vec<String>) {
        for album_id in album_ids {
            if album_id.is_empty()
                || self.album_tracks_cache.contains_key(&album_id)
                || self.album_tracks_loading.contains(&album_id)
                || self
                    .pending_artist_album_track_fetches
                    .iter()
                    .any(|pending| pending == &album_id)
            {
                continue;
            }
            self.pending_artist_album_track_fetches.push_back(album_id);
        }
        self.drain_artist_album_track_fetches();
    }

    /// Starts queued fallback fetches while the bounded fan-out has capacity.
    /// `fetch_album_tracks` remains the sole gate for the actual request, so
    /// an album the selection path already armed is skipped without consuming
    /// a slot, and a request that could not start never occupies one.
    pub(super) fn drain_artist_album_track_fetches(&mut self) {
        while self.artist_album_track_fetches_in_flight.len() < MAX_ARTIST_FALLBACK_TRACK_FETCHES {
            let Some(album_id) = self.pending_artist_album_track_fetches.pop_front() else {
                break;
            };
            if self.album_tracks_cache.contains_key(&album_id)
                || self.album_tracks_loading.contains(&album_id)
            {
                continue;
            }
            self.fetch_album_tracks(album_id.clone());
            if self.album_tracks_loading.contains(&album_id) {
                self.artist_album_track_fetches_in_flight.insert(album_id);
            }
        }
    }

    /// Artist artwork uses the existing image/cache worker, addressed by the
    /// artist's stable item ID. The small identity map lets the generic image
    /// completion become an explicit typed artist completion without teaching
    /// the image worker about Music's source identity. A fallback artist has
    /// no provider ID: the explicit no-artwork arm records that final state
    /// and never borrows a root album image.
    pub(super) fn request_artist_artwork(
        &mut self,
        destination: LibraryKey,
        target: MusicArtistTarget,
    ) {
        let Some(key) = self.artist_detail_key(&destination, &target) else {
            return;
        };
        let cache_key = artist_artwork_cache_key(&key.destination, key.generation, &key.artist_id);
        if let Some(status) = self.artist_artwork_status.get(&key) {
            // `Loading` and `None` are terminal for this identity. `Ready` is
            // terminal only while the decoded bitmap is still cached: the
            // image LRU (`shell_run`) can evict it, and a `Ready` status
            // without its `card_image_states` entry would project
            // `HeroImageState::None` forever for this revision. Treat that
            // cache miss as un-cached and re-arm, matching the album card
            // path, whose `card_image_states` membership is its fetch dedup.
            let bitmap_cached = self
                .card_image_states
                .get(&cache_key)
                .is_some_and(|entry| entry.img.is_some());
            if *status != ArtistArtworkStatus::Ready || bitmap_cached {
                return;
            }
            self.artist_artwork_status.remove(&key);
        }
        if let Some(entry) = self.card_image_states.get(&cache_key) {
            self.artist_artwork_status.insert(
                key,
                if entry.img.is_some() {
                    ArtistArtworkStatus::Ready
                } else {
                    ArtistArtworkStatus::None
                },
            );
            return;
        }
        if !self.images_enabled() {
            self.artist_artwork_status
                .insert(key, ArtistArtworkStatus::None);
            return;
        }
        self.artist_artwork_status
            .insert(key.clone(), ArtistArtworkStatus::Loading);
        let artist_id = key.artist_id.clone();
        self.artist_artwork_requests.insert(cache_key.clone(), key);
        self.fetch_card_image(cache_key, artist_id, String::new(), &["Primary"]);
    }

    pub(super) fn handle_artist_tracks_fetched(
        &mut self,
        destination: LibraryKey,
        generation: SetupGeneration,
        artist_id: String,
        revision: u64,
        result: Result<Vec<EmbyItem>, String>,
    ) {
        let key = ArtistDetailKey {
            destination: destination.clone(),
            generation: generation.value(),
            artist_id: artist_id.clone(),
            revision,
        };
        // Retire only this exact request. A newer revision has a different
        // key and must remain loading when this old response arrives.
        self.artist_detail_loading.remove(&key);
        if !self.emby_runtime.accepts(generation)
            || !current_artist_scope(self, &destination, revision, &artist_id)
        {
            return;
        }
        // Bound the cache: an artist keeps only its latest source identity.
        self.artist_detail_cache.retain(|cached, _| {
            cached.destination != key.destination
                || cached.artist_id != key.artist_id
                || *cached == key
        });
        match result {
            Ok(mut tracks) => {
                sort_audio_tracks(&mut tracks);
                self.artist_detail_cache.insert(
                    key,
                    ArtistDetailCacheEntry {
                        tracks,
                        failed: false,
                    },
                );
            }
            Err(_) => {
                self.artist_detail_cache.insert(
                    key,
                    ArtistDetailCacheEntry {
                        tracks: Vec::new(),
                        failed: true,
                    },
                );
                // The documented fallback for an unavailable artist-ID query
                // is the per-album aggregation; queue those fetches for the
                // root's still-current in-scope albums behind the bounded
                // fan-out rather than arming them all at once.
                let album_ids = scoped_album_ids(self, &destination, revision, &artist_id);
                self.enqueue_artist_album_tracks(album_ids);
            }
        }
    }

    pub(super) fn handle_artist_artwork_fetched(
        &mut self,
        destination: LibraryKey,
        generation: SetupGeneration,
        artist_id: String,
        revision: u64,
        cache_key: String,
        available: bool,
    ) {
        if !self.emby_runtime.accepts(generation)
            || cache_key != artist_artwork_cache_key(&destination, generation.value(), &artist_id)
            || !current_artist_scope(self, &destination, revision, &artist_id)
        {
            return;
        }
        let key = ArtistDetailKey {
            destination,
            generation: generation.value(),
            artist_id,
            revision,
        };
        self.artist_artwork_status.insert(
            key,
            if available {
                ArtistArtworkStatus::Ready
            } else {
                ArtistArtworkStatus::None
            },
        );
    }

    /// Project one artist root into the existing Music context (tasks 6.2/6.3).
    /// The context still contains the complete settled album tree; only the
    /// artist detail is added, so the tree remains the sole browser owner and
    /// the task-6.4 content switch reads one field.
    pub(super) fn project_music_artist_detail(
        &self,
        destination: &LibraryKey,
        mut context: MusicWideRenderCtx,
        target: MusicArtistTarget,
    ) -> MusicWideRenderCtx {
        let albums = current_album_ids(&context, &target);
        // Immediate summary facts (task 6.2): the in-scope album count and
        // the settled albums' release-year span, derived without any fetch.
        let mut years = Vec::new();
        for &(index, _, _, _) in &albums {
            if let Some(year) = context
                .album_info
                .get(index)
                .and_then(|(_, year, _)| parse_year(year))
            {
                years.push(year);
            }
        }
        years.sort_unstable();
        let summary = ArtistSummary {
            name: target.artist_name.clone(),
            album_count: albums.len(),
            year_start: years.first().copied(),
            year_end: years.last().copied(),
        };

        let artist_entry = self
            .artist_detail_key(destination, &target)
            .and_then(|key| self.artist_detail_cache.get(&key));
        let artist_cache = artist_entry
            .filter(|entry| !entry.failed)
            .map(|entry| entry.tracks.as_slice());
        // A missing/loading artist result must not leak partial per-album rows
        // under an ID-backed artist. Once the ID query has definitively
        // failed, the existing per-album cache is the documented fallback.
        let use_album_fallback =
            target.artist_id.is_none() || artist_entry.is_some_and(|entry| entry.failed);
        // Settled album order, then disc/track order within each album
        // (`sort_audio_tracks`); duplicate titles stay distinct rows because
        // the rows are keyed by the tracks' own stable IDs.
        let mut track_groups = Vec::new();
        for (_, _, album_id, album_title) in &albums {
            let mut tracks: Vec<EmbyItem> = if use_album_fallback {
                self.album_tracks_cache
                    .get(album_id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                artist_cache
                    .unwrap_or_default()
                    .iter()
                    .filter(|track| track_matches_album(track, album_id))
                    .cloned()
                    .collect()
            };
            sort_audio_tracks(&mut tracks);
            if !tracks.is_empty() {
                track_groups.push(ArtistTrackGroup {
                    album_id: album_id.clone(),
                    album_title: album_title.clone(),
                    tracks,
                });
            }
        }

        let (artwork, artwork_cache_key) =
            if let Some(key) = self.artist_detail_key(destination, &target) {
                let cache_key =
                    artist_artwork_cache_key(&key.destination, key.generation, &key.artist_id);
                let state = match self.artist_artwork_status.get(&key) {
                    Some(ArtistArtworkStatus::Loading) => HeroImageState::Loading,
                    Some(ArtistArtworkStatus::None) | None => HeroImageState::None,
                    Some(ArtistArtworkStatus::Ready) => self
                        .card_image_states
                        .get(&cache_key)
                        .and_then(|entry| {
                            entry.img.as_ref().map(|image| {
                                use image::GenericImageView;
                                HeroImageState::Ready {
                                    cache_key: cache_key.clone(),
                                    decoded: Some(image.dimensions()),
                                }
                            })
                        })
                        .unwrap_or(HeroImageState::None),
                };
                (state, Some(cache_key))
            } else {
                (HeroImageState::None, None)
            };

        // The artist push addresses no album: the Workspace rows and Hero
        // come from the projection below, and the album snapshot's own
        // selected album/track cache go quiet while an artist root is
        // focused.
        context.selected_album = None;
        context.album_tracks = None;
        context.artist_detail = Some(ArtistDetailProjection {
            target,
            summary,
            track_groups,
            artwork,
            artwork_cache_key,
        });
        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::library_panel::content::HeroImageState as State;
    use crate::app::music_grouping::{build_grouped_album_catalog, MusicGroupingState};
    use crate::app::render::make_music_group_app;
    use crate::app::render::MusicWideRenderCtx;
    use crate::app::tests::make_item;
    use mbv_core::api::EmbyArtistRef;
    use std::collections::HashMap;

    fn destination() -> LibraryKey {
        LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id: "lib-music".into(),
            kind: LibraryKind::Music,
        }
    }

    /// A configured-but-unroutable client: the loading reservation
    /// `fetch_album_tracks`/`fetch_card_image` performs happens synchronously
    /// before the doomed network attempt, so it is observable immediately
    /// without a live server.
    fn unroutable_emby_runtime() -> mbv_core::service_runtime::EmbyRuntime {
        let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
        client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
            server_url: "http://127.0.0.1:1".into(),
            user_id: "user-id".into(),
            token: "token".into(),
        });
        mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(std::sync::Mutex::new(
            client,
        )))
    }

    /// A settled one-album catalog whose only album carries a Service
    /// (`ArtistItems`) artist identity at revision 7.
    fn settled_artist_app() -> (App, MusicWideRenderCtx, MusicArtistTarget, LibraryKey) {
        let mut app = make_music_group_app();
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        level.items[0].artist_items = vec![EmbyArtistRef {
            name: "Alpha".into(),
            id: "artist-alpha".into(),
        }];
        let mut catalog = build_grouped_album_catalog(&level.items, &HashMap::new());
        catalog.revision = 7;
        catalog.parent_id = level.parent_id.clone();
        level.music_grouping = Some(MusicGroupingState {
            revision: 7,
            candidate: None,
            settled: Some(catalog),
        });
        let context = app.wide_music_render_ctx(0, Some(0));
        let target = MusicArtistTarget {
            artist_id: Some("artist-alpha".into()),
            artist_name: "Alpha".into(),
            album_targets: vec![context.album_targets[0].clone()],
            revision: 7,
        };
        (app, context, target, destination())
    }

    #[test]
    fn artist_projection_uses_settled_summary_and_disc_track_order() {
        let (mut app, context, target, destination) = settled_artist_app();
        let key = app
            .artist_detail_key(&destination, &target)
            .expect("artist ID key");
        let mut disc_two = make_item("Same Title", "Audio");
        disc_two.id = "track-2".into();
        disc_two.album_id = "album-1".into();
        disc_two.parent_index_number = 2;
        disc_two.index_number = 1;
        let mut disc_one = make_item("Same Title", "Audio");
        disc_one.id = "track-1".into();
        disc_one.album_id = "album-1".into();
        disc_one.parent_index_number = 1;
        disc_one.index_number = 2;
        app.artist_detail_cache.insert(
            key,
            ArtistDetailCacheEntry {
                tracks: vec![disc_two, disc_one],
                failed: false,
            },
        );

        let projected = app.project_music_artist_detail(&destination, context, target);
        let detail = projected.artist_detail.expect("artist detail");
        assert_eq!(detail.summary.album_count, 1);
        assert_eq!(detail.summary.year_span().as_deref(), Some("2001"));
        assert!(projected.selected_album.is_none());
        assert_eq!(detail.track_groups.len(), 1);
        assert_eq!(
            detail.track_groups[0]
                .tracks
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            ["track-1", "track-2"],
            "disc 1 track 2 precedes disc 2 track 1 and duplicate titles stay distinct"
        );
    }

    /// `settled_artist_app` plus a second in-scope album under the same
    /// display title but its own Service identity: the leaf targets
    /// (`id\0index`) and the Service album IDs keep the two groups distinct.
    fn duplicate_title_artist_app() -> (App, MusicWideRenderCtx, MusicArtistTarget, LibraryKey) {
        let (mut app, _context, mut target, destination) = settled_artist_app();
        let mut second = make_item("First Album", "MusicAlbum");
        second.id = "album-2".into();
        second.artist = "Alpha".into();
        second.production_year = 1999;
        app.libs[0].nav_stack[1].items.push(second);
        app.libs[0].nav_stack[1].total_count = 2;
        {
            let level = app.libs[0].nav_stack.last_mut().unwrap();
            for item in &mut level.items {
                item.artist_items = vec![EmbyArtistRef {
                    name: "Alpha".into(),
                    id: "artist-alpha".into(),
                }];
            }
            let mut catalog = build_grouped_album_catalog(&level.items, &HashMap::new());
            catalog.revision = 7;
            catalog.parent_id = level.parent_id.clone();
            level.music_grouping = Some(MusicGroupingState {
                revision: 7,
                candidate: None,
                settled: Some(catalog),
            });
        }
        let context = app.wide_music_render_ctx(0, None);
        target.album_targets = context.album_targets.clone();
        (app, context, target, destination)
    }

    #[test]
    fn artist_groups_follow_settled_album_order_with_duplicate_album_titles() {
        let (mut app, context, target, destination) = duplicate_title_artist_app();
        let mut track_a = make_item("Song", "Audio");
        track_a.id = "track-a".into();
        track_a.album_id = "album-1".into();
        let mut track_b = make_item("Song", "Audio");
        track_b.id = "track-b".into();
        track_b.album_id = "album-2".into();
        let key = app
            .artist_detail_key(&destination, &target)
            .expect("artist ID key");
        app.artist_detail_cache.insert(
            key,
            ArtistDetailCacheEntry {
                tracks: vec![track_b, track_a],
                failed: false,
            },
        );

        let projected = app.project_music_artist_detail(&destination, context, target);
        let detail = projected.artist_detail.expect("artist detail");
        assert_eq!(detail.summary.album_count, 2);
        assert_eq!(
            detail.summary.year_span().as_deref(),
            Some("1999\u{2013}2001")
        );
        assert_eq!(
            detail
                .track_groups
                .iter()
                .map(|group| group.album_id.as_str())
                .collect::<Vec<_>>(),
            ["album-1", "album-2"],
            "groups follow settled album order even with equal titles"
        );
        assert_eq!(detail.track_groups[0].tracks[0].id, "track-a");
        assert_eq!(detail.track_groups[1].tracks[0].id, "track-b");
    }

    /// The `ArtistIds` Audio query is user-scoped and omits `AlbumId`, so a
    /// track with an empty album ID and a matching album title is a live
    /// payload shape. Two in-scope albums can share that title: a title match
    /// would put one album's track in both groups and admit a foreign album's
    /// track as a playable row. Only Service album ID membership may project
    /// an artist-track row.
    #[test]
    fn empty_album_id_artist_tracks_never_cross_into_same_titled_groups() {
        let (mut app, context, target, destination) = duplicate_title_artist_app();
        let mut track_a = make_item("Song", "Audio");
        track_a.id = "track-a".into();
        track_a.album_id = "album-1".into();
        let mut track_b = make_item("Song", "Audio");
        track_b.id = "track-b".into();
        track_b.album_id = "album-2".into();
        // `make_item` leaves `album_id` empty: the live payload shape that a
        // title match would admit into both same-titled groups.
        let mut anonymous = make_item("Song", "Audio");
        anonymous.id = "track-no-album".into();
        anonymous.album = "First Album".into();
        let key = app
            .artist_detail_key(&destination, &target)
            .expect("artist ID key");
        app.artist_detail_cache.insert(
            key,
            ArtistDetailCacheEntry {
                tracks: vec![anonymous, track_a, track_b],
                failed: false,
            },
        );

        let projected = app.project_music_artist_detail(&destination, context, target);
        let detail = projected.artist_detail.expect("artist detail");
        let groups: Vec<Vec<&str>> = detail
            .track_groups
            .iter()
            .map(|group| group.tracks.iter().map(|track| track.id.as_str()).collect())
            .collect();
        assert_eq!(
            groups,
            vec![vec!["track-a"], vec!["track-b"]],
            "each same-titled album keeps only its own ID-matched track, and \
             the empty-album-ID track is never projected"
        );
    }

    #[test]
    fn fallback_artist_aggregates_album_cache_without_artist_artwork() {
        let (mut app, context, _, destination) = settled_artist_app();
        let mut track = make_item("Fallback", "Audio");
        track.id = "fallback-track".into();
        track.album_id = "album-1".into();
        app.album_tracks_cache.insert("album-1".into(), vec![track]);
        let target = MusicArtistTarget {
            artist_id: None,
            artist_name: "Alpha".into(),
            album_targets: vec![context.album_targets[0].clone()],
            revision: 7,
        };

        let projected = app.project_music_artist_detail(&destination, context, target);
        let detail = projected.artist_detail.expect("fallback detail");
        assert_eq!(detail.summary.album_count, 1);
        assert_eq!(detail.summary.name, "Alpha");
        assert_eq!(detail.track_groups[0].tracks[0].id, "fallback-track");
        assert_eq!(detail.artwork, State::None);
        assert!(detail.artwork_cache_key.is_none());
    }

    /// A cached fallback album projects immediately and is never re-armed: the
    /// bounded queue skips cached albums before consuming a slot, so the
    /// projection aggregates existing per-album results without a fetch.
    #[test]
    fn fallback_artist_projects_cached_albums_without_arming_a_fetch() {
        let (mut app, context, _, destination) = settled_artist_app();
        app.emby_runtime = unroutable_emby_runtime();
        let mut track = make_item("Cached", "Audio");
        track.id = "cached-track".into();
        track.album_id = "album-1".into();
        app.album_tracks_cache.insert("album-1".into(), vec![track]);
        let target = MusicArtistTarget {
            artist_id: None,
            artist_name: "Alpha".into(),
            album_targets: vec![context.album_targets[0].clone()],
            revision: 7,
        };

        app.request_artist_tracks(destination.clone(), target.clone());
        assert!(
            app.album_tracks_loading.is_empty(),
            "a cached album is projected from cache, never re-armed"
        );

        let projected = app.project_music_artist_detail(&destination, context, target);
        let detail = projected.artist_detail.expect("fallback detail");
        assert_eq!(detail.track_groups[0].tracks[0].id, "cached-track");
    }

    /// A fallback root's scope can be the whole settled catalog, so one focus
    /// must not arm one fetch per in-scope album. The queue arms the bound and
    /// each arrival frees exactly one slot for the next album, so rows keep
    /// appearing progressively instead of flooding the Service.
    #[test]
    fn fallback_artist_scope_arms_only_the_bounded_fetch_window() {
        let (mut app, _context, _target, destination) = settled_artist_app();
        app.emby_runtime = unroutable_emby_runtime();
        let target = MusicArtistTarget {
            artist_id: None,
            artist_name: "Alpha".into(),
            album_targets: (1..=8).map(|index| format!("album-{index}")).collect(),
            revision: 7,
        };

        app.request_artist_tracks(destination, target);

        assert_eq!(
            app.album_tracks_loading.len(),
            MAX_ARTIST_FALLBACK_TRACK_FETCHES,
            "one fallback focus arms at most the bounded fetch window"
        );
        assert_eq!(
            app.pending_artist_album_track_fetches.len(),
            8 - MAX_ARTIST_FALLBACK_TRACK_FETCHES,
            "the rest of the scope waits for a slot"
        );

        app.handle_lib_event(LibEvent::AlbumTracksFetched {
            album_id: "album-1".into(),
            tracks: Vec::new(),
        });

        assert!(app.album_tracks_cache.contains_key("album-1"));
        assert_eq!(
            app.album_tracks_loading.len(),
            MAX_ARTIST_FALLBACK_TRACK_FETCHES,
            "the arrival frees exactly one slot and the drain refills it"
        );
        assert!(
            app.album_tracks_loading
                .contains(&format!("album-{}", MAX_ARTIST_FALLBACK_TRACK_FETCHES + 1)),
            "the next waiting album takes the freed slot"
        );
        assert_eq!(
            app.pending_artist_album_track_fetches.len(),
            8 - MAX_ARTIST_FALLBACK_TRACK_FETCHES - 1
        );
    }

    #[test]
    fn loading_artist_result_projects_no_partial_album_rows() {
        let (mut app, context, target, destination) = settled_artist_app();
        app.album_tracks_cache
            .insert("album-1".into(), vec![make_item("Leak", "Audio")]);

        let projected = app.project_music_artist_detail(&destination, context, target);
        let detail = projected.artist_detail.expect("artist detail");
        assert!(
            detail.track_groups.is_empty(),
            "a still-loading ID query must not leak per-album rows under the artist"
        );
    }

    #[test]
    fn ready_artist_artwork_rearms_after_its_bitmap_is_evicted() {
        let (mut app, _context, target, destination) = settled_artist_app();
        app.image_protocol_enabled = true;
        app.emby_runtime = unroutable_emby_runtime();
        let key = app
            .artist_detail_key(&destination, &target)
            .expect("artist ID key");
        let cache_key = artist_artwork_cache_key(&destination, key.generation, &key.artist_id);
        // A completed artist fetch: the decoded bitmap is cached and the
        // status is terminal for this source identity.
        app.card_image_states.insert(
            cache_key.clone(),
            crate::app::images::CachedImage {
                img: Some(image::DynamicImage::ImageRgba8(
                    image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])),
                )),
                protocols: HashMap::new(),
                cover_box: None,
                applied_logo_key: None,
            },
        );
        app.artist_artwork_status
            .insert(key.clone(), ArtistArtworkStatus::Ready);
        app.request_artist_artwork(destination.clone(), target.clone());
        assert_eq!(
            app.artist_artwork_status.get(&key),
            Some(&ArtistArtworkStatus::Ready),
            "a cached Ready bitmap stays terminal and starts no redundant fetch"
        );
        assert!(
            app.artist_artwork_requests.is_empty(),
            "a cached Ready bitmap registers no request identity"
        );

        // The image LRU (`shell_run`) evicts the decoded bitmap while the
        // status stays `Ready`; without the re-arm the projection would fall
        // back to `HeroImageState::None` forever for this revision.
        app.card_image_states.remove(&cache_key);
        app.request_artist_artwork(destination, target);
        assert_eq!(
            app.artist_artwork_status.get(&key),
            Some(&ArtistArtworkStatus::Loading),
            "a Ready status without its bitmap is a cache miss and re-arms"
        );
        assert!(
            app.card_image_loading.contains(&cache_key),
            "the re-armed request reserves its stable-ID cache key"
        );
        assert_eq!(app.artist_artwork_requests.get(&cache_key), Some(&key));
    }

    #[test]
    fn stale_artist_completion_is_rejected_and_cache_hit_does_not_refetch() {
        let (mut app, _context, target, destination) = settled_artist_app();
        let generation = app.emby_runtime.generation();
        app.handle_artist_tracks_fetched(
            destination.clone(),
            generation,
            "artist-alpha".into(),
            6,
            Ok(Vec::new()),
        );
        assert!(
            app.artist_detail_cache.is_empty(),
            "a replaced snapshot's completion is rejected, not cached"
        );

        let key = app
            .artist_detail_key(&destination, &target)
            .expect("artist ID key");
        app.artist_detail_cache
            .insert(key.clone(), ArtistDetailCacheEntry::default());
        app.request_artist_tracks(destination, target);
        assert!(app.artist_detail_loading.is_empty());
        assert!(app.artist_detail_cache.contains_key(&key));
    }

    #[test]
    fn failed_artist_query_falls_back_to_per_album_fetches() {
        let (mut app, _context, _target, destination) = settled_artist_app();
        app.emby_runtime = unroutable_emby_runtime();
        let generation = app.emby_runtime.generation();
        app.handle_artist_tracks_fetched(
            destination.clone(),
            generation,
            "artist-alpha".into(),
            7,
            Err("unsupported".into()),
        );
        let key = ArtistDetailKey {
            destination,
            generation: generation.value(),
            artist_id: "artist-alpha".into(),
            revision: 7,
        };
        assert!(
            app.artist_detail_cache
                .get(&key)
                .is_some_and(|entry| entry.failed),
            "the failure is cached so pushes cannot loop the query"
        );
        assert!(
            app.album_tracks_loading.contains("album-1"),
            "the per-album fallback fetches are armed on failure"
        );
    }
}
