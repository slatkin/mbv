//! The GUI Library's plain Emby-library contracts through the GUI tick
//! (add-gui-library-panel design D12, row 8.1): the mocked bootstrap's open
//! Movies destination finds the `emby` frame, a select command moves the
//! Hero, and an `embySetFeedGroup` pill switch loads the chosen group's items
//! into the next frame. The Emby answers come from the in-memory mock
//! transport, so no request leaves the process, and no live Owner, Service,
//! or window runs (design D13).

use crate::Held;
use mbv_client_core::ClientTickHost;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_gui::bridge::frame::library::{
    GuiDestinationList, GuiEmbyDetail, GuiEmbyList, GuiHeroPillFrame, GuiLibraryDetail,
    GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::gui_core::test_support::GuiStateSeams;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::feed::FeedHomeVideoSelection;

use super::{TestCore, detail_of, list_of, pump_until, pump_until_client_fetch, send_library};
use crate::{test_core_with, test_emby_client};

/// The library's one listing, as the mock transport returns it: the body the
/// root listing, the folder-count query, and the selected group's items
/// fetch all parse — the listing's folder is the one counted folder, and the
/// video is the item the switched group's rows carry.
const LIBRARY_BODY: &str = r#"{"Items":[{"Id":"folder-1","Name":"Channel 1","Type":"Folder"},{"Id":"vid-1","Name":"Clip","Type":"Video"}],"TotalRecordCount":2}"#;

/// A movie row of a root-list store's loaded list.
fn movie(id: &str, name: &str) -> EmbyItem {
    let mut item = mbv_emby_model::test_support::make_item(name, "Movie");
    item.id = id.to_string();
    item
}

/// The plain Emby-library list and detail parts a frame carries, present only
/// while a plain Emby-library destination presents.
fn emby_parts(frame: &Held) -> Option<(GuiEmbyList, GuiEmbyDetail)> {
    let GuiDestinationList::Emby(list) = list_of(frame)?.destination else {
        return None;
    };
    let GuiLibraryDetail::Emby(detail) = detail_of(frame)? else {
        return None;
    };
    Some((list, detail))
}

/// Pump GUI ticks until the plain Emby-library section a frame carries
/// satisfies `want`, waiting for the next library completion between steps
/// (the fetch workers' own progress) instead of sleeping between them.
fn pump_until_emby(
    test: &mut TestCore,
    want: impl Fn(&GuiEmbyList, &GuiEmbyDetail) -> bool,
    what: &str,
) {
    pump_until(
        test,
        |frame| emby_parts(frame).is_some_and(|(list, detail)| want(&list, &detail)),
        what,
    );
}

/// Pump until the Emby list shows its root-list fetch failure. That result is a
/// Client-only fetch, so this waits on the Client-only channel.
fn pump_until_emby_root_failure(test: &mut TestCore) {
    pump_until_client_fetch(
        test,
        |frame| emby_parts(frame).is_some_and(|(list, _)| list.failed),
        "the failed root fetch",
    );
}

/// The mocked bootstrap's catalog presents the Movies destination, and the
/// restore's open finds the `emby` frame (row 8.1, design D12): the plain
/// mode's section names the library and carries no pill, and the root-list
/// fetch fails against the no-URL stub client; a select command over the
/// seeded store then moves the Hero to that row.
#[test]
fn the_bootstrap_movies_destination_finds_the_emby_frame_and_a_select_moves_the_hero() {
    let view = EmbyItem::folder(
        "lib-movies".to_string(),
        "Movies".to_string(),
        "movies".to_string(),
    );
    let mut test = test_core_with(mbv_client_core::stub_app_with_completed_emby_startup(vec![
        view,
    ]));

    // The mocked bootstrap drains on the first tick; the catalog's one
    // presented destination is the Movies library, and the start stays on
    // Home until the user opens it.
    assert!(test.core.step());
    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 1 },
    );
    let frame = test.held();
    let (list, detail) =
        emby_parts(&frame).expect("the open Movies destination presents its content");
    assert_eq!(
        list.lib_idx, 0,
        "the frame names the library the commands carry"
    );
    assert!(!list.feed_group, "the Movies library is the plain mode");
    assert!(
        list.collections.is_none() && list.rows.is_empty() && detail.hero.is_none(),
        "the root list has not landed rows yet"
    );
    // The no-URL stub client's root-list fetch fails; the frame names the
    // failure once it lands, and a later frame build re-issues nothing
    // behind the seed below.
    pump_until_emby_root_failure(&mut test);

    test.core
        .app_mut()
        .seed_gui_emby_roots_for_test(0, vec![movie("m-1", "A Movie"), movie("m-2", "B Movie")]);
    send_library(
        &mut test,
        GuiLibraryCommand::EmbySelect {
            lib_idx: 0,
            row: "m-2".into(),
        },
    );
    let frame = test.held();
    let (_, detail) = emby_parts(&frame).expect("the frame carries the Emby content");
    assert_eq!(
        detail.selected.as_deref(),
        Some("m-2"),
        "the frame paints the core's selection"
    );
    assert_eq!(
        detail.hero.as_ref().expect("the Hero moved").title,
        "B Movie",
        "the Hero's facts follow the selected row"
    );
}

/// An `embySetFeedGroup` pill switch loads the checked folder's items into the
/// next frame (row 8.1, the task's own contract; gui-collection-dropdown
/// design D2, D3): the press checks the folder in the GUI's own set, whose
/// fetch the mock answers, and the frame path's projection carries the folder
/// pill as a checked one with the folder's items as the rows — while the TUI's
/// shared selection stays where the TUI left it.
#[test]
fn an_emby_set_feed_group_command_loads_the_groups_items_into_the_next_frame() {
    let http = MockHttp::new();
    http.respond_repeated(200, LIBRARY_BODY, 12);
    let (app, _cmd_rx) = mbv_client_core::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    // The All selection arrives cached (empty), so the folder fetch the
    // switched group starts is the one items fetch the mock answers.
    test.core
        .app_mut()
        .seed_gui_feed_view_library_for_test(test_emby_client(&http));

    // The restore's open re-activates the position, whose root listing and
    // count fan-out land through the tick's drains.
    pump_until_emby(
        &mut test,
        |list, _| {
            list.collections
                .as_ref()
                .is_some_and(|collections| collections.entries.len() == 1)
                && !list.loading
        },
        "the folder entry",
    );

    send_library(
        &mut test,
        GuiLibraryCommand::CollectionToggle {
            target: mbv_gui::gui_core::GuiCollectionTarget::EmbyFeed(0),
            key: "folder-1".into(),
        },
    );
    pump_until_emby(
        &mut test,
        |list, _| {
            list.rows.iter().any(|row| row.id == "vid-1")
                && list.collections.as_ref().is_some_and(|collections| {
                    collections
                        .entries
                        .iter()
                        .any(|entry| entry.key == "folder-1" && entry.checked)
                })
        },
        "the switched group's items",
    );
    let app = test.core.app_mut();
    let state = app.libs_for_test()[0]
        .feed_home_video
        .as_ref()
        .expect("the feed state");
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::All,
        "the TUI's own selection stays where the TUI left it"
    );
    assert!(
        state
            .items
            .get(&FeedHomeVideoSelection::Folder("folder-1".into()))
            .is_some_and(|items| items.iter().any(|item| item.id == "vid-1")),
        "the checked folder's fetch filled the shared cache"
    );
}

/// Opens the mocked Movies destination with `items` as its root list and
/// selects the first one, so the next frame carries that item's Hero.
fn movies_hero_frame(items: Vec<EmbyItem>) -> Held {
    let view = EmbyItem::folder(
        "lib-movies".to_string(),
        "Movies".to_string(),
        "movies".to_string(),
    );
    let mut test = test_core_with(mbv_client_core::stub_app_with_completed_emby_startup(vec![
        view,
    ]));
    assert!(test.core.step());
    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 1 },
    );
    pump_until_emby_root_failure(&mut test);
    test.core.app_mut().seed_gui_emby_roots_for_test(0, items);
    send_library(
        &mut test,
        GuiLibraryCommand::EmbySelect {
            lib_idx: 0,
            row: "m-1".into(),
        },
    );
    test.held()
}

/// An Emby Hero's duration fact reaches the frame as a `time`-typed pill, so
/// the window can paint it in the time colour (design D14;
/// gui-hero-design-polish design D1).
#[test]
fn an_emby_heros_duration_fact_arrives_as_a_time_pill() {
    let mut item = movie("m-1", "A Movie");
    item.runtime_ticks = 5_400 * TICKS_PER_SECOND;
    let frame = movies_hero_frame(vec![item]);
    let hero = emby_parts(&frame)
        .and_then(|(_, detail)| detail.hero)
        .expect("the Hero paints");
    assert_eq!(
        hero.facts.pills,
        vec![GuiHeroPillFrame {
            segments: vec![GuiMetaSegmentFrame {
                text: "01:30:00".into(),
                kind: GuiMetaKindFrame::Time,
            }],
            link: None,
        }],
        "the runtime is the one pill, typed as time"
    );
}
