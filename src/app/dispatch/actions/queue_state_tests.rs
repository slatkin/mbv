use super::*;
use crate::app::ContextAction;
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry,
};
use rstest::rstest;

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
        mbv_config::set_test_env_var("XDG_CONFIG_HOME", &dir);
        mbv_config::remove_test_env_var("MBV_SYSTEM");
        let state_dir = crate::config::TestStateDirGuard::new_at(dir.join("mbv"));
        Self {
            dir,
            _state_dir: state_dir,
        }
    }
}

impl Drop for XdgHomeGuard {
    fn drop(&mut self) {
        mbv_config::remove_test_env_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&self.dir);
    }
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
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(AudiobookshelfQueueItem {
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
        })),
        QueueItem::Audiobookshelf(AudiobookshelfItem::Book(AudiobookshelfBookQueueItem {
            library_item_id: "book-1".into(),
            title: "Book 1".into(),
            author: None,
            duration_ticks: Some(200),
            position_ticks: 84,
            played: false,
            is_finished: false,
            cover_path: None,
        })),
    ]
}

fn assert_attached_context_selection_preserves_local_queue(action: ContextAction) {
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("session-1".into());
    app.local_view
        .adopt_items(crate::app::tests::make_items(2), app.local_view.cursor());
    app.local_view.set_cursor(1);
    let before: Vec<_> = app
        .local_view
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();

    app.execute_context_action(Some(action), None);

    let after: Vec<_> = app
        .local_view
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();
    assert_eq!(after, before);
    assert_eq!(app.local_view.cursor(), 1);
}

#[rstest]
#[case::play(ContextAction::PlaySelection as fn(Vec<EmbyItem>) -> ContextAction)]
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
fn audiobookshelf_service_removal_and_replacement_wait_for_owner_queue_snapshot() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();
    let mixed = mixed_audiobookshelf_queue();
    let mut app = crate::app::tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup = Some(mbv_config::AudiobookshelfSetup::new(
        "https://old-books.example",
    ));
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "old-secret").unwrap();
    app.local_view.adopt_queue_items(mixed.clone(), 2);
    app.remote_view = Some(crate::app::state::queue_view::QueueView::empty());
    app.remote_view
        .as_mut()
        .unwrap()
        .adopt_queue_items(mixed.clone(), 3);

    // D5: Service cleanup requests owner edits; adopted views stay intact
    // until the owner publishes the resulting queue snapshot.
    app.remove_audiobookshelf_confirmed();

    assert_eq!(app.local_view.slots().len(), mixed.len());
    assert_eq!(app.remote_view.as_ref().unwrap().slots().len(), mixed.len());
    assert!(
        app.local_view
            .slots()
            .iter()
            .any(|slot| slot.item.is_audiobookshelf())
    );

    // Refill the projections and make the local slot active: this is the
    // local Bound replacement path, while remote_view remains remote Bound.
    let mixed = mixed_audiobookshelf_queue();
    app.config.lock().unwrap().audiobookshelf_setup = Some(mbv_config::AudiobookshelfSetup::new(
        "https://replacement-books.example",
    ));
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "replacement-secret")
        .unwrap();
    app.local_view
        .adopt_queue_items_with_active(mixed.clone(), 2, 2);
    app.player.status.lock().unwrap().active = true;
    app.remote_view = Some(crate::app::state::queue_view::QueueView::empty());
    app.remote_view
        .as_mut()
        .unwrap()
        .adopt_queue_items_with_active(mixed.clone(), 3, 3);
    let generation = app.audiobookshelf_runtime.generation();
    app.setup.pending_audiobookshelf_replacement = Some(
        crate::app::dispatch::session::service_startup::AudiobookshelfPendingReplacement {
            candidate:
                crate::app::dispatch::session::service_startup::AudiobookshelfValidatedCandidate {
                    setup: mbv_config::AudiobookshelfSetup::new(
                        "https://replacement-books.example",
                    ),
                    user: mbv_audiobookshelf::AudiobookshelfUser {
                        id: "reader-id".into(),
                        username: "reader".into(),
                    },
                    api_key: "replacement-secret".into(),
                },
            previous_state: mbv_core::service_runtime::ServiceState::Ready,
        },
    );

    app.replace_audiobookshelf_confirmed(generation);

    assert_eq!(app.local_view.slots().len(), mixed.len());
    assert_eq!(app.remote_view.as_ref().unwrap().slots().len(), mixed.len());
    assert!(
        app.local_view
            .slots()
            .iter()
            .any(|slot| slot.item.is_audiobookshelf())
    );
}
