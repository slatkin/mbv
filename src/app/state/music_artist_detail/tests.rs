use super::*;
use crate::app::render::make_music_group_app;
use crate::app::render::MusicWideRenderCtx;
use crate::app::state::music_grouping::{build_grouped_album_catalog, MusicGroupingState};
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

    let projected = app.project_music_artist_detail(&destination, context, &target);
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
    };
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

    let projected = app.project_music_artist_detail(&destination, context, &target);
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

    let projected = app.project_music_artist_detail(&destination, context, &target);
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

    let projected = app.project_music_artist_detail(&destination, context, &target);
    let detail = projected.artist_detail.expect("fallback detail");
    assert_eq!(detail.summary.album_count, 1);
    assert_eq!(detail.summary.name, "Alpha");
    assert_eq!(detail.track_groups[0].tracks[0].id, "fallback-track");
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

    app.request_artist_tracks(destination.clone(), &target);
    assert!(
        app.album_tracks_loading.is_empty(),
        "a cached album is projected from cache, never re-armed"
    );

    let projected = app.project_music_artist_detail(&destination, context, &target);
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

    app.request_artist_tracks(destination, &target);

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

    let projected = app.project_music_artist_detail(&destination, context, &target);
    let detail = projected.artist_detail.expect("artist detail");
    assert!(
        detail.track_groups.is_empty(),
        "a still-loading ID query must not leak per-album rows under the artist"
    );
}

#[test]
fn cached_artist_artwork_is_adopted_without_a_fetch() {
    let (mut app, _context, target, destination) = settled_artist_app();
    app.image_protocol_enabled = true;
    let key = app
        .artist_detail_key(&destination, &target)
        .expect("artist ID key");
    let cache_key = artist_artwork_cache_key(&destination, key.generation, &key.artist_id);
    app.card_image_states.insert(
        cache_key,
        crate::app::images::CachedImage {
            img: Some(image::DynamicImage::ImageRgba8(
                image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])),
            )),
            protocols: HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        },
    );

    app.request_artist_artwork(&destination, &target);

    assert_eq!(
        app.artist_artwork_status.get(&key),
        Some(&ArtistArtworkStatus::Ready)
    );
    assert!(app.artist_artwork_requests.is_empty());
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
    app.request_artist_artwork(&destination, &target);
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
    app.request_artist_artwork(&destination, &target);
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
fn artist_request_without_client_caches_a_failed_completion() {
    let (mut app, _context, target, destination) = settled_artist_app();
    let key = app
        .artist_detail_key(&destination, &target)
        .expect("artist ID key");

    app.request_artist_tracks(destination, &target);

    assert!(app.artist_detail_loading.is_empty());
    assert!(app
        .artist_detail_cache
        .get(&key)
        .is_some_and(|entry| entry.failed));
}

#[test]
fn artist_artwork_completion_requires_the_current_identity() {
    let (mut app, _context, target, destination) = settled_artist_app();
    let generation = app.emby_runtime.generation();
    let key = app
        .artist_detail_key(&destination, &target)
        .expect("artist ID key");
    let cache_key = artist_artwork_cache_key(&destination, key.generation, &key.artist_id);

    app.handle_artist_artwork_fetched(
        destination.clone(),
        generation,
        &key.artist_id,
        key.revision,
        cache_key.as_str(),
        true,
    );
    assert_eq!(
        app.artist_artwork_status.get(&key),
        Some(&ArtistArtworkStatus::Ready)
    );

    app.handle_artist_artwork_fetched(
        destination.clone(),
        generation,
        &key.artist_id,
        key.revision,
        "stale-cache-key".into(),
        false,
    );
    app.handle_artist_artwork_fetched(
        destination,
        generation,
        &key.artist_id,
        key.revision - 1,
        cache_key.as_str(),
        false,
    );
    assert_eq!(
        app.artist_artwork_status.get(&key),
        Some(&ArtistArtworkStatus::Ready)
    );
}

#[test]
fn stale_artist_completion_is_rejected_and_cache_hit_does_not_refetch() {
    let (mut app, _context, target, destination) = settled_artist_app();
    let generation = app.emby_runtime.generation();
    app.handle_artist_tracks_fetched(
        &destination.clone(),
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
    app.request_artist_tracks(destination, &target);
    assert!(app.artist_detail_loading.is_empty());
    assert!(app.artist_detail_cache.contains_key(&key));
}

#[test]
fn current_artist_completion_replaces_older_revision_cache() {
    let (mut app, _context, _target, destination) = settled_artist_app();
    let generation = app.emby_runtime.generation();
    let old_key = ArtistDetailKey {
        destination: destination.clone(),
        generation: generation.value(),
        artist_id: "artist-alpha".into(),
        revision: 6,
    };
    app.artist_detail_cache
        .insert(old_key.clone(), ArtistDetailCacheEntry::default());

    let mut later = make_item("Later", "Audio");
    later.id = "track-later".into();
    later.album_id = "album-1".into();
    let mut earlier = make_item("Earlier", "Audio");
    earlier.id = "track-earlier".into();
    earlier.album_id = "album-1".into();
    app.handle_artist_tracks_fetched(
        &destination.clone(),
        generation,
        "artist-alpha".into(),
        7,
        Ok(vec![later, earlier]),
    );

    let current_key = ArtistDetailKey {
        destination,
        generation: generation.value(),
        artist_id: "artist-alpha".into(),
        revision: 7,
    };
    assert!(!app.artist_detail_cache.contains_key(&old_key));
    let current = app
        .artist_detail_cache
        .get(&current_key)
        .expect("current completion is cached");
    assert!(!current.failed);
    assert_eq!(
        current
            .tracks
            .iter()
            .map(|track| track.id.as_str())
            .collect::<Vec<_>>(),
        ["track-earlier", "track-later"]
    );
}

#[test]
fn failed_artist_query_falls_back_to_per_album_fetches() {
    let (mut app, _context, _target, destination) = settled_artist_app();
    app.emby_runtime = unroutable_emby_runtime();
    let generation = app.emby_runtime.generation();
    app.handle_artist_tracks_fetched(
        &destination.clone(),
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
