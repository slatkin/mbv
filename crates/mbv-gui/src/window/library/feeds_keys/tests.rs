//! Headless tests of the Feeds keys (D16): the contracts of
//! `feeds-keys.test.ts` on the row the pointer marks
//! (`gui-pointer-target-interaction` D2) — the movement keys scroll the list
//! under the pointer, Enter plays the hovered entry, `a` queues it, `n` is
//! consumed and does nothing, and Space stays the core's key. The router under
//! test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Feeds dispatch it hands to [`super::route_feeds_panel_key`]. The harness
//! builds a `DioxusDocument` whose root creates the store and runs each
//! scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiCollectionEntry, GuiCollections, GuiDestinationList, GuiEntryRowFrame,
    GuiFeedsDetail, GuiFeedsHeroFrame, GuiFeedsList, GuiHomeRowStateFrame, GuiLibraryDetail,
    GuiLibraryList, GuiLibraryTabFrame, GuiMetaKindFrame, GuiMetaSegmentFrame, GuiRowArtFrame,
    GuiRowMediaKindFrame,
};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::window::library::feeds::initial_feeds_ui;
use crate::window::library::keys_shared::tests::{
    commands, route, route_hero, route_row, run, scroll_facts, ui_facts,
};
use crate::window::library::lettered::FeedsList;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;

/// The entry rows the Latest pill merges, newest first.
fn entries() -> Vec<GuiEntryRowFrame> {
    vec![
        entry(
            "s1::e1",
            "Entry A",
            "Sub One",
            "12 May",
            GuiHomeRowStateFrame::Ordinary,
        ),
        entry(
            "s2::e2",
            "Entry B",
            "Sub Two",
            "9 May",
            GuiHomeRowStateFrame::Played,
        ),
    ]
}

/// One entry row, as the frame carries it.
fn entry(
    id: &str,
    title: &str,
    subtitle: &str,
    date: &str,
    state: GuiHomeRowStateFrame,
) -> GuiEntryRowFrame {
    GuiEntryRowFrame {
        sort_letter: None,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some(subtitle.to_owned()),
        date: Some(date.to_owned()),
        state,
    }
}

/// The All selection or the one-subscription selection, over the two
/// subscriptions (design D6).
fn collections(selected: PillSelection) -> GuiCollections {
    let entry = |key: &str, name: &str, checked| GuiCollectionEntry {
        key: key.to_owned(),
        name: name.to_owned(),
        checked,
    };
    GuiCollections {
        target: mbv::GuiCollectionTarget::Feeds,
        all: selected == PillSelection::Latest,
        entries: vec![
            entry(
                "https://sub.one",
                "Sub One",
                selected == PillSelection::Subscription,
            ),
            entry("https://sub.two", "Sub Two", false),
        ],
        noun: mbv::GuiCollectionNoun::Subscriptions,
    }
}

/// A Feeds Hero over the feed `parent`, with its entries in the list band.
fn feeds_hero(parent: &str) -> GuiFeedsHeroFrame {
    GuiFeedsHeroFrame {
        title: "Entry A".to_owned(),
        parent: Some(parent.to_owned()),
        meta_rows: vec![vec![GuiMetaSegmentFrame {
            text: "0 h 54 m".to_owned(),
            kind: GuiMetaKindFrame::Time,
        }]],
        duration_row: Some(0),
        progress_row: None,
        art_shape: GuiArtShapeFrame::Square,
    }
}

/// The collection selection the fixtures name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PillSelection {
    Latest,
    Subscription,
}

/// The carried content while the Feeds destination is active (design D12):
/// no other destination's content, the collection selector, and `hero` the
/// caller picks. The frame carries `selected` — the first row's, as the TS default
/// does.
fn feeds_content(
    pill_selection: PillSelection,
    rows: &[GuiEntryRowFrame],
    hero: Option<GuiFeedsHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    feeds_content_selected(
        rows.first().map(|row| row.id.as_str()),
        pill_selection,
        rows,
        hero,
    )
}

/// The carried content with `selected` as the core's selection, which the
/// Hero's Workspace never reads — the keys act on the hovered row.
fn feeds_content_selected(
    selected: Option<&str>,
    pill_selection: PillSelection,
    rows: &[GuiEntryRowFrame],
    hero: Option<GuiFeedsHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    let selected = selected.map(str::to_owned);
    let list = GuiLibraryList {
        tabs: vec![
            GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            },
            GuiLibraryTabFrame {
                position: 2,
                label: "Feeds".to_owned(),
            },
        ],
        current_tab: Some(2),
        destination: GuiDestinationList::Feeds(GuiFeedsList {
            sort: None,
            collections: collections(pill_selection),
            loading: false,
            failed: false,
            platform: None,
            rows: rows.to_owned(),
        }),
    };
    let detail = GuiLibraryDetail::Feeds(GuiFeedsDetail { selected, hero });
    (list, detail)
}

/// Carry the Feeds content and start the list's interaction state over.
fn carry_feeds(state: &LibraryState, content: (GuiLibraryList, GuiLibraryDetail)) {
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
    let mut feeds_ui = state.feeds_ui;
    feeds_ui.with_mut(|ui| *ui = initial_feeds_ui());
}

// Enter on the hovered row plays the entry.
#[test]
fn enter_on_the_hovered_row_plays_the_entry() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_row(state, facts, "enter", "Enter", PointerList::Feeds, "s2::e2");
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::FeedsPlay {
            item_id: "s2::e2".to_owned(),
        }]
    );
}

// `a` on the hovered row queues the entry.
#[test]
fn a_on_the_hovered_row_queues_the_entry() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_row(state, facts, "a", "a", PointerList::Feeds, "s1::e1");
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::FeedsQueue {
            item_id: "s1::e1".to_owned(),
        }]
    );
}

// A feed entry has no play-next (row 11.1): `n` is consumed and sends
// nothing, so it neither queues nor reaches the core's playback chords.
#[test]
fn n_on_the_hovered_row_sends_nothing_and_is_handled() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_row(state, facts, "n", "n", PointerList::Feeds, "s1::e1");
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Enter, `n`, and `a` with the pointer over no entry row do nothing (design
// D2, step 3): the key is the window's, and it sends no command.
#[test]
fn enter_n_and_a_with_no_hovered_row_send_nothing() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

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
fn down_over_the_feeds_list_scrolls_it_by_one_row() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());
        state.measure_lettered_list::<FeedsList>(40.0);

        route_row(
            state,
            facts,
            "down",
            "ArrowDown",
            PointerList::Feeds,
            "s1::e1",
        );
        facts
            .lock()
            .expect("facts")
            .push(scroll_facts(state.scrolls.feeds));
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
fn down_off_the_feeds_list_is_the_cores_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());
        state.measure_lettered_list::<FeedsList>(40.0);

        route(state, facts, "down", "ArrowDown", false);
        facts
            .lock()
            .expect("facts")
            .push(scroll_facts(state.scrolls.feeds));
    });
    assert_eq!(facts[0], "down=Core(None)");
    assert_eq!(facts[1], "top=0 viewport=40");
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_feeds_filter_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route(state, facts, "slash", "/", false);
        facts
            .lock()
            .expect("facts")
            .push(ui_facts(state, state.feeds_ui));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Feeds list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_feeds_filter_and_drops_its_text() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());
        let mut feeds_ui = state.feeds_ui;
        feeds_ui.with_mut(|ui| ui.filter = "s1".to_owned());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts
            .lock()
            .expect("facts")
            .push(ui_facts(state, state.feeds_ui));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(facts[1].contains("filter_open=false"));
    assert!(facts[1].contains("filter=\"\""));
    assert_eq!(commands(&rx), []);
}

// Enter on the hovered Workspace entry plays that entry (design D2): the Hero's
// list band carries the feed's entries, and their pointer marker is the entry's
// stable id.
#[test]
fn enter_on_the_hovered_workspace_entry_plays_it() {
    let content = feeds_content_selected(
        Some("s2::e2"),
        PillSelection::Latest,
        &entries(),
        Some(feeds_hero("Sub One")),
    );
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_hero(state, facts, "enter", "Enter", Some("s1::e1"));
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::FeedsPlay {
            item_id: "s1::e1".to_owned(),
        }]
    );
}

// `a` on the hovered Workspace entry queues that entry, not the core's
// selected one (design D2).
#[test]
fn a_on_the_hovered_workspace_entry_queues_it_not_the_selected_one() {
    let content = feeds_content_selected(
        Some("s2::e2"),
        PillSelection::Latest,
        &entries(),
        Some(feeds_hero("Sub One")),
    );
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_hero(state, facts, "a", "a", Some("s1::e1"));
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::FeedsQueue {
            item_id: "s1::e1".to_owned(),
        }]
    );
}

// Enter on a marker that names no entry the Hero paints sends nothing: another
// feed's entry, and no row at all.
#[test]
fn enter_on_no_painted_workspace_entry_sends_nothing() {
    let content = feeds_content(
        PillSelection::Latest,
        &entries(),
        Some(feeds_hero("Sub One")),
    );
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_hero(state, facts, "enter", "Enter", Some("s2::e2"));
        route_hero(state, facts, "enter", "Enter", None);
    });
    assert_eq!(facts, ["enter=Handled", "enter=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Feeds has no play-next: `n` on the Workspace is consumed and sends nothing.
#[test]
fn n_on_the_workspace_is_consumed_without_a_call() {
    let content = feeds_content(
        PillSelection::Latest,
        &entries(),
        Some(feeds_hero("Sub One")),
    );
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_hero(state, facts, "n", "n", Some("s1::e1"));
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// A movement key on the Workspace moves no row and sends no key: the Workspace
// keeps no cursor (design D6).
#[test]
fn a_movement_key_on_the_workspace_moves_no_row() {
    let content = feeds_content(
        PillSelection::Latest,
        &entries(),
        Some(feeds_hero("Sub One")),
    );
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone());

        route_hero(state, facts, "down", "ArrowDown", Some("s1::e1"));
        facts
            .lock()
            .expect("facts")
            .push(scroll_facts(state.scrolls.feeds));
    });
    assert_eq!(facts[0], "down=Handled");
    assert_eq!(facts[1], "top=0 viewport=0");
    assert_eq!(commands(&rx), []);
}
