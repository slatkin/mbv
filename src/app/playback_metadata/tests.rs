//! Tests for the Client-side item-metadata fetches (add-tauri-gui-client
//! design D4a): the fetch seams map each kind's response into the
//! decorations, failures project as empty, the drain lands completions in
//! the cache, and the projection reads it. The fill's own tests live in
//! `crate::app::tests::emby_client_fill`.

use super::*;

use crate::app::app_stub::make_app_stub;

/// A URL that never leaves the process: the mock transport ignores it, but
/// it must remain an IP literal so the default resolver never attempts DNS.
const TEST_URL: &str = "http://127.0.0.1:1";

fn emby_client(http: &mbv_net::mock_http::MockHttp) -> mbv_emby::EmbyClient {
    mbv_emby::EmbyClient::new(crate::config::Config {
        server_url: TEST_URL.into(),
        ..crate::config::Config::default()
    })
    .with_test_agent(http.agent())
}

#[test]
fn emby_fetch_maps_chapters_marks_and_trickplay() {
    let http = mbv_net::mock_http::MockHttp::new();
    http.respond(
        200,
        r#"{"Chapters":[{"Name":"Opening","StartPositionTicks":0},{"Name":"Act Two","StartPositionTicks":600000000}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"s1","Type":"Intro","StartTicks":100,"EndTicks":200},{"Id":"s2","Type":"Commercial","StartTicks":250,"EndTicks":260},{"Id":"s3","Type":"Outro","StartTicks":300,"EndTicks":400}]}"#,
    );
    http.respond(
        200,
        r#"{"854":{"Width":854,"Height":480,"TileWidth":10,"TileHeight":10,"ThumbnailCount":100,"Interval":10000,"MediaSourceId":"ms1"}}"#,
    );

    let decorations = fetch_emby_decorations(&emby_client(&http), "item1");

    assert_eq!(
        decorations.chapters,
        vec![
            PlaybackChapter {
                name: "Opening".to_string(),
                start_ticks: 0,
            },
            PlaybackChapter {
                name: "Act Two".to_string(),
                start_ticks: 600_000_000,
            },
        ]
    );
    assert_eq!(
        decorations.intro,
        Some(SegmentMark {
            start_ticks: 100,
            end_ticks: 200,
        })
    );
    assert_eq!(
        decorations.credits,
        Some(SegmentMark {
            start_ticks: 300,
            end_ticks: 400,
        })
    );
    assert_eq!(
        decorations.trickplay,
        Some(vec![TrickplayGrid {
            width: 854,
            height: 480,
            tile_width: 10,
            tile_height: 10,
            thumbnail_count: 100,
            interval_ms: 10_000,
            media_source_id: "ms1".to_string(),
        }])
    );
}

#[test]
fn emby_fetch_failure_projects_empty() {
    let http = mbv_net::mock_http::MockHttp::new();
    http.respond(500, "");
    http.respond(404, "");
    http.respond(500, "");

    let decorations = fetch_emby_decorations(&emby_client(&http), "item1");

    assert_eq!(decorations, PlaybackDecorations::default());
}

#[test]
fn audiobookshelf_fetch_maps_book_chapters() {
    let http = mbv_net::mock_http::MockHttp::new();
    http.respond(
        200,
        r#"{"id":"book1","media":{"chapters":[{"id":0,"start":0.0,"end":60.0,"title":"Ch 1"},{"id":1,"start":60.0,"end":120.0,"title":"Ch 2"}]}}"#,
    );
    let client = mbv_audiobookshelf::AudiobookshelfClient::new(TEST_URL)
        .expect("test URL must build a client")
        .with_test_agent(http.agent());

    let decorations = fetch_audiobookshelf_decorations(&client, "key", "book1");

    assert_eq!(
        decorations,
        PlaybackDecorations {
            chapters: vec![
                PlaybackChapter {
                    name: "Ch 1".to_string(),
                    start_ticks: 0,
                },
                PlaybackChapter {
                    name: "Ch 2".to_string(),
                    start_ticks: 600_000_000,
                },
            ],
            ..PlaybackDecorations::default()
        }
    );
}

#[test]
fn audiobookshelf_fetch_failure_projects_empty() {
    let http = mbv_net::mock_http::MockHttp::new();
    http.respond(500, "");
    let client = mbv_audiobookshelf::AudiobookshelfClient::new(TEST_URL)
        .expect("test URL must build a client")
        .with_test_agent(http.agent());

    let decorations = fetch_audiobookshelf_decorations(&client, "key", "book1");

    assert_eq!(decorations, PlaybackDecorations::default());
}

/// The projection reads the cache the drain fills: a seeded decoration for
/// the active item lands in the playback projection (design D4a), and an
/// item with no cached entry projects empty.
#[test]
fn projection_reads_the_active_items_cached_decorations() {
    use crate::app::tests::QueueViewTestExt;

    let mut app = make_app_stub();
    app.local_view.adopt_queue_items_with_active(
        vec![mbv_queue::QueueItem::Emby(Box::new(
            mbv_emby_model::test_support::make_item("Item 0", "Movie"),
        ))],
        0,
        0,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 1;
    });
    let key = app
        .playback_queue()
        .item_at(0)
        .expect("the stub queue has a first slot")
        .content_id();
    app.playback_metadata_cache.insert(
        key,
        PlaybackDecorations {
            chapters: vec![PlaybackChapter {
                name: "Opening".to_string(),
                start_ticks: 0,
            }],
            intro: Some(SegmentMark {
                start_ticks: 100,
                end_ticks: 200,
            }),
            credits: Some(SegmentMark {
                start_ticks: 300,
                end_ticks: 400,
            }),
            trickplay: Some(vec![TrickplayGrid {
                width: 854,
                height: 480,
                tile_width: 10,
                tile_height: 10,
                thumbnail_count: 100,
                interval_ms: 10_000,
                media_source_id: "ms1".to_string(),
            }]),
        },
    );

    let projection = app.queue_playback_projection();

    assert_eq!(projection.chapters.len(), 1);
    assert_eq!(projection.chapters[0].name, "Opening");
    assert_eq!(
        projection.intro_mark,
        Some(SegmentMark {
            start_ticks: 100,
            end_ticks: 200,
        })
    );
    assert_eq!(
        projection.credits_mark,
        Some(SegmentMark {
            start_ticks: 300,
            end_ticks: 400,
        })
    );
    assert_eq!(projection.trickplay.as_ref().map(Vec::len), Some(1));
}

/// The drain moves a landed worker result into the cache the projection
/// reads and frees the loading slot.
#[test]
fn drain_moves_landed_decorations_into_the_cache() {
    let mut app = make_app_stub();
    let key = QueueItemContentId::Emby("item1".to_string());
    app.playback_metadata_loading.insert(key.clone());
    app.channels
        .metadata_tx
        .send((
            key.clone(),
            PlaybackDecorations {
                chapters: vec![PlaybackChapter {
                    name: "Opening".to_string(),
                    start_ticks: 0,
                }],
                ..PlaybackDecorations::default()
            },
        ))
        .expect("the test channel accepts the send");

    assert!(app.drain_playback_metadata_events());

    assert!(!app.playback_metadata_loading.contains(&key));
    assert_eq!(app.playback_metadata_cache[&key].chapters.len(), 1);
}
