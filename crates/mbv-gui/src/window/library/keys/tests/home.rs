//! The `home` contracts of `keys.test.ts` (D16), over the shared harness: the
//! Home cursor owns the movement keys wherever the pane focus sits, Enter
//! plays the row it rests on, `n` and `a` enqueue from it, and `/` and Esc run
//! the Home filter's own resolve (design D12, row 6.2).

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
        date: None,
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: None,
    }
}

/// The `Recently added` row the movement steps to.
fn added_row() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed("f1".to_owned())),
        title: "A Feed Entry".to_owned(),
        subtitle: None,
        date: Some("12 May".to_owned()),
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: None,
    }
}

/// The carried content while Home is active (design D12): no Music content,
/// the flat list's rules, and the Home rows.
fn home_content() -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
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
    format!("cursor={:?} filter={:?}", home.cursor, home.filter)
}

/// The Home commands the core's row ids ride in.
fn recent_row() -> mbv::GuiHomeRowId {
    mbv::GuiHomeRowId::Recent(QueueItemContentId::Feed("f1".to_owned()))
}

/// The Home commands the core's row ids ride in.
fn continued_row_id() -> mbv::GuiHomeRowId {
    mbv::GuiHomeRowId::Continue("e1".to_owned())
}

// Enter on Home plays the cursor row by its opaque id.
#[test]
fn enter_on_home_plays_the_cursor_row_by_its_opaque_id() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        let mut home_ui = state.home_ui;
        home_ui.with_mut(|ui| ui.cursor = Some(home_row_key(&continued_row().id)));

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::HomePlay {
            row: continued_row_id()
        }]
    );
}

// `n` on Home sends play next for the cursor row.
#[test]
fn n_on_home_sends_play_next_for_the_cursor_row() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        let mut home_ui = state.home_ui;
        home_ui.with_mut(|ui| ui.cursor = Some(home_row_key(&continued_row().id)));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::HomePlayNext {
            row: continued_row_id()
        }]
    );
}

// `a` on Home adds the cursor row to the queue.
#[test]
fn a_on_home_adds_the_cursor_row_to_the_queue() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        let mut home_ui = state.home_ui;
        home_ui.with_mut(|ui| ui.cursor = Some(home_row_key(&continued_row().id)));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::HomeQueue {
            row: continued_row_id()
        }]
    );
}

// The cursor's settle selects (design D12): the Hero follows the cursor,
// named by the opaque id and never a position.
#[test]
fn down_on_home_moves_the_cursor_and_selects_the_row_it_settles_on() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        let mut home_ui = state.home_ui;
        home_ui.with_mut(|ui| ui.cursor = Some(home_row_key(&continued_row().id)));

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(home_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    assert_eq!(
        facts[1],
        format!(
            "cursor=Some(\"{}\") filter=\"\"",
            home_row_key(&added_row().id)
        )
    );
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::HomeSelect { row: recent_row() }]
    );
}

// `/` opens the Home list's filter under Library focus (design D3, D7):
// the rail's keyboard path, and the Queue filter never sees the key.
#[test]
fn slash_opens_the_home_filter_and_sends_no_key() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);

        route(state, facts, "slash", "/", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Home list's own resolve.
#[test]
fn esc_closes_the_home_filter_and_drops_its_text() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        let mut home_ui = state.home_ui;
        home_ui.with_mut(|ui| {
            ui.filter = "row".to_owned();
            ui.cursor = Some(home_row_key(&continued_row().id));
        });
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts.lock().expect("facts").push(ui_facts(state));
        facts.lock().expect("facts").push(home_facts(state));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(facts[1].contains("filter_open=false"));
    assert_eq!(facts[2], "cursor=Some(\"home-continue:e1\") filter=\"\"");
    assert_eq!(commands(&rx), []);
}
