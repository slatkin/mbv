//! The `list` contracts of `keys.test.ts` (D16), over the shared harness.

use super::*;

#[test]
fn a_cursor_move_below_the_rows_in_view_scrolls_its_row_into_view() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &[]);
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.cursor = Some("artist:service:a1".to_owned()));
        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(ui_facts(state));
        let scroll = *state.music_scroll.peek();
        facts
            .lock()
            .expect("facts")
            .push(format!("top={}", scroll.scroll_top));
    });
    assert_eq!(facts[0], "down=Handled");
    assert!(facts[1].contains("cursor=Some(\"artist:service:a2\")"));
    assert_eq!(facts[2], "top=44");
    assert_eq!(commands(&rx), []);
}

// Right opens the cursor album and asks for its tracks.
#[test]
fn right_opens_the_cursor_album_and_asks_for_its_tracks() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &["artist:service:a1"]);
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.cursor = Some("album:al2".to_owned()));
        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::OpenAlbum {
            album_id: "al2".to_owned(),
        }
        .into()]
    );
    assert!(facts[1].contains("open=[\"artist:service:a1\", \"album:al2\"]"));
}

// The Library is mouse-driven for picking and keyboard-driven for playback
// (design D7): Enter plays the item under the cursor and never picks it, so a
// mouse press stays the only way to pick an album for the Hero.
#[test]
fn enter_in_the_list_plays_the_item_under_the_cursor() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &["artist:service:a1"]);
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.cursor = Some("album:al2".to_owned()));
        route(state, facts, "enter", "Enter", false);
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

// A track row plays its own track, not the album a pick would show.
#[test]
fn enter_in_the_list_plays_the_track_under_the_cursor() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &["artist:service:a1", "album:al1"]);
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.cursor = Some("track:t1".to_owned()));
        route(state, facts, "enter", "Enter", false);
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

// Review of row 4.5, finding P2: the filter opens a node that holds a match,
// so the cursor can rest on a track row that only the narrowing paints. Esc
// clears the filter, and the row goes: the cursor must move to a row that
// does paint, or Enter and Right would reach no row at all.
#[test]
fn esc_that_clears_the_filter_moves_a_stranded_cursor_to_a_painted_row() {
    let (facts, _rx) = run(|state, facts| {
        carry_list(state, &[]);
        state.set_library_filter("rhymin".to_owned());
        route(state, facts, "down", "ArrowDown", false);
        route(state, facts, "down2", "ArrowDown", false);
        facts
            .lock()
            .expect("facts")
            .push(format!("mid_cursor={:?}", state.ui.peek().cursor));
        state.open_library_filter();
        route(state, facts, "escape", "Escape", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    // The settle over the narrowed rows carries the cursor to the first
    // painted row, then the two presses land it on the track.
    assert_eq!(facts[2], "mid_cursor=Some(\"track:t1\")");
    // Esc clears the filter and the stranded cursor falls to a painted row.
    assert!(facts[4].contains("filter_open=false"));
    assert!(facts[4].contains("filter=\"\""));
    assert!(facts[4].contains("cursor=Some(\"artist:service:a1\")"));
}

// Review of row 4.7: the pointer's control decides what `n`, `a`, Enter, and
// the movement keys act on (design D2, step 3), and Esc closes the front end's
// own menu and overlay.
#[test]
fn n_with_the_list_cursor_on_an_album_sends_play_next_for_that_album() {
    let (facts, rx) = run(|state, facts| {
        carry_list(state, &["artist:service:a1"]);
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.cursor = Some("album:al1".to_owned()));
        route(state, facts, "n", "n", false);
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
#[test]
fn a_with_the_pointer_on_the_tracklist_sends_add_to_queue_for_the_cursor_track() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.track_cursor = Some("t1".to_owned()));
        route_hero(state, facts, "a", "a");
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::AddToQueue {
            item_id: "t1".to_owned(),
        }
        .into()]
    );
    assert_eq!(facts, ["a=Handled"]);
}
#[test]
fn down_with_the_pointer_on_the_tracklist_moves_the_tracklist_cursor() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.track_cursor = Some("t1".to_owned()));
        route_hero(state, facts, "down", "ArrowDown");
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "down=Handled",
            "filter_open=false filter=\"\" pill_select=None cursor=None open=[] track_cursor=Some(\"t2\") dismissed=true"
        ]
    );
    assert_eq!(commands(&rx), []);
}

// Review of row 4.7, finding P2-a: the tracklist's cursor derives from the
// tracks that paint, so Enter acts on the first track before any movement key.
#[test]
fn enter_on_the_tracklist_plays_the_track_the_painted_tracklist_shows() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        route_hero(state, facts, "enter", "Enter");
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
    assert!(facts[1].contains("filter_open=false"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the Narrow Hero overlay and sends no key.
