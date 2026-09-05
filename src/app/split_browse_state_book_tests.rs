//! Characterization tests for `split-browse-state-interaction-fields` task 2.1.
//!
//! They pin two behaviours the `AudiobookshelfBookBrowseState` content/
//! interaction split (tasks 2.2–2.4) must not regress:
//!   1. the selected book is restored on tab re-entry from the saved position;
//!   2. the selected surname bucket re-anchors to the selected book when a
//!      page append shifts book indices.

use super::super::types_audiobookshelf_browse::{
    build_surname_buckets, AudiobookshelfBookBrowseState,
};
use super::super::types_tab_selection::TabSelection;
use crate::app::tests::make_app_stub;
use mbv_core::config::AudiobookshelfSetup;

fn library() -> mbv_core::audiobookshelf::AudiobookshelfLibrary {
    mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Books".into(),
        media_type: "book".into(),
    }
}

fn books(surnames: &[&str]) -> Vec<mbv_core::audiobookshelf::AudiobookshelfBook> {
    surnames
        .iter()
        .map(|surname| mbv_core::audiobookshelf::AudiobookshelfBook {
            library_item_id: format!("book-{surname}"),
            title: format!("A Book by {surname}"),
            author_display: Some((*surname).into()),
            author_sort_key: (*surname).into(),
            cover_path: None,
            duration_seconds: 0.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        })
        .collect()
}

#[test]
fn book_position_restores_selected_id_after_tab_switch_away_and_back() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://books.example"));
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.audiobookshelf_libraries.push(library());
    let mut state = AudiobookshelfBookBrowseState::new(library());
    state.books = books(&["Adams", "Brown", "Clark", "Davis", "Evans"]);
    state.total = 5;
    state.buckets = build_surname_buckets(&state.books);
    app.audiobookshelf_book_browse.push(state);

    app.select_audiobookshelf_book(3);
    assert_eq!(
        app.audiobookshelf_book_browse[0].selected_id.as_deref(),
        Some("book-Davis"),
    );

    // Tab away and back: re-entry has a fresh browse state with no selection,
    // then the saved position is re-applied.
    app.audiobookshelf_book_browse[0].selected_id = None;
    app.activate_audiobookshelf_book_position(0);

    assert_eq!(
        app.audiobookshelf_book_browse[0].selected_id.as_deref(),
        Some("book-Davis"),
        "the saved position must restore the selected book on re-entry",
    );
}

#[test]
fn selected_bucket_reanchors_to_selected_book_after_page_append() {
    use crate::app::components::AudiobookshelfBookComponent;

    let mut state = AudiobookshelfBookBrowseState::new(library());
    state.append_page_books(0, 4, books(&["Brown", "Davis"]));
    state.selected_id = Some("book-Davis".into());
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);

    // Page in earlier surnames, shifting "Davis" to a higher index. The
    // content push re-anchors the component's bucket onto the still-selected
    // book.
    state.append_page_books(1, 4, books(&["Adams", "Carter"]));
    component.set_content(&state, false);
    component.set_focused(true);

    let davis_after = state
        .books
        .iter()
        .position(|book| book.author_sort_key == "Davis")
        .unwrap();
    let bucket_after = state.buckets[component.selected_bucket()];
    assert!(
        davis_after >= bucket_after.start && davis_after < bucket_after.end,
        "selected bucket must still contain the selected book after a page append",
    );
}
