//! Audiobookshelf context menus follow the standard action set (context-menu
//! spec, "Library lists share one standard action set" and "Shuffle is
//! offered only for collections and multi-selections") and enqueue keeps the
//! carried target order (media-list-multi-select, "Play, Shuffle and Add to
//! Queue use list order"). Standard-media-context-menus task 5.6.

use super::*;
use mbv_ui_model::context_menu::AudiobookshelfMenuTarget;
use rstest::rstest;

fn episode_target(episode_id: &str) -> AudiobookshelfMenuTarget {
    AudiobookshelfMenuTarget::Episode {
        library_item_id: "show-a".into(),
        episode_id: episode_id.into(),
    }
}

/// Open the Audiobookshelf menu for `targets` and return the ordered entry
/// labels.
fn menu_labels(app: &mut App, targets: Vec<AudiobookshelfMenuTarget>) -> Vec<&'static str> {
    app.open_audiobookshelf_context_menu(targets, None);
    match app.pending_overlay.take() {
        Some(mbv_ui_model::overlay::OverlayRequest::ContextMenu(menu)) => {
            menu.entries.iter().map(|entry| entry.label).collect()
        }
        other => panic!("expected a context menu overlay, got {other:?}"),
    }
}

/// One case per target shape: a single row offers the leaf set with no
/// Shuffle (a book is a leaf, and a single episode is a leaf) plus the one
/// mark entry its cached progress state chooses (a missing entry counts as
/// unfinished); a multi-selection adds Shuffle and both mark entries.
/// `seed_finished` writes the cached progress entry for episode-a before the
/// menu opens.
#[rstest]
#[case::one_episode(
    vec![episode_target("episode-a")],
    false,
    &["Play", "Add to Queue", "Mark Played"],
)]
#[case::one_finished_episode(
    vec![episode_target("episode-a")],
    true,
    &["Play", "Add to Queue", "Mark Unplayed"],
)]
#[case::one_book(
    vec![AudiobookshelfMenuTarget::Book {
        library_item_id: "book-a".into(),
    }],
    false,
    &["Play", "Add to Queue", "Mark Played"],
)]
#[case::two_episodes(
    vec![episode_target("episode-a"), episode_target("episode-b")],
    false,
    &["Play", "Shuffle", "Add to Queue", "Mark Played", "Mark Unplayed"],
)]
fn audiobookshelf_menu_follows_the_standard_action_set_for(
    #[case] targets: Vec<AudiobookshelfMenuTarget>,
    #[case] seed_finished: bool,
    #[case] expected: &[&str],
) {
    let mut app = super::podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 0.0,
            is_finished: seed_finished,
        },
    );
    assert_eq!(menu_labels(&mut app, targets), expected);
}

/// `AudiobookshelfEnqueue` with two episode targets appends both resolved
/// episodes to the owner queue in the order the targets carry.
#[test]
fn audiobookshelf_enqueue_appends_both_targets_in_given_order() {
    let mut app = super::podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0]
        .detail_cache
        .get_mut("show-a")
        .unwrap()
        .push(mbv_audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(),
            title: "Episode B".into(),
            description: None,
            published_at: None,
            duration_seconds: None,
        });
    let cmd_rx = super::live_owner_channel(&mut app);

    app.execute_context_action(
        Some(ContextAction::AudiobookshelfEnqueue(vec![
            episode_target("episode-a"),
            episode_target("episode-b"),
        ])),
        None,
    );

    let appended: Vec<String> = cmd_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, .. } => match &*items {
                [
                    mbv_queue::QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
                        entry,
                    )),
                ] => Some(entry.episode_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        appended,
        vec!["episode-a".to_string(), "episode-b".to_string()],
        "the appends reach the owner in the carried target order"
    );
}
