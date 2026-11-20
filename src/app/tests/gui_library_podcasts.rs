//! Tests of the GUI Library's Podcasts data path (add-gui-library-panel
//! design D12, row 10.1): an open of an unloaded show catalog starts the
//! TUI's own show-list chain and the fetched page lands on the App through
//! the GUI tick's drain, All lands every listed show's episodes through the
//! TUI's bounded fan-out, a checked show lands that show's episodes only, two
//! checked shows merge with no committed scope, an episode select stores the
//! pick, the presented destination set names the podcast libraries and
//! restores a saved podcast location, and the All shelf's shared one-shot
//! guard requests once. The Audiobookshelf boundary is the lib event channel,
//! so no request leaves the process beyond a mocked loopback port's refusal
//! and no worker outlives the assertion. The data path's tests live in the
//! `data_path` child module, so this file stays within its line ceiling.

use super::*;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_ui_model::media_list::{ActiveProgress, MediaSemanticState};
mod data_path;

/// Press one show in the Podcasts destination's collection dropdown.
fn toggle_show(app: &mut App, lib_idx: usize, show_id: &str) {
    app.apply_gui_library_command(GuiLibraryCommand::CollectionToggle {
        target: GuiCollectionTarget::Podcasts(lib_idx),
        key: show_id.to_string(),
    });
}

/// Press the All row of the Podcasts destination's collection dropdown.
fn select_all_shows(app: &mut App, lib_idx: usize) {
    app.apply_gui_library_command(GuiLibraryCommand::CollectionAll {
        target: GuiCollectionTarget::Podcasts(lib_idx),
    });
}

/// The names of the Podcasts dropdown's checked shows.
fn checked_names(projection: &GuiPodcastsProjection) -> Vec<&str> {
    let collections = &projection.collections;
    let entries = collections.entries.iter();
    entries
        .filter(|entry| entry.checked)
        .map(|entry| entry.name.as_str())
        .collect()
}

/// A stub `App` whose Audiobookshelf catalog holds one book library and one
/// podcast library, with the podcast destination active and a resolvable
/// Audiobookshelf setup over a closed loopback port: the state a settled
/// Audiobookshelf startup leaves before its show pages land, with the
/// loaders' workers sending their failure class instead of hanging. The
/// caller holds the test state dir, so the secret file and the workers'
/// connection attempts stay inside the scratch path.
fn podcast_app() -> App {
    let mut app = make_app_stub();
    app.seed_gui_abs_libraries_for_test();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(mbv_config::AudiobookshelfSetup::new("http://127.0.0.1:1"));
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "test-token")
        .expect("the secret is written under the test state dir");
    app.tab = TabSelection::AudiobookshelfLibrary(1);
    app
}

/// The podcast browse state the seeded catalog holds for library 1.
fn podcast_state(app: &App) -> &AudiobookshelfBrowseState {
    &app.audiobookshelf_browse[1]
}

/// A stub `App` whose podcast library 1 holds `shows` in the list's title
/// order, with each show's episodes cached through [`episode_cache`] — the
/// data a landed fan-out leaves.
fn projection_app() -> App {
    let mut app = podcast_app();
    app.seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A"), ("show-b", "Show B")]);
    app.seed_gui_podcast_episodes_for_test(
        1,
        "show-a",
        Some("show-a-cover"),
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
    app.seed_gui_podcast_episodes_for_test(
        1,
        "show-b",
        Some("show-b-cover"),
        &[(
            "b-new",
            "B New",
            Some("The B show's newest episode."),
            Some(1_800_000_000),
            Some(1200.0),
        )],
    );
    app
}

/// All merges every listed show's episodes in the date order (row 10.1,
/// contract a): newest first with the undated episode last, the pill row All
/// plus one pill per show by title with All checked, the first row as the
/// selected episode, and the Hero carrying that episode's facts, its
/// description, and the parent show's Square cover.
#[test]
fn all_merges_two_shows_in_date_order_with_the_default_hero() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();

    let projection = app.gui_podcasts_projection(1);

    let titles: Vec<&str> = projection
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["B New", "A Old", "A Undated"],
        "Latest merges both shows newest first with the undated episode last"
    );
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("Show B"),
        "the merged row names its own show"
    );
    assert_eq!(
        projection.rows[0].date.as_deref(),
        Some(mbv_ui_model::ui_util::fmt_publish_date_short(1_800_000_000).as_str()),
        "the gutter carries the publish date"
    );
    assert_eq!(
        projection.rows[2].date, None,
        "an undated episode carries no gutter"
    );

    let collections = &projection.collections;
    let rows: Vec<(&str, bool)> = collections
        .entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.checked))
        .collect();
    assert_eq!(
        rows,
        [("Show A", false), ("Show B", false)],
        "the dropdown offers every show, all unchecked under All"
    );

    let first = GuiPodcastEpisodeId::new("show-b".into(), "b-new".into()).encode();
    assert_eq!(
        projection.selected.as_deref(),
        Some(first.as_str()),
        "the first row is the selected episode"
    );

    let hero = projection.hero.expect("the Hero paints");
    assert_eq!(hero.title, "B New");
    assert_eq!(
        hero.parent.as_deref(),
        Some("Show B"),
        "the show is the largest unit, so it rides as the parent and not as a meta row"
    );
    assert_eq!(hero.meta_rows.len(), 2, "the duration, the date");
    assert_eq!(hero.duration_row, Some(0));
    assert_eq!(
        hero.overview.as_deref(),
        Some("The B show's newest episode."),
        "the episode's description rides the overview"
    );
    assert_eq!(
        hero.art_shape,
        GuiArtShape::Square,
        "podcast artwork is Square"
    );
    assert!(
        hero.art.is_some(),
        "the parent show's cover resolves through the Service-scoped chain"
    );
}

/// A checked show scopes the list to that show (row 10.1, contract a): the
/// show's pill is the one checked and only that show's episodes ride the
/// rows.
#[test]
fn a_checked_show_scopes_the_rows_to_that_show() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();

    toggle_show(&mut app, 1, "show-a");
    let projection = app.gui_podcasts_projection(1);

    let titles: Vec<&str> = projection
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["A Old", "A Undated"],
        "a checked show holds that show's episodes only"
    );
    assert_eq!(
        checked_names(&projection),
        ["Show A"],
        "the checked show's row is the one checked"
    );
}

/// Two checked shows merge their episodes (gui-collection-dropdown design
/// D2): both pills stay checked, and the merged list keeps the date order
/// with the undated episode last.
#[test]
fn two_checked_shows_merge_their_episodes_in_date_order() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();

    toggle_show(&mut app, 1, "show-a");
    toggle_show(&mut app, 1, "show-b");
    let projection = app.gui_podcasts_projection(1);

    let titles: Vec<&str> = projection
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["B New", "A Old", "A Undated"],
        "the two checked shows' episodes merge in the date order"
    );
    assert_eq!(
        checked_names(&projection),
        ["Show A", "Show B"],
        "both shows stay checked"
    );
    assert_eq!(
        podcast_state(&app).committed_show_pill,
        None,
        "several checked shows commit the every-show scope, the same loads as All"
    );
}

/// Add a second podcast library at index 2, so a stale frame's collection
/// command can name a podcast library the tab is not on.
fn add_second_podcast_library(app: &mut App) {
    let library = mbv_audiobookshelf::AudiobookshelfLibrary {
        id: "lib-abs-podcasts-b".into(),
        name: "Podcasts B".into(),
        media_type: "podcast".into(),
    };
    app.audiobookshelf_libraries.push(library.clone());
    app.audiobookshelf_browse
        .push(AudiobookshelfBrowseState::new(library));
}

/// A collection command that names a podcast library other than the active
/// destination starts no load (review of gui-one-destination-ensure units 2.4
/// and 2.5): the shared collection path installs the set for the library it
/// names, as it always did, and the driver's active-tab guard keeps the
/// destination's load pass inert.
#[test]
fn a_collection_command_on_another_podcast_library_starts_no_load() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    add_second_podcast_library(&mut app);

    toggle_show(&mut app, 2, "show-a");

    let stale = &app.audiobookshelf_browse[2];
    assert_eq!(
        stale.catalog_request, 0,
        "the stale library started no catalog chain"
    );
    assert_eq!(
        stale.committed_show_pill, None,
        "the stale library's scope was not committed"
    );
    assert!(
        stale.detail_loading_ids.is_empty(),
        "no per-show fetch started"
    );
    assert!(
        !app.audiobookshelf_shelf_requested
            .contains("lib-abs-podcasts-b"),
        "no shelf fetch started"
    );
}

/// The Oldest sort puts the earliest episode first, with the undated
/// episodes last (gui-flat-list-sort design D5, D6): the stored sort sorts
/// the merged list without a refetch.
#[test]
fn the_oldest_sort_puts_the_earliest_episode_first_with_undated_last() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();
    app.set_gui_list_sort(GuiSortTarget::Podcasts(1), GuiListSort::Oldest);

    let projection = app.gui_podcasts_projection(1);

    let titles: Vec<&str> = projection
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["A Old", "B New", "A Undated"],
        "Oldest sorts the earliest episode first and keeps the undated one last"
    );
}

/// A checked show the published show list no longer carries drops from the
/// set (row 10.1; gui-collection-dropdown design D1's vanish rule): the
/// projection paints All's merged rows and marks All checked.
#[test]
fn a_vanished_checked_show_falls_back_to_all() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();
    toggle_show(&mut app, 1, "show-a");
    app.audiobookshelf_browse[1]
        .shows
        .retain(|show| show.library_item_id != "show-a");

    let projection = app.gui_podcasts_projection(1);

    let titles: Vec<&str> = projection
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["B New"],
        "the fallback view merges every listed show"
    );
    assert_eq!(
        checked_names(&projection),
        Vec::<&str>::new(),
        "All checks no show"
    );
}

/// The rows carry the shared row rule (row 10.1, spec `playback-resume`):
/// the Audiobookshelf constructor's states — a finished episode is `Played`
/// whatever its position, an unfinished one that Play would resume is
/// `Active` with its percent, and every other episode is `Ordinary`.
#[test]
fn the_rows_carry_the_played_and_resume_state() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();
    app.seed_gui_podcast_progress_for_test(1, "show-a", "a-old", 300.0, false);
    app.seed_gui_podcast_progress_for_test(1, "show-a", "a-undated", 600.0, true);

    let projection = app.gui_podcasts_projection(1);

    let states: Vec<MediaSemanticState> = projection
        .rows
        .iter()
        .map(|row| row.state.clone())
        .collect();
    assert_eq!(
        states,
        [
            MediaSemanticState::Ordinary,
            MediaSemanticState::Active {
                progress: Some(ActiveProgress::new(50))
            },
            MediaSemanticState::Played,
        ]
    );
}

/// The episode row id round-trips through its one encoding (row 10.1): the
/// length prefix keeps the pair unambiguous even when the ids hold the
/// separator, and a string of another shape decodes to nothing.
#[test]
fn the_episode_row_id_round_trips_through_its_encoding() {
    let episode = GuiPodcastEpisodeId::new("show:1".into(), "ep:2".into());
    let encoded = episode.encode();

    assert_eq!(GuiPodcastEpisodeId::decode(&encoded), Some(episode));
    assert_eq!(GuiPodcastEpisodeId::decode("no-separator"), None);
    assert_eq!(GuiPodcastEpisodeId::decode("99:too-short"), None);
    assert_eq!(
        GuiPodcastEpisodeId::decode("0:empty-show"),
        None,
        "an empty show id names no episode"
    );
}

/// The play and queue commands reach the Owner under the admission (row
/// 10.1): the play replaces the queue with the episode, and the queue command
/// appends it.
#[test]
fn the_play_and_queue_commands_replace_and_append_under_the_admission() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();
    let cmd_rx = super::live_owner_channel(&mut app);
    let row = GuiPodcastEpisodeId::new("show-a".into(), "a-old".into()).encode();

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsPlay {
        lib_idx: 1,
        item_id: row.clone(),
    });
    let replaced = cmd_rx
        .try_iter()
        .find_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueReplace { items, .. } => match &*items {
                [
                    mbv_queue::QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
                        episode,
                    )),
                ] => Some((episode.library_item_id.clone(), episode.episode_id.clone())),
                _ => None,
            },
            _ => None,
        })
        .expect("the Owner received a queue replacement");
    assert_eq!(replaced, ("show-a".to_string(), "a-old".to_string()));

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsQueue {
        lib_idx: 1,
        item_id: row,
    });
    let appended: Vec<String> = cmd_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, .. } => match &*items {
                [
                    mbv_queue::QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
                        episode,
                    )),
                ] => Some(episode.episode_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(appended, vec!["a-old".to_string()]);
}

/// A denied Owner flashes the TUI's owner-unavailable toast and submits
/// nothing (row 10.1): both the play and the queue command gate on the
/// Audiobookshelf admission.
#[test]
fn a_denied_podcast_owner_flashes_the_owner_unavailable_toast() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();
    let cmd_rx = super::live_owner_channel(&mut app);
    app.set_abs_admission_for_test(false);
    let row = GuiPodcastEpisodeId::new("show-a".into(), "a-old".into()).encode();

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsPlay {
        lib_idx: 1,
        item_id: row.clone(),
    });
    assert_eq!(
        app.newest_toast_text(),
        "Audiobookshelf playback owner is unavailable",
        "the play's refusal flashes the owner-unavailable toast"
    );
    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsQueue {
        lib_idx: 1,
        item_id: row,
    });
    assert!(
        cmd_rx.try_recv().is_err(),
        "a denied Owner receives no queue change"
    );
}

/// Stale Podcasts ids change nothing (row 10.1): a row id of no valid shape, a
/// play of an episode the cache does not hold, and an enqueue of one all
/// submit nothing and flash no toast, and a select of a stale row leaves the
/// pick as it was.
#[test]
fn stale_podcast_ids_change_nothing() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();
    let cmd_rx = super::live_owner_channel(&mut app);
    let picked = GuiPodcastEpisodeId::new("show-a".into(), "a-old".into()).encode();
    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsSelect {
        lib_idx: 1,
        row: picked.clone(),
    });
    let gone = GuiPodcastEpisodeId::new("show-a".into(), "gone".into()).encode();

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsSelect {
        lib_idx: 1,
        row: "not-an-id".into(),
    });
    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsPlay {
        lib_idx: 1,
        item_id: gone.clone(),
    });
    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::PodcastsQueue {
        lib_idx: 1,
        item_id: gone,
    });

    assert!(cmd_rx.try_recv().is_err(), "a stale id submits nothing");
    assert!(
        app.newest_toast_text().is_empty(),
        "a stale id flashes no toast"
    );
    assert_eq!(
        app.gui_podcast_picked_episode(1)
            .map(GuiPodcastEpisodeId::from)
            .map(|episode| episode.encode())
            .as_deref(),
        Some(picked.as_str()),
        "a stale select leaves the pick as it was"
    );
}

/// A podcast row shows its show's cover (gui-delineated-rows design D4, D5):
/// the episode artwork policy names the parent show's cover, under the
/// Service-scoped row key holding the show's last update time, and the row
/// paints the audio media kind.
#[test]
fn a_podcast_row_shows_its_show_cover() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app();
    let server = app
        .audiobookshelf_server_url()
        .expect("the app names its Audiobookshelf server");
    let key = mbv_images::abs_row_cache_key(&server, "show-b", 0);
    let cover = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        72,
        72,
        image::Rgba([20, 40, 60, 255]),
    ));
    app.seed_card_image_for_test(key.clone(), cover);

    let projection = app.gui_podcasts_projection(1);

    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("Show B"),
        "the newest row belongs to the show whose cover it shows"
    );
    assert_eq!(
        projection.rows[0].art,
        GuiRowArt::Image {
            key: key.clone(),
            square: true
        },
        "the episode row shows the cached show cover"
    );
    assert_eq!(projection.rows[0].media, GuiRowMediaKind::Audio);
}
