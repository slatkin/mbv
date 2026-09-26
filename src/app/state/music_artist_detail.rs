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

use crate::app::components::library_panel::{LibraryKey, LibraryKind};
use crate::app::components::msg::MusicArtistTarget;
use crate::app::render::MusicWideRenderCtx;
use crate::app::state::music_grouping::ArtistKey;
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
pub(in crate::app) struct ArtistDetailKey {
    pub(in crate::app) destination: LibraryKey,
    pub(in crate::app) generation: u64,
    pub(in crate::app) artist_id: String,
    pub(in crate::app) revision: u64,
}

#[derive(Debug, Clone, Default)]
pub(in crate::app) struct ArtistDetailCacheEntry {
    pub(in crate::app) tracks: Vec<EmbyItem>,
    /// A terminal query failure (the Service rejected or does not support the
    /// `ArtistIds` query). Projection switches to the documented per-album
    /// aggregation fallback, and repeated pushes do not turn the Service
    /// error into an unbounded request loop.
    pub(in crate::app) failed: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::app) enum ArtistArtworkStatus {
    Loading,
    Ready,
    None,
}

/// Immediate facts for the focused artist. These derive from the settled
/// in-scope album set, so they are available while tracks/artwork are still
/// loading. Read by the task-6.4 Hero wiring.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(in crate::app) struct ArtistSummary {
    pub(in crate::app) name: String,
    pub(in crate::app) album_count: usize,
    pub(in crate::app) year_start: Option<u32>,
    pub(in crate::app) year_end: Option<u32>,
}

impl ArtistSummary {
    pub(in crate::app) fn year_span(&self) -> Option<String> {
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
pub(in crate::app) struct ArtistTrackGroup {
    pub(in crate::app) album_id: String,
    pub(in crate::app) album_title: String,
    pub(in crate::app) tracks: Vec<EmbyItem>,
}

/// The shell-owned projection for one focused artist root. Its summary facts
/// and grouped rows feed the Music Hero and canonical track Workspace
/// (`MusicContent`, task 6.4). The legacy typed artist-artwork cache remains
/// retained for its completion machinery, but the Hero image source is now the
/// selected album's existing artwork path.
#[derive(Clone)]
pub(in crate::app) struct ArtistDetailProjection {
    pub(in crate::app) target: MusicArtistTarget,
    /// Immediate artist facts (task 6.2): the Hero's artist name, in-scope
    /// album count, and year span.
    pub(in crate::app) summary: ArtistSummary,
    pub(in crate::app) track_groups: Vec<ArtistTrackGroup>,
}

pub(in crate::app) fn artist_artwork_cache_key(
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
                .map_or_else(|| item.display_name(), |(_, _, title)| title.clone());
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
pub(in crate::app) fn track_matches_album(track: &EmbyItem, album_id: &str) -> bool {
    !album_id.is_empty() && track.album_id == album_id
}

impl App {
    pub(in crate::app) fn artist_detail_key(
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
    pub(in crate::app) fn request_artist_tracks(
        &mut self,
        destination: LibraryKey,
        target: &MusicArtistTarget,
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
    pub(in crate::app) fn enqueue_artist_album_tracks(&mut self, album_ids: Vec<String>) {
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
    pub(in crate::app) fn drain_artist_album_track_fetches(&mut self) {
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
    pub(in crate::app) fn request_artist_artwork(
        &mut self,
        destination: &LibraryKey,
        target: &MusicArtistTarget,
    ) {
        let Some(key) = self.artist_detail_key(destination, target) else {
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

    pub(in crate::app) fn handle_artist_tracks_fetched(
        &mut self,
        destination: &LibraryKey,
        generation: SetupGeneration,
        artist_id: &str,
        revision: u64,
        result: Result<Vec<EmbyItem>, String>,
    ) {
        let key = ArtistDetailKey {
            destination: destination.clone(),
            generation: generation.value(),
            artist_id: artist_id.to_string(),
            revision,
        };
        // Retire only this exact request. A newer revision has a different
        // key and must remain loading when this old response arrives.
        self.artist_detail_loading.remove(&key);
        if !self.emby_runtime.accepts(generation)
            || !current_artist_scope(self, destination, revision, artist_id)
        {
            return;
        }
        // Bound the cache: an artist keeps only its latest source identity.
        self.artist_detail_cache.retain(|cached, _| {
            cached.destination != key.destination
                || cached.artist_id != key.artist_id
                || *cached == key
        });
        if let Ok(mut tracks) = result {
            sort_audio_tracks(&mut tracks);
            self.artist_detail_cache.insert(
                key,
                ArtistDetailCacheEntry {
                    tracks,
                    failed: false,
                },
            );
        } else {
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
            let album_ids = scoped_album_ids(self, destination, revision, artist_id);
            self.enqueue_artist_album_tracks(album_ids);
        }
    }

    pub(in crate::app) fn handle_artist_artwork_fetched(
        &mut self,
        destination: LibraryKey,
        generation: SetupGeneration,
        artist_id: &str,
        revision: u64,
        cache_key: &str,
        available: bool,
    ) {
        if !self.emby_runtime.accepts(generation)
            || cache_key != artist_artwork_cache_key(&destination, generation.value(), artist_id)
            || !current_artist_scope(self, &destination, revision, artist_id)
        {
            return;
        }
        let key = ArtistDetailKey {
            destination,
            generation: generation.value(),
            artist_id: artist_id.to_string(),
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
    /// the task-6.4 content switch reads one field. Album artwork is resolved
    /// later by `MusicContent` from the selected artist album/track group.
    pub(in crate::app) fn project_music_artist_detail(
        &self,
        destination: &LibraryKey,
        mut context: MusicWideRenderCtx,
        target: &MusicArtistTarget,
    ) -> MusicWideRenderCtx {
        let albums = current_album_ids(&context, target);
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
            .artist_detail_key(destination, target)
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

        // The artist push addresses no album: the Workspace rows and Hero
        // come from the projection below, and the album snapshot's own
        // selected album/track cache go quiet while an artist root is
        // focused.
        context.selected_album = None;
        context.album_tracks = None;
        context.artist_detail = Some(ArtistDetailProjection {
            target: target.clone(),
            summary,
            track_groups,
        });
        context
    }
}

#[cfg(test)]
mod tests;
