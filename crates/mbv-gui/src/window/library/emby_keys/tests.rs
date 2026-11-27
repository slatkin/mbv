//! Headless tests of the plain Emby-library keys (D16): the contracts of
//! `emby-keys.test.ts`, on the row the pointer marks
//! (`gui-pointer-target-interaction` D2) — the movement keys scroll the list
//! under the pointer, Enter and the double-click's play command play every row
//! (a collection row too), `a` queues, `/` is taken, and `n` goes to the core.
//! The router under test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Emby dispatch it hands to [`super::route_emby_panel_key`]. The harness builds
//! a `DioxusDocument` whose root creates the store and runs each scenario in one
//! effect over a recording core link, so no test opens a window or sleeps, and
//! no IPC runs.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiCollectionEntry, GuiCollections, GuiDestinationList, GuiEmbyDetail, GuiEmbyHeroFrame,
    GuiEmbyList, GuiEmbyRowFrame, GuiEmbyRowKindFrame, GuiHomeRowStateFrame, GuiLibraryDetail,
    GuiLibraryList, GuiLibraryTabFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::bridge::sections::GuiSectionBatch;
use crate::window::library::keys_shared::tests::{
    commands, route, route_row, run, scroll_facts, ui_facts,
};
use crate::window::library::lettered::EmbyList;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;

/// One plain-mode movie row.
fn movie(id: &str, title: &str, year: &str, sort_letter: char) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some(year.to_owned()),
        date: None,
        sort_letter: Some(sort_letter),
        kind: GuiEmbyRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
    }
}

/// One feed-group row.
fn feed_row(id: &str, title: &str, year: &str, kind: GuiEmbyRowKindFrame) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        date: Some(year.to_owned()),
        sort_letter: None,
        kind,
        state: GuiHomeRowStateFrame::Ordinary,
    }
}

/// The plain mode's name-sorted rows: two `A` movies and one `B` movie.
fn movies() -> Vec<GuiEmbyRowFrame> {
    vec![
        movie("m1", "Alien", "1979", 'A'),
        movie("m2", "Amélie", "2001", 'A'),
        movie("m3", "Brazil", "1985", 'B'),
    ]
}

/// The feed-group mode's date-ordered rows, with a folder as the first.
fn feed_rows() -> Vec<GuiEmbyRowFrame> {
    vec![
        feed_row("f1", "Family", "1998", GuiEmbyRowKindFrame::Collection),
        feed_row("f2", "Fargo", "1996", GuiEmbyRowKindFrame::Media),
    ]
}

/// The carried content while an Emby library is active (row 8.1): no Music,
/// Home, or Shows content, the mode's rows, and the Hero the frame paints.
fn emby_content(
    feed_group: bool,
    rows: &[GuiEmbyRowFrame],
    hero: Option<GuiEmbyHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        refreshing: false,
        tabs: vec![
            GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            },
            GuiLibraryTabFrame {
                position: 3,
                label: "Movies".to_owned(),
            },
        ],
        current_tab: Some(3),
        destination: GuiDestinationList::Emby(GuiEmbyList {
            view: crate::gui_core::GuiLibraryView::All,
            sort: None,
            lib_idx: 3,
            library_name: "Movies".to_owned(),
            collections: feed_group.then(|| GuiCollections {
                target: crate::gui_core::GuiCollectionTarget::EmbyFeed(3),
                all: true,
                entries: vec![GuiCollectionEntry {
                    key: "folder-1".to_owned(),
                    name: "Family".to_owned(),
                    checked: false,
                }],
                noun: crate::gui_core::GuiCollectionNoun::Folders,
            }),
            collection_names: if feed_group {
                vec!["All".to_owned()]
            } else {
                Vec::new()
            },
            feed_group,
            loading: false,
            failed: false,
            rows: rows.to_owned(),
            favourites: None,
        }),
    };
    let detail = GuiLibraryDetail::Emby(GuiEmbyDetail {
        selected: rows.first().map(|row| row.id.clone()),
        hero,
    });
    (list, detail)
}

/// Carry the Emby content and start the list's interaction state over.
fn carry_emby(state: &LibraryState, content: (GuiLibraryList, GuiLibraryDetail)) {
    let (list, detail) = content;
    let batch = GuiSectionBatch {
        library_list: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(list),
        }),
        library_detail: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
    let mut emby_ui = state.emby_ui;
    emby_ui.with_mut(|ui| {
        ui.filter.clear();
    });
}

// Enter on the hovered row plays it.
#[test]
fn enter_on_the_hovered_row_plays_it() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());

        route_row(state, facts, "enter", "Enter", PointerList::Emby, "m2");
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: "m2".to_owned(),
        }]
    );
}

// A folder or boxset row plays through the core's folder-play rule, so Enter
// on it sends the same play command as any other row (row 8.1).
#[test]
fn enter_on_a_collection_row_plays_it() {
    let content = emby_content(true, &feed_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());

        route_row(state, facts, "enter", "Enter", PointerList::Emby, "f1");
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: "f1".to_owned(),
        }]
    );
}

// `a` queues the hovered row, and `n` is not a Library key.
#[test]
fn a_queues_the_hovered_row_and_n_is_the_cores() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());

        route_row(state, facts, "n", "n", PointerList::Emby, "m1");
        route_row(state, facts, "a", "a", PointerList::Emby, "m1");
    });
    assert_eq!(facts, ["n=Core(None)", "a=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::EmbyQueue {
            lib_idx: 3,
            item_id: "m1".to_owned(),
        }]
    );
}

// Enter, `n`, and `a` with the pointer over no Emby row do nothing (design D2,
// step 3): the key is the window's, and it sends no command.
#[test]
fn enter_n_and_a_with_no_hovered_row_send_nothing() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());

        route(state, facts, "enter", "Enter", false);
        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["enter=Handled", "n=Core(None)", "a=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Down over the Emby list scrolls the list by the height of the row at the top
// edge (design D6) and selects nothing.
#[test]
fn down_over_the_emby_list_scrolls_it_by_one_row() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());
        state.measure_lettered_list::<EmbyList>(40.0);

        route_row(state, facts, "down", "ArrowDown", PointerList::Emby, "m1");
        facts
            .lock()
            .expect("facts")
            .push(scroll_facts(state.scrolls.emby));
    });
    assert_eq!(facts[0], "down=Handled");
    // The letter heading covers the list's top edge, so the step is its 24 px.
    assert_eq!(facts[1], "top=24 viewport=40");
    assert_eq!(commands(&rx), []);
}

// The movement keys are the core's while the pointer rests off the Emby list
// (design D2, step 4): nothing scrolls.
#[test]
fn down_off_the_emby_list_is_the_cores_key() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());
        state.measure_lettered_list::<EmbyList>(40.0);

        route(state, facts, "down", "ArrowDown", false);
        facts
            .lock()
            .expect("facts")
            .push(scroll_facts(state.scrolls.emby));
    });
    assert_eq!(facts[0], "down=Core(None)");
    assert_eq!(facts[1], "top=0 viewport=40");
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_emby_filter_and_sends_no_key() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());

        route(state, facts, "slash", "/", false);
        facts
            .lock()
            .expect("facts")
            .push(ui_facts(state, state.emby_ui));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Emby list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_emby_filter_and_drops_its_text() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone());
        let mut emby_ui = state.emby_ui;
        emby_ui.with_mut(|ui| ui.filter = "m1".to_owned());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts
            .lock()
            .expect("facts")
            .push(ui_facts(state, state.emby_ui));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(facts[1].contains("filter_open=false"));
    assert!(facts[1].contains("filter=\"\""));
    assert_eq!(commands(&rx), []);
}
