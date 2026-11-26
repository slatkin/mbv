//! The `home` contracts of `keys.test.ts` (D16), over the shared harness: the
//! Home keys act on the row the pointer marks (`gui-pointer-target-interaction`
//! D2) — Enter plays it, and `n` and `a` enqueue it. The movement keys scroll
//! the Home list under the pointer, `/` and Esc run the Home filter's own
//! resolve, and the list keeps no cursor.

use super::*;
use mbv_queue::QueueItemContentId;

use crate::bridge::frame::library::{
    GuiHomeItemRowFrame, GuiHomeList, GuiHomeRowFrame, GuiHomeRowIdFrame, GuiHomeRowKindFrame,
    GuiHomeRowStateFrame,
};
use crate::window::library::home_list::home_row_key;

/// The `Continue watching` row the Home fixture carries.
fn continued_row() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Continue("e1".to_owned()),
        title: "Better Call Saul".to_owned(),
        subtitle: None,
        trailing: None,
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The `Recently added` row the pointer also rests on.
fn added_row() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed("f1".to_owned())),
        title: "A Feed Entry".to_owned(),
        subtitle: None,
        trailing: Some("12 May".to_owned()),
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The carried content while Home is active (design D12): no Music content,
/// the flat list's rules, and the Home rows.
fn home_content() -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        refreshing: false,
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: None,
        destination: GuiDestinationList::Home(GuiHomeList {
            loading: false,
            rows: vec![
                GuiHomeRowFrame::Heading {
                    text: "Continue watching".to_owned(),
                },
                GuiHomeRowFrame::Item(continued_row()),
                GuiHomeRowFrame::Spacer,
                GuiHomeRowFrame::Heading {
                    text: "Recently added".to_owned(),
                },
                GuiHomeRowFrame::Item(added_row()),
            ],
        }),
    };
    let detail = GuiLibraryDetail::Home(crate::bridge::frame::library::GuiHomeDetail {
        selected: Some(GuiHomeRowIdFrame::Continue("e1".to_owned())),
        hero: None,
    });
    (list, detail)
}

/// Carry the Home content, so the panel's presence fact holds and the row
/// model has its rows (design D2, D12).
fn carry_home(state: &LibraryState) {
    let (list, detail) = home_content();
    carry(state, list, detail);
}

/// The Home interaction state's facts, as the assertions read them.
fn home_facts(state: &LibraryState) -> String {
    let home = state.home_ui.peek().clone();
    format!("filter={:?}", home.filter)
}

/// The Home commands the core's row ids ride in.
fn recent_row() -> crate::gui_core::GuiHomeRowId {
    crate::gui_core::GuiHomeRowId::Recent(QueueItemContentId::Feed("f1".to_owned()))
}

/// The Home commands the core's row ids ride in.
fn continued_row_id() -> crate::gui_core::GuiHomeRowId {
    crate::gui_core::GuiHomeRowId::Continue("e1".to_owned())
}

/// The row key the `Continue watching` row paints.
fn continued_key() -> String {
    home_row_key(&continued_row().id)
}

/// The row key the `Recently added` row paints.
fn added_row_key() -> String {
    home_row_key(&added_row().id)
}

// Enter on Home plays the hovered row by its opaque id.
#[test]
fn enter_on_home_plays_the_hovered_row_by_its_opaque_id() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        route_on_row(
            state,
            facts,
            "enter",
            "Enter",
            PointerList::Home,
            &continued_key(),
        );
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::HomePlay {
            row: continued_row_id()
        }]
    );
}

// `a` on Home adds the hovered row to the queue.
#[test]
fn a_on_home_adds_the_hovered_row_to_the_queue() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        route_on_row(state, facts, "a", "a", PointerList::Home, &continued_key());
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::HomeQueue {
            row: continued_row_id()
        }]
    );
}

// Enter and `a` act on the hovered row, never on the core's selected one: the
// second section's row is the pointer's, not the selection's.
#[test]
fn enter_on_home_plays_the_hovered_row_not_the_selected_one() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        route_on_row(
            state,
            facts,
            "enter",
            "Enter",
            PointerList::Home,
            &added_row_key(),
        );
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::HomePlay { row: recent_row() }]
    );
}

// The movement keys scroll the Home list under the pointer (design D2, step 3,
// D6) and select nothing: the Hero keeps the core's selection.
#[test]
fn down_over_the_home_list_scrolls_the_list_and_selects_nothing() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        state.measure_home_list(60.0);
        route_on_row(
            state,
            facts,
            "down",
            "ArrowDown",
            PointerList::Home,
            &continued_key(),
        );
        let scroll = *state.scrolls.home.peek();
        facts
            .lock()
            .expect("facts")
            .push(format!("top={}", scroll.scroll_top));
    });
    assert_eq!(facts[0], "down=Handled");
    // The heading covers the list's top edge, so the step is its 28 px.
    assert_eq!(facts[1], "top=28");
    assert_eq!(commands(&rx), []);
}

// `/` opens the Home list's filter while the pointer is over the Library
// column (design D3, D7): the rail's keyboard path, and the Queue filter never
// sees the key.
#[test]
fn slash_opens_the_home_filter_and_sends_no_key() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);

        route(state, facts, "slash", "/", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(
        facts[1].contains("filter_open=true"),
        "the Library filter opened"
    );
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it, and
// the close runs the Home list's own resolve.
#[test]
fn esc_closes_the_home_filter_and_drops_its_text() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        let mut home_ui = state.home_ui;
        home_ui.with_mut(|ui| ui.filter = "row".to_owned());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts.lock().expect("facts").push(ui_facts(state));
        facts.lock().expect("facts").push(home_facts(state));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(
        facts[1].contains("filter_open=false"),
        "the Esc closed the filter"
    );
    assert_eq!(facts[2], "filter=\"\"");
    assert_eq!(commands(&rx), []);
}
