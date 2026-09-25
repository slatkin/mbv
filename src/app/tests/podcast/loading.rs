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
fn start_detail_without_service_setup_does_not_leak_the_in_flight_mark() {
    let mut app = make_app_stub();
    // No Audiobookshelf setup at all: the setup/key lookup fails.
    app.audiobookshelf_libraries
        .push(mbv_core::audiobookshelf::AudiobookshelfLibrary {
            id: "abs-podcasts".into(),
            name: "ABS Podcasts".into(),
            media_type: "podcast".into(),
        });
    app.audiobookshelf_browse.push(
        crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState::new(
            mbv_core::audiobookshelf::AudiobookshelfLibrary {
                id: "abs-podcasts".into(),
                name: "ABS Podcasts".into(),
                media_type: "podcast".into(),
            },
        ),
    );
    app.audiobookshelf_browse[0]
        .shows
        .push(mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-0".into(),
            title: "Show 0".into(),
            author: None,
            description: None,
            cover_path: None,
        });
    app.tab = TabSelection::AudiobookshelfLibrary(0);

    app.start_audiobookshelf_detail("show-0".into());

    assert!(
        app.audiobookshelf_browse[0].detail_loading_ids.is_empty(),
        "the early return on missing setup must not leave the show marked in flight"
    );
}

#[test]
fn state_pill_scope_fans_out_bounded_across_every_listed_show() {
    let mut app = unfetched_podcast_app(6);

    app.commit_audiobookshelf_podcast_state_scope();

    let state = &app.audiobookshelf_browse[0];
    assert_eq!(state.committed_show_pill, None);
    assert_eq!(
        loading(state),
        ["show-0", "show-1", "show-2", "show-3"],
        "the batch starts the required shows in pill-bar order up to the in-flight cap"
    );
}

#[test]
fn show_pill_scope_costs_one_request_for_that_show() {
    let mut app = unfetched_podcast_app(6);

    app.select_audiobookshelf_show_target("show-2");

    let state = &app.audiobookshelf_browse[0];
    assert_eq!(state.committed_show_pill.as_deref(), Some("show-2"));
    assert_eq!(loading(state), ["show-2"], "other shows are not required");
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
fn detail_completion_continues_the_bounded_batch() {
    let mut app = unfetched_podcast_app(6);
    app.commit_audiobookshelf_podcast_state_scope();

    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: app.audiobookshelf_runtime.generation(),
        request: app.audiobookshelf_browse[0].detail_loading_ids["show-0"],
        library_item_id: "show-0".into(),
        result: Ok(Vec::new()),
    });

    let state = &app.audiobookshelf_browse[0];
    assert!(state.detail_cache.contains_key("show-0"));
    assert_eq!(
        loading(state),
        ["show-1", "show-2", "show-3", "show-4"],
        "the retired request's slot starts the next required show"
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
fn tab_activation_rerequests_the_active_pills_required_shows() {
    // A state pill (the default scope): every listed show's cached episodes
    // are replaced by re-requests.
    let mut app = unfetched_podcast_app(3);
    for i in 0..3 {
        app.audiobookshelf_browse[0].cache_detail(format!("show-{i}"), Vec::new());
    }

    app.activate_audiobookshelf_position(0);

    let state = &app.audiobookshelf_browse[0];
    assert!(
        state.detail_cache.is_empty(),
        "cached episodes are replaced"
    );
    assert_eq!(
        loading(state),
        ["show-0", "show-1", "show-2"],
        "the re-requests are bounded by the same fan-out cap"
    );

    // A show pill re-requests only that show's episodes; the other shows'
    // caches survive.
    let mut app = unfetched_podcast_app(3);
    for i in 0..3 {
        app.audiobookshelf_browse[0].cache_detail(format!("show-{i}"), vec![]);
    }
    app.select_audiobookshelf_show_target("show-1");
    app.activate_audiobookshelf_position(0);
    let state = &app.audiobookshelf_browse[0];
    assert!(state.detail_cache.contains_key("show-0"));
    assert!(state.detail_cache.contains_key("show-2"));
    assert!(
        loading(state) == ["show-1"],
        "only the pill's show re-requests"
    );
}

#[test]
fn rejected_generation_result_frees_its_slot_and_the_batch_continues() {
    let mut app = unfetched_podcast_app(6);
    app.commit_audiobookshelf_podcast_state_scope();
    let retired_generation = app.audiobookshelf_runtime.generation();
    // The Service is replaced while the batch is in flight; the response
    // below carries the generation it was spawned under.
    app.audiobookshelf_runtime.begin_setup();

    let retired_serial = app.audiobookshelf_browse[0].detail_loading_ids["show-3"];
    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: retired_generation,
        request: retired_serial,
        library_item_id: "show-3".into(),
        result: Ok(vec![episode("show-3", "rejected")]),
    });

    let state = &app.audiobookshelf_browse[0];
    assert!(
        !state.detail_cache.contains_key("show-3"),
        "the rejected-generation payload is discarded, never cached"
    );
    assert_ne!(
        state.detail_loading_ids.get("show-3"),
        Some(&retired_serial),
        "the rejected result retired its in-flight slot: the show re-requests under the live runtime instead of leaking the mark"
    );
    assert_eq!(
        loading(state),
        ["show-0", "show-1", "show-2", "show-3"],
        "the batch continues at the bounded cap"
    );
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

#[test]
fn refresh_fetches_shelves_only_for_the_selected_podcast_library() {
    use std::time::Duration;

    let mut app = crate::app::tests::podcast::audiobookshelf_app();
    let second = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts-2".into(),
        name: "Second podcast library".into(),
        media_type: "podcast".into(),
    };
    app.audiobookshelf_browse.push(
        crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState::new(
            second.clone(),
        ),
    );
    app.audiobookshelf_libraries.push(second);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    let expected_generation = app.audiobookshelf_runtime.generation();

    app.audiobookshelf_refresh();

    let mut shelf_events = Vec::new();
    while let Ok(event) = app.lib_rx.recv_timeout(Duration::from_secs(2)) {
        if let LibEvent::AudiobookshelfShelfFetched {
            generation,
            library_id,
            ..
        } = event
        {
            shelf_events.push((generation, library_id));
        }
    }
    assert_eq!(
        shelf_events,
        [(expected_generation, "abs-podcasts".into())],
        "refresh requests exactly the selected library's shelf"
    );
}

#[test]
fn refresh_key_resets_the_fan_out_scope_and_rerequests() {
    let mut app = unfetched_podcast_app(2);
    app.select_audiobookshelf_show_target("show-0");
    app.refresh_current_view();

    let state = &app.audiobookshelf_browse[0];
    assert_eq!(
        state.committed_show_pill, None,
        "the cleared list drops the show pill; the fan-out scope follows the state pills"
    );
    assert!(state.shows.is_empty());
    assert!(state.loading_pages.contains(&0));
}
