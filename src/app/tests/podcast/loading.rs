//! App-level loading tests for the podcast tab's lazy, scoped episode
//! fan-out (reorganize-podcast-pill-navigation 3.3, design D5): the active
//! pill decides the required shows, requests are bounded in flight, each
//! show is fetched at most once per session, and results append as they
//! arrive. The fetch boundary is hermetic: the fixture's Audiobookshelf
//! setup carries an empty server URL, so the spawned request thread fails
//! client construction immediately without touching the network.

use super::*;
use mbv_core::config::{AudiobookshelfSetup, ServiceKind};

/// A podcast tab whose listed shows have no cached episodes yet, with a
/// configured Audiobookshelf Service whose URL fails client construction
/// (no network) and a secret file so the setup/key lookup succeeds.
fn unfetched_podcast_app(shows: usize) -> App {
    let mut app = make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup = Some(AudiobookshelfSetup::new(String::new()));
    mbv_core::config::save_service_secret(ServiceKind::Audiobookshelf, "test-token").unwrap();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state = crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState::new(
        library.clone(),
    );
    let shows: Vec<mbv_core::audiobookshelf::AudiobookshelfShow> = (0..shows)
        .map(|i| mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: format!("show-{i}"),
            title: format!("Show {i}"),
            author: None,
            description: None,
            cover_path: None,
        })
        .collect();
    state.append_page(0, 20, shows.len(), shows);
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

fn loading(
    state: &crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState,
) -> Vec<String> {
    let mut ids: Vec<String> = state.detail_loading_ids.keys().cloned().collect();
    ids.sort();
    ids
}

fn episode(show: &str, id: &str) -> mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
    mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
        library_item_id: show.into(),
        episode_id: id.into(),
        title: id.into(),
        description: None,
        published_at: None,
        duration_seconds: None,
    }
}

#[test]
fn fan_out_skips_cached_and_in_flight_shows() {
    let mut app = unfetched_podcast_app(6);
    app.audiobookshelf_browse[0].cache_detail("show-0".into(), Vec::new());
    app.audiobookshelf_browse[0]
        .detail_loading_ids
        .insert("show-1".into(), 0);

    app.commit_audiobookshelf_podcast_state_scope();

    assert_eq!(
        loading(&app.audiobookshelf_browse[0]),
        ["show-1", "show-2", "show-3", "show-4"],
        "already-cached and already-in-flight shows are not re-requested"
    );
}

#[test]
fn failed_fetch_consumes_the_session_request_instead_of_looping() {
    let mut app = unfetched_podcast_app(6);
    app.commit_audiobookshelf_podcast_state_scope();

    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: app.audiobookshelf_runtime.generation(),
        request: app.audiobookshelf_browse[0].detail_loading_ids["show-0"],
        library_item_id: "show-0".into(),
        result: Err(
            match mbv_core::audiobookshelf::AudiobookshelfClient::new("") {
                Err(error) => error,
                Ok(_) => unreachable!("an empty server URL cannot build a client"),
            },
        ),
    });

    let state = &app.audiobookshelf_browse[0];
    assert_eq!(
        loading(state),
        ["show-1", "show-2", "show-3", "show-4"],
        "the failed show retires and the batch continues"
    );
    // Re-arming the fan-out must not re-request the failed show.
    app.start_audiobookshelf_podcast_fan_out(0);
    let state = &app.audiobookshelf_browse[0];
    assert_eq!(loading(state), ["show-1", "show-2", "show-3", "show-4"]);
    assert!(state.detail_cache.contains_key("show-0"));
}

#[test]
fn stale_fetch_after_service_replacement_is_rejected() {
    let mut app = unfetched_podcast_app(2);
    let stale_generation = mbv_core::service_runtime::SetupGeneration::new(
        app.audiobookshelf_runtime.generation().value() + 1,
    );

    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: stale_generation,
        request: 0,
        library_item_id: "show-0".into(),
        result: Ok(Vec::new()),
    });

    let state = &app.audiobookshelf_browse[0];
    assert!(!state.detail_cache.contains_key("show-0"));
}

#[test]
fn activation_with_a_fetch_in_flight_does_not_double_fetch_and_the_newer_result_wins() {
    let mut app = unfetched_podcast_app(3);
    // show-1 landed; show-2's fetch thread is still running from the
    // pre-activation fan-out.
    app.audiobookshelf_browse[0].cache_detail("show-1".into(), Vec::new());
    app.start_audiobookshelf_detail("show-2".into());
    let in_flight = app.audiobookshelf_browse[0].detail_loading_ids["show-2"];

    app.activate_audiobookshelf_position(0);

    let state = &app.audiobookshelf_browse[0];
    assert_eq!(
        state.detail_loading_ids.get("show-2"),
        Some(&in_flight),
        "activation keeps the running fetch's own request: it is never re-requested"
    );
    assert!(
        !state.detail_cache.contains_key("show-1"),
        "the landed shows' caches are replaced"
    );
    assert_eq!(loading(state), ["show-0", "show-1", "show-2"]);

    // The still-running pre-activation fetch lands after the activation: it
    // is the show's one cache write.
    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: app.audiobookshelf_runtime.generation(),
        request: in_flight,
        library_item_id: "show-2".into(),
        result: Ok(vec![episode("show-2", "fresh")]),
    });

    // A replay of the retired request cannot overwrite the newer result.
    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: app.audiobookshelf_runtime.generation(),
        request: in_flight,
        library_item_id: "show-2".into(),
        result: Ok(vec![episode("show-2", "replayed")]),
    });
    let state = &app.audiobookshelf_browse[0];
    assert_eq!(
        state.detail_cache.get("show-2").map(|episodes| episodes
            .iter()
            .map(|episode| episode.episode_id.as_str())
            .collect::<Vec<_>>()),
        Some(vec!["fresh"]),
        "the newer result wins: the stale response never overwrites it"
    );

    // A superseded serial cannot retire a live request's mark either:
    // show-0's re-request keeps its own serial and its slot.
    let show0_serial = app.audiobookshelf_browse[0].detail_loading_ids["show-0"];
    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: app.audiobookshelf_runtime.generation(),
        request: in_flight,
        library_item_id: "show-0".into(),
        result: Ok(vec![episode("show-0", "misdelivered")]),
    });
    let state = &app.audiobookshelf_browse[0];
    assert_eq!(
        state.detail_loading_ids.get("show-0"),
        Some(&show0_serial),
        "the superseded response leaves the live request's mark alone"
    );
    assert!(!state.detail_cache.contains_key("show-0"));
    assert_eq!(
        loading(state),
        ["show-0", "show-1"],
        "the show was fetched exactly once per session"
    );
}
