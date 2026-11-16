//! The `list` contracts of `keys.test.ts` (D16), over the shared harness: the
//! Music keys act on the row the pointer marks (`gui-pointer-target-interaction`
//! D2) — Enter plays it, `n` and `a` enqueue it, and the movement keys scroll
//! the list under the pointer. The list keeps no cursor, so no key moves one.

use super::*;

/// The scroll facts the assertions read.
fn scroll_facts(state: &LibraryState) -> String {
    let scroll = *state.music_scroll.peek();
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

// Down with the pointer over the Music list scrolls the list by the height of
// the row at the top edge (design D6), picks nothing, and changes no Hero.
#[test]
fn down_over_the_music_list_scrolls_the_list_by_one_row() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &[]);
        route_on_row(
            state,
            facts,
            "down",
            "ArrowDown",
            PointerList::Music,
            "artist:service:a1",
        );
        facts.lock().expect("facts").push(scroll_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    // The letter heading covers the list's top edge, so the step is its 24 px.
    assert_eq!(facts[1], "top=24 viewport=40");
    assert_eq!(commands(&rx), []);
}

// The movement keys scroll the destination's list only while the pointer rests
// over it (design D2, step 4): over the Hero's tracklist they are the core's.
#[test]
fn down_over_the_tracklist_is_the_cores_key() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        route_hero(state, facts, "down", "ArrowDown", "t1");
    });
    assert_eq!(facts, ["down=Core(None)"]);
    assert_eq!(commands(&rx), []);
}

// The Library is mouse-driven for picking and pointer-driven for playback
// (design D2): Enter plays the hovered row's item and never picks it, so a
// mouse press stays the only way to pick an album for the Hero.
#[test]
fn enter_plays_the_hovered_album_row() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &["artist:service:a1"]);
        route_on_row(
            state,
            facts,
            "enter",
            "Enter",
            PointerList::Music,
            "album:al2",
        );
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::Play {
            item_id: "al2".to_owned(),
        }
        .into()]
    );
    assert_eq!(facts, ["enter=Handled"]);
}

// A hovered track row plays its own track, not the album a pick would show.
#[test]
fn enter_plays_the_hovered_track_row() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &["artist:service:a1", "album:al1"]);
        route_on_row(
            state,
            facts,
            "enter",
            "Enter",
            PointerList::Music,
            "track:t1",
        );
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::Play {
            item_id: "t1".to_owned(),
        }
        .into()]
    );
    assert_eq!(facts, ["enter=Handled"]);
}

// Enter, `n`, and `a` with no hovered row do nothing (design D2, step 3): the
// key is the window's, and it sends no command.
#[test]
fn enter_n_and_a_with_no_hovered_row_send_nothing() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &[]);
        route(state, facts, "enter", "Enter", false);
        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["enter=Handled", "n=Handled", "a=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `n` on a hovered album row sends play next for that album.
#[test]
fn n_on_the_hovered_album_row_sends_play_next_for_that_album() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &["artist:service:a1"]);
        route_on_row(state, facts, "n", "n", PointerList::Music, "album:al1");
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::PlayNext {
            item_id: "al1".to_owned(),
        }
        .into()]
    );
    assert_eq!(facts, ["n=Handled"]);
}

// `a` with the pointer on a tracklist row adds that track to the queue.
#[test]
fn a_with_the_pointer_on_a_tracklist_row_sends_add_to_queue_for_that_track() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        route_hero(state, facts, "a", "a", "t2");
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::AddToQueue {
            item_id: "t2".to_owned(),
        }
        .into()]
    );
    assert_eq!(facts, ["a=Handled"]);
}

// Enter on a tracklist row plays the track the pointer marks.
#[test]
fn enter_on_a_tracklist_row_plays_that_track() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        route_hero(state, facts, "enter", "Enter", "t2");
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::Play {
            item_id: "t2".to_owned(),
        }
        .into()]
    );
    assert_eq!(facts, ["enter=Handled"]);
}

// Esc closes the open item menu and sends no key.
#[test]
fn esc_closes_the_open_item_menu() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.item_menu_open = true);
        route(state, facts, "escape", "Escape", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "escape=Handled");
    assert!(
        facts[1].contains("filter_open=false"),
        "the Esc reached the window's own menu, not the core"
    );
    assert_eq!(commands(&rx), []);
}
