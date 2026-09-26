use super::*;
use crate::app::tests::{confirm_replace_queue, make_item};
use crate::app::ContextAction;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfQueueItem, FeedEntry, QueueItem, QueueItemContentId,
};
use rstest::{fixture, rstest};

use crate::config::tests::SYS_ENV_LOCK as XDG_HOME_LOCK;

/// RAII guard that points `XDG_CONFIG_HOME` (subtitle-mode saves) and
/// test-only state-dir lookups (prefs/queue saves) at a fresh tempdir,
/// restoring and cleaning up on drop -- including on panic.
pub(super) struct XdgHomeGuard {
    dir: std::path::PathBuf,
    _state_dir: crate::config::TestStateDirGuard,
}

impl XdgHomeGuard {
    pub(super) fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("MBV_SYSTEM");
        let state_dir = crate::config::TestStateDirGuard::new_at(dir.join("mbv"));
        Self {
            dir,
            _state_dir: state_dir,
        }
    }
}

impl Drop for XdgHomeGuard {
    fn drop(&mut self) {
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
#[fixture]
fn make_queue_items() -> Vec<mbv_core::playback_queue::QueueItem> {
    crate::app::tests::make_items(3)
        .into_iter()
        .map(|i| mbv_core::playback_queue::QueueItem::Emby(Box::new(i)))
        .collect()
}

fn mixed_audiobookshelf_queue() -> Vec<QueueItem> {
    vec![
        QueueItem::Emby(Box::new(make_item("Emby", "Movie"))),
        QueueItem::Feed(FeedEntry {
            guid: "feed-entry".into(),
            title: "Feed entry".into(),
            enclosure_url: Some("https://example.test/feed.mp3".into()),
            link: None,
            mime_type: Some("audio/mpeg".into()),
            duration_ticks: Some(100),
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        }),
        QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
            library_item_id: "show-1".into(),
            episode_id: "episode-1".into(),
            title: "Episode 1".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: Some(100),
            position_ticks: 42,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        }),
        QueueItem::AudiobookshelfBook(AudiobookshelfBookQueueItem {
            library_item_id: "book-1".into(),
            title: "Book 1".into(),
            author: None,
            duration_ticks: Some(200),
            position_ticks: 84,
            played: false,
            is_finished: false,
            cover_path: None,
        }),
    ]
}

fn assert_audiobookshelf_queue_purged(items: &[QueueItem]) {
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0], QueueItem::Emby(item) if item.name == "Emby"));
    assert!(matches!(&items[1], QueueItem::Feed(item) if item.guid == "feed-entry"));
    assert!(items.iter().all(|item| !item.is_audiobookshelf_any()));
}

fn assert_context_selection_replaces_nonsequential_queue(action: ContextAction) {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.queue.append_with_id(
        mbv_core::playback_queue::QueueSlotId::from_raw(700),
        QueueItem::Emby(Box::new(make_item("stale", "Movie"))),
    );
    app.player_tab.queue.append_with_id(
        mbv_core::playback_queue::QueueSlotId::from_raw(900),
        QueueItem::Emby(Box::new(make_item("stale-2", "Movie"))),
    );
    app.execute_context_action(Some(action), None);
    // The target queue is populated, so the selection is held behind the
    // replacement gate; confirming runs its rebuild + submission.
    confirm_replace_queue(&mut app);

    let slots = app.player_tab.queue.slots();
    assert_eq!(slots.len(), 2);
    assert_eq!(
        slots
            .iter()
            .map(|slot| slot.slot_id.raw())
            .collect::<Vec<_>>(),
        vec![901, 902],
        "context playback must preserve monotonic replacement slot identities"
    );
    assert!(slots.iter().all(|slot| {
        matches!(&slot.item, QueueItem::Emby(item) if item.name == "selected" || item.name == "selected-2")
    }));
}

#[rstest]
#[case::play(ContextAction::PlaySelection as fn(Vec<EmbyItem>) -> ContextAction)]
#[case::shuffle(ContextAction::ShuffleSelection as fn(Vec<EmbyItem>) -> ContextAction)]
fn context_selection_rebuilds_canonical_queue_before_submission(
    #[case] action: fn(Vec<EmbyItem>) -> ContextAction,
) {
    let selected = vec![
        make_item("selected", "Movie"),
        make_item("selected-2", "Movie"),
    ];
    assert_context_selection_replaces_nonsequential_queue(action(selected));
}

fn assert_attached_context_selection_preserves_local_queue(action: ContextAction) {
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("session-1".into());
    app.player_tab.set_items(
        crate::app::tests::make_items(2),
        app.player_tab.queue_cursor,
    );
    app.player_tab.queue_cursor = 1;
    let before: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();

    app.execute_context_action(Some(action), None);

    let after: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();
    assert_eq!(after, before);
    assert_eq!(app.player_tab.queue_cursor, 1);
}

#[rstest]
#[case::play(ContextAction::PlaySelection as fn(Vec<EmbyItem>) -> ContextAction)]
#[case::shuffle(ContextAction::ShuffleSelection as fn(Vec<EmbyItem>) -> ContextAction)]
fn attached_context_selection_preserves_local_queue(
    #[case] action: fn(Vec<EmbyItem>) -> ContextAction,
) {
    let selected = vec![
        make_item("selected", "Movie"),
        make_item("selected-2", "Movie"),
    ];
    assert_attached_context_selection_preserves_local_queue(action(selected));
}

#[test]
fn audiobookshelf_service_removal_and_replacement_purge_all_queue_projections() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();
    let mixed = mixed_audiobookshelf_queue();
    let mut app = crate::app::tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("https://old-books.example"),
    );
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "old-secret",
    )
    .unwrap();
    app.player_tab.set_queue_items(mixed.clone(), 2);
    app.remote_player_tab = Some(crate::app::state::types::player_tab::PlayerTab::new(
        mixed.clone(),
        3,
    ));
    mbv_core::config::save_queue_state(&mbv_core::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: mixed.clone(),
        cursor: 3,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    })
    .unwrap();

    // A cold local queue is Composed; the remote tab is the remote Bound view.
    app.remove_audiobookshelf_confirmed();

    assert_audiobookshelf_queue_purged(&app.player_tab.all_queue_items());
    assert_audiobookshelf_queue_purged(&app.remote_player_tab.as_ref().unwrap().all_queue_items());
    assert_audiobookshelf_queue_purged(&mbv_core::config::load_queue_state().unwrap().items);

    // Refill the projections and make the local slot active: this is the
    // local Bound replacement path, while remote_player_tab remains remote Bound.
    let mixed = mixed_audiobookshelf_queue();
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("https://replacement-books.example"),
    );
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "replacement-secret",
    )
    .unwrap();
    app.player_tab.set_queue_items(mixed.clone(), 2);
    let active_slot = app.player_tab.slot_id_at(2).unwrap();
    assert!(matches!(
        app.player_tab.queue.set_active_slot(active_slot),
        mbv_core::playback_queue::QueueMutationResult::Applied(())
    ));
    app.player.status.lock().unwrap().active = true;
    app.remote_player_tab = Some(crate::app::state::types::player_tab::PlayerTab::new(
        mixed.clone(),
        3,
    ));
    mbv_core::config::save_queue_state(&mbv_core::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: mixed,
        cursor: 3,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    })
    .unwrap();
    let generation = app.audiobookshelf_runtime.generation();
    app.pending_audiobookshelf_replacement = Some(
        crate::app::dispatch::session::service_startup::AudiobookshelfPendingReplacement {
            candidate:
                crate::app::dispatch::session::service_startup::AudiobookshelfValidatedCandidate {
                    setup: mbv_core::config::AudiobookshelfSetup::new(
                        "https://replacement-books.example",
                    ),
                    user: mbv_core::audiobookshelf::AudiobookshelfUser {
                        id: "reader-id".into(),
                        username: "reader".into(),
                    },
                    api_key: "replacement-secret".into(),
                },
            previous_state: mbv_core::service_runtime::ServiceState::Ready,
        },
    );

    app.replace_audiobookshelf_confirmed(generation);

    assert_audiobookshelf_queue_purged(&app.player_tab.all_queue_items());
    assert_audiobookshelf_queue_purged(&app.remote_player_tab.as_ref().unwrap().all_queue_items());
    assert_audiobookshelf_queue_purged(&mbv_core::config::load_queue_state().unwrap().items);
}

#[rstest]
fn queue_restore_cursor_finds_last_played_by_id(
    make_queue_items: Vec<mbv_core::playback_queue::QueueItem>,
) {
    let items = make_queue_items;
    let cursor = queue_restore_cursor(&items, 0, None, Some("id1"), false);
    assert_eq!(cursor, 1);
}

#[rstest]
fn queue_restore_cursor_advances_past_a_completed_last_played_item(
    make_queue_items: Vec<mbv_core::playback_queue::QueueItem>,
) {
    let items = make_queue_items;
    let cursor = queue_restore_cursor(&items, 0, None, Some("id1"), true);
    assert_eq!(cursor, 2);
}

#[rstest]
fn queue_restore_cursor_falls_back_to_saved_cursor_when_last_played_id_missing(
    make_queue_items: Vec<mbv_core::playback_queue::QueueItem>,
) {
    let items = make_queue_items;
    // "id5" isn't in the restored list (e.g. it was removed from the
    // queue before quitting) — must fall back to the saved cursor, not
    // silently snap back to the front of the queue.
    let cursor = queue_restore_cursor(&items, 2, None, Some("id5"), false);
    assert_eq!(cursor, 2);
}

#[rstest]
fn queue_restore_cursor_falls_back_to_saved_cursor_clamped_to_len(
    make_queue_items: Vec<mbv_core::playback_queue::QueueItem>,
) {
    let items = make_queue_items;
    let cursor = queue_restore_cursor(&items, 99, None, Some("id5"), false);
    #[rustfmt::skip]
    assert_eq!(
        cursor, 2,
        "out-of-range saved cursor must clamp to the last valid index"
    );
}

#[rstest]
fn queue_restore_cursor_uses_saved_cursor_when_no_last_played_id(
    make_queue_items: Vec<mbv_core::playback_queue::QueueItem>,
) {
    let items = make_queue_items;
    let cursor = queue_restore_cursor(&items, 1, None, None, false);
    assert_eq!(cursor, 1);
}

#[test]
fn queue_restore_cursor_typed_identity_never_crosses_services() {
    let e = QueueItem::Emby(Box::new(make_item("same", "Movie")));
    let f = QueueItem::Feed(FeedEntry {
        guid: "same".into(),
        title: "f".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    });
    assert_eq!(
        queue_restore_cursor(
            &[f],
            0,
            Some(&QueueItemContentId::Emby("same".into())),
            None,
            false
        ),
        0
    );
    assert_eq!(
        queue_restore_cursor(
            &[e],
            0,
            Some(&QueueItemContentId::Feed("same".into())),
            None,
            false
        ),
        0
    );
}

#[test]
fn queue_restore_cursor_typed_feed_and_abs_are_provider_qualified() {
    let f = QueueItem::Feed(FeedEntry {
        guid: "same".into(),
        title: "f".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    });
    let a = QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
        library_item_id: "lib".into(),
        episode_id: "same".into(),
        title: "a".into(),
        show_title: None,
        author: None,
        description: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    });
    assert_eq!(
        queue_restore_cursor(
            &[f.clone(), a.clone()],
            0,
            Some(&QueueItemContentId::Feed("same".into())),
            None,
            false
        ),
        0
    );
    assert_eq!(
        queue_restore_cursor(
            &[f, a],
            0,
            Some(&QueueItemContentId::Audiobookshelf {
                library_item_id: "lib".into(),
                episode_id: "same".into()
            }),
            None,
            false
        ),
        1
    );
}

#[test]
fn queue_restore_cursor_legacy_ambiguous_id_uses_saved_cursor() {
    let items = vec![
        mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item("same", "Movie"))),
        mbv_core::playback_queue::QueueItem::Feed(FeedEntry {
            guid: "same".into(),
            title: "feed".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        }),
    ];
    assert_eq!(
        queue_restore_cursor(&items, 1, None, Some("same"), false),
        1
    );
}

#[test]
fn restore_queue_state_with_no_saved_file_does_nothing() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    app.restore_queue_state();

    assert!(app.player_tab.emby_items().is_empty());
}

#[test]
fn restore_queue_state_with_no_items_does_nothing() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: vec![],
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    })
    .expect("save queue state");

    let mut app = crate::app::tests::make_app_stub();
    app.restore_queue_state();

    assert!(app.player_tab.emby_items().is_empty());
}

#[test]
fn restore_queue_state_populates_queue_synchronously_from_disk() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let items = crate::app::tests::make_items(3);
    crate::config::save_queue_state(&crate::config::QueueState::from_emby_items(
        items,
        1,
        crate::config::QueueSource::Unknown,
    ))
    .expect("save queue state");

    let mut app = crate::app::tests::make_app_stub();
    app.restore_queue_state();

    // No network call is needed for the queue to already be correct —
    // this is a synchronous, local read, not a spawned background fetch.
    assert_eq!(app.player_tab.emby_items().len(), 3);
    assert_eq!(app.player_tab.queue_cursor, 1);
}

#[test]
fn restore_queue_state_clears_a_stale_dirty_flag() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: crate::app::tests::make_items(1)
            .into_iter()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    })
    .expect("save queue state");

    let mut app = crate::app::tests::make_app_stub();
    app.queue_dirty = true;
    app.restore_queue_state();

    assert!(
        !app.queue_dirty,
        "restoring a queue from disk is not a local edit — it must not \
         leave a stale dirty flag that could trigger an unwanted \
         save_playlist_to_emby() push on the next consume"
    );
}

#[test]
fn quit_preserves_saved_playlist_source_for_restart_restore() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(2),
        app.player_tab.queue_cursor,
    );
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-id".into()),
        name: "Saved Queue".into(),
    };
    app.queue_dirty = true;
    app.config.lock().unwrap().quit_timeout_secs = 0;

    assert!(app.try_quit());
    app.save_queue_state_no_clear();

    let state = crate::config::load_queue_state().expect("queue state should be saved");
    assert_eq!(
        state.source,
        crate::config::QueueSource::Playlist {
            id: Some("playlist-id".into()),
            name: "Saved Queue".into(),
        },
        "shutdown persistence must keep the saved-playlist association so \
         a restart can still autosave/consume against the playlist"
    );

    let mut restarted = crate::app::tests::make_app_stub();
    restarted.restore_queue_state();
    assert_eq!(restarted.player_tab.emby_items().len(), 2);
    assert_eq!(restarted.queue_source, state.source);
}

#[test]
fn queue_restore_cursor_uses_unique_last_played_identity() {
    let mut first = make_item("first", "Movie");
    first.id = "first".into();
    let mut second = make_item("second", "Movie");
    second.id = "second".into();
    let items = vec![
        QueueItem::Emby(Box::new(first)),
        QueueItem::Emby(Box::new(second)),
    ];
    assert_eq!(
        queue_restore_cursor(&items, 0, None, Some("second"), false),
        1
    );
}
