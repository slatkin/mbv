//! Headless tests of the Podcasts keys (D16): the contracts of
//! `podcasts-keys.test.ts` on the row the pointer marks
//! (`gui-pointer-target-interaction` D2) — the movement keys scroll the list
//! under the pointer, Enter plays the hovered episode, `a` queues it, `n` is
//! consumed and does nothing, and Space stays the core's key. The router under
//! test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Podcasts dispatch it hands to [`super::route_podcasts_panel_key`]. The
//! harness builds a `DioxusDocument` whose root creates the store and runs
//! each scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiCollectionEntry, GuiCollections, GuiDestinationList, GuiEntryRowFrame, GuiHomeRowStateFrame,
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiPodcastsDetail, GuiPodcastsHeroFrame,
    GuiPodcastsList, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::window::library::keys_shared::tests::{
    commands, route, route_row, run, scroll_facts, ui_facts,
};
use crate::window::library::lettered::PodcastsList;
use crate::window::library::podcasts::initial_podcasts_ui;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;

/// The episode rows the Latest pill merges, newest first.
fn episodes() -> Vec<GuiEntryRowFrame> {
    vec![
        episode(
            "e1:ep1",
            "Episode A",
            "Show One",
            "12 May",
            GuiHomeRowStateFrame::Ordinary,
        ),
        episode(
            "e2:ep2",
            "Episode B",
            "Show Two",
            "9 May",
            GuiHomeRowStateFrame::Played,
        ),
    ]
}

/// One episode row, as the frame carries it.
fn episode(
    id: &str,
    title: &str,
    subtitle: &str,
    date: &str,
    state: GuiHomeRowStateFrame,
) -> GuiEntryRowFrame {
    GuiEntryRowFrame {
        sort_letter: None,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some(subtitle.to_owned()),
        date: Some(date.to_owned()),
        state,
    }
}

/// The All selection or the one-show selection, over the two shows (design
/// D6).
fn collections(selected: PillSelection) -> GuiCollections {
    let entry = |key: &str, name: &str, checked| GuiCollectionEntry {
        key: key.to_owned(),
        name: name.to_owned(),
        checked,
    };
    GuiCollections {
        target: crate::gui_core::GuiCollectionTarget::Podcasts(0),
        all: selected == PillSelection::Latest,
        entries: vec![
            entry("sh1", "Show One", selected == PillSelection::Show),
            entry("sh2", "Show Two", false),
        ],
        noun: crate::gui_core::GuiCollectionNoun::Shows,
    }
}

/// The collection selection the fixtures name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PillSelection {
    Latest,
    Show,
}

/// The carried content while Podcasts is active (design D12): no other
/// destination's content, the collection selector, and `hero` the caller
/// picks. The frame carries `selected` — the first row's, as the TS default
/// does.
fn podcasts_content(
    pill_selection: PillSelection,
    rows: &[GuiEntryRowFrame],
    hero: Option<GuiPodcastsHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    let selected = rows.first().map(|row| row.id.clone());
    let list = GuiLibraryList {
        tabs: vec![
            GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            },
            GuiLibraryTabFrame {
                position: 2,
                label: "Podcasts".to_owned(),
            },
        ],
        current_tab: Some(2),
        destination: GuiDestinationList::Podcasts(GuiPodcastsList {
            sort: None,
            lib_idx: 0,
            collections: collections(pill_selection),
            loading: false,
            failed: false,
            rows: rows.to_owned(),
        }),
    };
    let detail = GuiLibraryDetail::Podcasts(GuiPodcastsDetail { selected, hero });
    (list, detail)
}

/// Carry the Podcasts content and start the list's interaction state over.
fn carry_podcasts(state: &LibraryState, content: (GuiLibraryList, GuiLibraryDetail)) {
    let (list, detail) = content;
    let batch = GuiSectionBatch {
        library_list: Some(GuiSection {
            revision: 1,
            value: Some(list),
        }),
        library_detail: Some(GuiSection {
            revision: 1,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
    let mut ui = state.ui;
    ui.with_mut(|ui| {
        ui.hero_dismissed = true;
        ui.item_menu_open = true;
    });
    let mut podcasts_ui = state.podcasts_ui;
    podcasts_ui.with_mut(|ui| *ui = initial_podcasts_ui());
}

// Enter on the hovered row plays the episode.
#[test]
fn enter_on_the_hovered_row_plays_the_episode() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route_row(
            state,
            facts,
            "enter",
            "Enter",
            PointerList::Podcasts,
            "e2:ep2",
        );
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::PodcastsPlay {
            lib_idx: 0,
            item_id: "e2:ep2".to_owned(),
        }]
    );
}

// `a` on the hovered row queues the episode.
#[test]
fn a_on_the_hovered_row_queues_the_episode() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route_row(state, facts, "a", "a", PointerList::Podcasts, "e1:ep1");
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::PodcastsQueue {
            lib_idx: 0,
            item_id: "e1:ep1".to_owned(),
        }]
    );
}

// A podcast episode has no play-next (row 10.1): `n` is consumed and sends
// nothing, so it neither queues nor reaches the core's playback chords.
#[test]
fn n_on_the_hovered_row_sends_nothing_and_is_handled() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route_row(state, facts, "n", "n", PointerList::Podcasts, "e1:ep1");
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Enter, `n`, and `a` with the pointer over no episode row do nothing (design
// D2, step 3): the key is the window's, and it sends no command.
#[test]
fn enter_n_and_a_with_no_hovered_row_send_nothing() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route(state, facts, "enter", "Enter", false);
        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["enter=Handled", "n=Handled", "a=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Down over the list scrolls it by the height of the row at the top edge
// (design D6) and plays nothing.
#[test]
fn down_over_the_podcasts_list_scrolls_it_by_one_row() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());
        state.measure_lettered_list::<PodcastsList>(40.0);

        route_row(
            state,
            facts,
            "down",
            "ArrowDown",
            PointerList::Podcasts,
            "e1:ep1",
        );
        facts
            .lock()
            .expect("facts")
            .push(scroll_facts(state.scrolls.podcasts));
    });
    assert_eq!(facts[0], "down=Handled");
    // The first row is 56 px, so one row step moves the 40 px viewport down
    // a row.
    assert_eq!(facts[1], "top=56 viewport=40");
    assert_eq!(commands(&rx), []);
}

// The movement keys are the core's while the pointer rests off the list
// (design D2, step 4): nothing scrolls.
#[test]
fn down_off_the_podcasts_list_is_the_cores_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());
        state.measure_lettered_list::<PodcastsList>(40.0);

        route(state, facts, "down", "ArrowDown", false);
        facts
            .lock()
            .expect("facts")
            .push(scroll_facts(state.scrolls.podcasts));
    });
    assert_eq!(facts[0], "down=Core(None)");
    assert_eq!(facts[1], "top=0 viewport=40");
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_podcasts_filter_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route(state, facts, "slash", "/", false);
        facts
            .lock()
            .expect("facts")
            .push(ui_facts(state, state.podcasts_ui));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Podcasts list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_podcasts_filter_and_drops_its_text() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());
        let mut podcasts_ui = state.podcasts_ui;
        podcasts_ui.with_mut(|ui| ui.filter = "e1".to_owned());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts
            .lock()
            .expect("facts")
            .push(ui_facts(state, state.podcasts_ui));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(facts[1].contains("filter_open=false"));
    assert!(facts[1].contains("filter=\"\""));
    assert_eq!(commands(&rx), []);
}
