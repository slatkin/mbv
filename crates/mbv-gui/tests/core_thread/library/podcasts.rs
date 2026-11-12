//! The Podcasts destination's contracts (add-gui-library-panel design D12, row
//! 10.1): the frame carries the pill row and the active pill's episode rows
//! once the destination opens, a pill command re-scopes the rows, a select
//! moves the Hero, and the play and queue commands reach the Owner through the
//! Audiobookshelf admission. Every identity is the frame's own opaque row id,
//! so a stale row changes nothing.

use mbv::ClientTickHost;
use mbv_ctrl::CtrlCmd;
use mbv_gui::bridge::frame::library::GuiPodcastsFrame;
use mbv_gui::bridge::library::GuiPodcastPill;
use mbv_queue::AudiobookshelfItem;
use mbv_ui_model::tab_selection::TabSelection;

use super::{library_of, send_library};

/// The seeded podcasts core over the recording stub Owner the queue commands'
/// tests read.
fn seeded_podcasts_core() -> (crate::TestCore, std::sync::mpsc::Receiver<CtrlCmd>) {
    let (app, cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = crate::test_core_with(app);
    test.core.app_mut().seed_gui_abs_libraries_for_test();
    (test, cmd_rx)
}

/// Open the Podcasts destination and return the tab position the open
/// command named (design D2, D12).
fn open_podcasts(test: &mut crate::TestCore) -> usize {
    let position = test
        .core
        .app_mut()
        .gui_library_tabs()
        .tabs
        .iter()
        .find(|tab| tab.label == "Podcasts")
        .map(|tab| tab.position)
        .expect("the presented tabs carry the Podcasts tab");
    send_library(
        test,
        mbv_gui::bridge::library::GuiLibraryCommand::OpenDestination { position },
    );
    position
}

/// Seed both shows' episodes as landed per-show fetches leave them. The tab
/// activation drops the committed pill's episode caches, so the seed follows
/// the open, exactly as the fan-out's own completions would.
fn seed_podcast_catalog(test: &mut crate::TestCore) {
    test.core
        .app_mut()
        .seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A"), ("show-b", "Show B")]);
    test.core.app_mut().seed_gui_podcast_episodes_for_test(
        1,
        "show-a",
        None,
        &[
            (
                "a-old",
                "A Old",
                Some("The A show's older episode."),
                Some(1_700_000_000),
                Some(600.0),
            ),
            ("a-undated", "A Undated", None, None, Some(600.0)),
        ],
    );
    test.core.app_mut().seed_gui_podcast_episodes_for_test(
        1,
        "show-b",
        None,
        &[(
            "b-new",
            "B New",
            Some("The B show's newest episode."),
            Some(1_800_000_000),
            Some(1200.0),
        )],
    );
}

/// The newest frame the core emitted, draining the older ones: the frames the
/// core emitted before the seed stay queued, so a test reads the frame its
/// last step built. The read takes no step of its own. A tick that lands a
/// fetch completion emits a frame as well, and the Library memo re-carries its
/// content only on a changed revision (design D2), so that frame carries no
/// section content and would shadow the frame under test.
fn newest_frame(test: &mut crate::TestCore) -> crate::Held {
    test.held()
}

/// Report a layout, as a window content reload does, so the Library section
/// re-carries the content it built from the seeded data: the reload re-arms
/// the memo, and the report's own step emits the frame that carries it.
fn reloaded_frame(test: &mut crate::TestCore) -> crate::Held {
    test.tx
        .send(crate::measured_layout(10, Vec::new()))
        .unwrap();
    assert!(test.core.step());
    newest_frame(test)
}

/// The Podcasts section a frame carries (row 10.1).
fn podcasts_of(frame: &crate::Held) -> GuiPodcastsFrame {
    library_of(frame)
        .and_then(|content| content.podcasts)
        .expect("the frame carries the Podcasts section")
}

/// The episode a queue command named, from the recording stub Owner's channel.
fn commanded_episode_id(cmd_rx: &std::sync::mpsc::Receiver<CtrlCmd>) -> String {
    cmd_rx
        .try_iter()
        .find_map(|command| match command {
            CtrlCmd::UnifiedQueueReplace { items, .. }
            | CtrlCmd::UnifiedQueueAppend { items, .. } => match &*items {
                [mbv_queue::QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(episode))] => {
                    Some(episode.episode_id.clone())
                }
                _ => None,
            },
            _ => None,
        })
        .expect("the Owner received the episode's queue command")
}

/// The frame carries the Podcasts list after the open (row 10.1): the section
/// names the podcast library, the episodes are in the TUI's date order with
/// the undated one last, the pill row is Latest first then each show by title,
/// the first row is the Hero default, and the Hero carries that episode's
/// facts, description, and Square art shape.
#[test]
fn the_frame_carries_the_podcast_list_after_the_open() {
    let (mut test, _cmd_rx) = seeded_podcasts_core();
    open_podcasts(&mut test);
    seed_podcast_catalog(&mut test);

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::AudiobookshelfLibrary(1),
        "the Podcasts destination is active"
    );
    let podcasts = podcasts_of(&reloaded_frame(&mut test));
    assert_eq!(podcasts.lib_idx, 1);
    let titles: Vec<&str> = podcasts.rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(
        titles,
        ["B New", "A Old", "A Undated"],
        "Latest merges both shows newest first with the undated episode last"
    );
    let names: Vec<&str> = podcasts
        .pills
        .iter()
        .map(|pill| pill.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Latest", "All", "Unplayed", "Played", "Show A", "Show B"],
        "the pill row carries the TUI's state pills between Latest and the shows"
    );
    assert_eq!(
        podcasts.selected.as_deref(),
        podcasts.rows.first().map(|row| row.id.as_str()),
        "the first row is the Hero default"
    );
    let hero = podcasts.hero.expect("the Hero paints");
    assert_eq!(hero.title, "B New");
    assert_eq!(hero.parent.as_deref(), Some("Show B"));
    assert_eq!(
        hero.overview.as_deref(),
        Some("The B show's newest episode.")
    );
    assert_eq!(
        hero.art_shape,
        mbv_gui::bridge::frame::library::GuiArtShapeFrame::Square
    );
}

/// A pill command re-scopes the rows (row 10.1): the set-pill command's show
/// leaves only that show's episodes, and its pill is the selected one.
#[test]
fn the_pill_command_changes_the_rows() {
    let (mut test, _cmd_rx) = seeded_podcasts_core();
    open_podcasts(&mut test);
    seed_podcast_catalog(&mut test);

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::PodcastsSetPill {
            lib_idx: 1,
            pill: GuiPodcastPill::Show("show-a".into()),
        },
    );

    let podcasts = podcasts_of(&newest_frame(&mut test));
    let titles: Vec<&str> = podcasts.rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, ["A Old", "A Undated"]);
    let selected: Vec<&str> = podcasts
        .pills
        .iter()
        .filter(|pill| pill.selected)
        .map(|pill| pill.name.as_str())
        .collect();
    assert_eq!(selected, ["Show A"]);
}

/// A select command moves the Hero (row 10.1): the second row's opaque id
/// becomes the frame's selection, and the Hero shows that episode.
#[test]
fn the_select_command_moves_the_hero() {
    let (mut test, _cmd_rx) = seeded_podcasts_core();
    open_podcasts(&mut test);
    seed_podcast_catalog(&mut test);
    let row = podcasts_of(&reloaded_frame(&mut test))
        .rows
        .get(1)
        .map(|row| row.id.clone())
        .expect("the list holds the second row");

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::PodcastsSelect {
            lib_idx: 1,
            row: row.clone(),
        },
    );

    let podcasts = podcasts_of(&newest_frame(&mut test));
    assert_eq!(podcasts.selected.as_deref(), Some(row.as_str()));
    let hero = podcasts.hero.expect("the Hero paints");
    assert_eq!(hero.title, "A Old");
}

/// The play and queue commands reach the Owner through the admission (row
/// 10.1): the play replaces the Owner's queue with the episode and the queue
/// command appends it.
#[test]
fn the_play_and_queue_commands_reach_the_owner() {
    let (mut test, cmd_rx) = seeded_podcasts_core();
    open_podcasts(&mut test);
    seed_podcast_catalog(&mut test);
    let row = podcasts_of(&reloaded_frame(&mut test))
        .rows
        .first()
        .map(|row| row.id.clone())
        .expect("the list holds rows");

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::PodcastsPlay {
            lib_idx: 1,
            item_id: row.clone(),
        },
    );
    assert_eq!(
        commanded_episode_id(&cmd_rx),
        "b-new",
        "the play replaced the Owner's queue with the episode"
    );

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::PodcastsQueue {
            lib_idx: 1,
            item_id: row,
        },
    );
    assert_eq!(
        commanded_episode_id(&cmd_rx),
        "b-new",
        "the queue command appended the episode"
    );
}

/// Stale Podcasts row ids change nothing (row 10.1): a play of a row id of no
/// valid shape submits nothing and flashes no toast.
#[test]
fn a_stale_podcast_row_submits_nothing() {
    let (mut test, cmd_rx) = seeded_podcasts_core();
    open_podcasts(&mut test);
    seed_podcast_catalog(&mut test);
    let _ = reloaded_frame(&mut test);

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::PodcastsPlay {
            lib_idx: 1,
            item_id: "not-a-row-id".into(),
        },
    );

    assert!(cmd_rx.try_recv().is_err(), "a stale id submits nothing");
    assert!(newest_frame(&mut test).status().toast.is_none());
}
