use super::test_helpers::*;
use super::*;
use crate::app::tests::make_app_stub;
use crate::app::types_audiobookshelf_browse::{
    build_surname_buckets, AudiobookshelfBookBrowseState,
};
use crate::app::TabSelection;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};

fn book(id: &str, title: &str, author_surname: &str) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: title.into(),
        author_display: Some(author_surname.into()),
        author_sort_key: author_surname.into(),
        cover_path: None,
        duration_seconds: 0.0,
        narrator: None,
        published_year: None,
        genres: Vec::new(),
        description: None,
        series_name: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

/// Variant of [`book`] that populates description, narrator, year, and
/// duration so hero-metadata render tests can assert they appear.
fn book_with_meta(
    id: &str,
    title: &str,
    author: &str,
    description: &str,
    narrator: Option<&str>,
    year: Option<&str>,
    duration: f64,
) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: title.into(),
        author_display: Some(author.into()),
        author_sort_key: author.into(),
        cover_path: None,
        duration_seconds: duration,
        narrator: narrator.map(str::to_string),
        published_year: year.map(str::to_string),
        genres: Vec::new(),
        description: Some(description.into()),
        series_name: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

/// Three books spanning three different alphabetical surname buckets
/// (Adams -> A-C, Mason -> J-L... actually M-O, Zephyr -> V-Z), so the
/// A-C bucket is selected by default and only "Alpha Tales" is in range.
fn make_audiobookshelf_book_app() -> App {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library.clone());
    state.append_page_books(
        0,
        3,
        vec![
            book("book-a", "Alpha Tales", "Adams"),
            book("book-m", "Middle Ground", "Mason"),
            book("book-z", "Zenith Story", "Zephyr"),
        ],
    );
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 60.0,
                title: "Chapter One".into(),
            }],
            Vec::new(),
        ),
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

#[test]
fn surname_buckets_use_inclusive_boundaries_and_omit_empty_ranges() {
    let books = [
        book("a", "A", "Adams"),
        book("c", "C", "Carter"),
        book("d", "D", "Dover"),
        book("f", "F", "Frost"),
        book("g", "G", "Grant"),
        book("i", "I", "Irwin"),
        book("j", "J", "Jones"),
        book("l", "L", "Lane"),
        book("m", "M", "Mason"),
        book("o", "O", "Oliver"),
        book("p", "P", "Poe"),
        book("r", "R", "Roth"),
        book("s", "S", "Stone"),
        book("u", "U", "Underwood"),
        book("v", "V", "Vale"),
        book("z", "Z", "Zane"),
    ];
    let buckets = build_surname_buckets(&books);
    assert_eq!(
        buckets
            .iter()
            .map(|bucket| bucket.label)
            .collect::<Vec<_>>(),
        [
            "A\u{2013}C",
            "D\u{2013}F",
            "G\u{2013}I",
            "J\u{2013}L",
            "M\u{2013}O",
            "P\u{2013}R",
            "S\u{2013}U",
            "V\u{2013}Z",
        ]
    );
    assert_eq!(
        buckets.first().map(|bucket| (bucket.start, bucket.end)),
        Some((0, 2))
    );
    assert_eq!(
        buckets.last().map(|bucket| (bucket.start, bucket.end)),
        Some((14, 16))
    );

    let sparse = [book("a", "A", "Adams"), book("z", "Z", "Zane")];
    assert_eq!(
        build_surname_buckets(&sparse)
            .iter()
            .map(|bucket| bucket.label)
            .collect::<Vec<_>>(),
        ["A\u{2013}C", "V\u{2013}Z"]
    );
}

#[test]
fn refresh_reanchors_selected_book_and_bucket_after_resort() {
    let mut state = AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    });
    state.append_page_books(
        0,
        3,
        vec![
            book("z", "Z", "Zephyr"),
            book("a", "A", "Adams"),
            book("m", "M", "Mason"),
        ],
    );
    state.select(2);
    assert_eq!(state.selected_id.as_deref(), Some("z"));

    state.books.clear();
    state.buckets.clear();
    state.total = 0;
    state.append_page_books(
        0,
        3,
        vec![
            book("z", "Z", "Zephyr"),
            book("m", "M", "Mason"),
            book("a", "A", "Adams"),
        ],
    );

    assert_eq!(state.selected_id.as_deref(), Some("z"));
    assert_eq!(state.cursor(), 2);
    assert_eq!(state.buckets[state.selected_bucket].label, "V\u{2013}Z");
}

/// Book-browsing spec: "Book libraries use the Music tab composition" --
/// both the hero+chapters pane and the bucket-filtered browser pane render
/// at once, with no Enter/open action, at the wide two-column breakpoint.
#[test]
fn wide_layout_renders_hero_chapters_and_browser_together() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let out = render_library_to_string_sized(&mut app, &mut layout, 100, 20);

    assert!(
        out.contains("Alpha Tales"),
        "hero must show the cursor's book without an open action:\n{out}"
    );
    assert!(
        out.contains("Chapter One"),
        "the persistent chapter list must render beside the hero:\n{out}"
    );
    assert!(
        out.contains("A\u{2013}C"),
        "the alphabetical-bucket pill row must render in the right pane:\n{out}"
    );
    assert!(
        layout.audiobookshelf_book_right_area.width > 0
            && layout.audiobookshelf_book_right_area.height > 0,
        "the right-pane browser area must be populated in wide mode"
    );
}

/// Task 2.4: the narrow-terminal fallback must still render both the
/// hero+chapters pane and the bucket-filtered browser, not just the hero.
#[test]
fn narrow_layout_still_renders_hero_chapters_and_browser_together() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let out = render_library_to_string(&mut app, &mut layout); // 60x20, below TWO_COLUMN_THRESHOLD

    assert!(
        out.contains("Alpha Tales"),
        "narrow hero must still show the cursor's book:\n{out}"
    );
    assert!(
        out.contains("A\u{2013}C"),
        "narrow layout must still render the bucket-pill row:\n{out}"
    );
    assert!(
        layout.audiobookshelf_book_right_area.height > 0,
        "narrow layout must still populate the browser area, not just the hero"
    );
}

/// The hero must render the book's author, narrator, year, and description
/// -- metadata the API parsing now carries (iteration 2 root-cause fix).
/// Both the narrow and wide paths go through the shared beside-image hero,
/// so both must show the same metadata.
#[test]
fn hero_renders_author_narrator_year_and_description() {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library.clone());
    state.append_page_books(
        0,
        1,
        vec![book_with_meta(
            "book-a",
            "Alpha Tales",
            "Adams",
            "A sweeping description of alpha things.",
            Some("Jim"),
            Some("2024"),
            3600.0,
        )],
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;

    // Narrow (single-column hero-on-top) — 80 cols gives the meta row
    // enough room for the narrator span without truncation.
    let mut layout = LayoutMain::default();
    let out = render_library_to_string_sized(&mut app, &mut layout, 80, 24);
    assert!(
        out.contains("Alpha Tales"),
        "hero must show the title:\n{out}"
    );
    assert!(out.contains("Adams"), "hero must show the author:\n{out}");
    assert!(
        out.contains("Read by Jim"),
        "hero must show the narrator:\n{out}"
    );
    assert!(
        out.contains("2024"),
        "hero must show the publication year:\n{out}"
    );
    assert!(
        out.contains("sweeping description"),
        "hero must show the description:\n{out}"
    );
    assert!(out.contains("1h"), "hero must show the duration:\n{out}");
}

/// Selecting a different bucket narrows the right-pane list: a book outside
/// the selected bucket is not reachable by scrolling until its bucket is
/// selected (book-browsing spec).
#[test]
fn selecting_a_bucket_narrows_cursor_movement_to_that_range() {
    let mut app = make_audiobookshelf_book_app();
    // Books sorted by surname: Adams(0, A-C), Mason(1, M-O), Zephyr(2, V-Z)
    // -> three singleton buckets.
    let state = &app.audiobookshelf_book_browse[0];
    assert_eq!(state.buckets.len(), 3, "expected three singleton buckets");
    assert_eq!(
        state.selected_bucket, 0,
        "cursor starts in the first bucket"
    );

    // Moving the cursor within the A-C bucket (a single book) must not
    // cross into the next bucket.
    app.move_audiobookshelf_book_cursor(1);
    assert_eq!(
        app.audiobookshelf_book_browse[0].selected_id.as_deref(),
        Some("book-a"),
        "cursor movement must clamp within the selected bucket"
    );

    // Selecting the M-O bucket (position 1) moves the cursor into it.
    app.select_audiobookshelf_book_bucket(1);
    assert_eq!(app.audiobookshelf_book_browse[0].selected_bucket, 1);
    assert_eq!(
        app.audiobookshelf_book_browse[0].selected_id.as_deref(),
        Some("book-m"),
        "selecting a bucket must move the cursor into it"
    );

    let mut layout = LayoutMain::default();
    let out = render_library_to_string_sized(&mut app, &mut layout, 100, 20);
    assert!(out.contains("Middle Ground"));
    assert!(!out.contains("Zenith Story"));
    assert!(!out.contains("Alpha Tales"));
}

/// Book-browsing spec: "Hero tracks the browser cursor without an explicit
/// open action" -- moving the cursor updates `selected_id` without touching
/// `chapter_selection` (which pane-focus flag governs, not visibility now).
#[test]
fn cursor_movement_updates_selection_without_opening_chapter_focus() {
    let mut app = make_audiobookshelf_book_app();
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, None);

    app.select_audiobookshelf_book_bucket(2); // V-Z: only "Zenith Story"
    assert_eq!(
        app.audiobookshelf_book_browse[0].selected_id.as_deref(),
        Some("book-z")
    );
    assert_eq!(
        app.audiobookshelf_book_browse[0].chapter_selection, None,
        "moving the browser cursor must not implicitly focus chapters"
    );
}

/// Left/right arrow toggles pane focus without hiding either pane
/// (book-browsing spec scenario).
#[test]
fn left_right_focus_toggle_leaves_both_panes_populated() {
    let mut app = make_audiobookshelf_book_app();
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, None);

    let right = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Right,
        crossterm::event::KeyModifiers::NONE,
    );
    let left = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Left,
        crossterm::event::KeyModifiers::NONE,
    );
    assert_eq!(app.handle_key_view_dispatch(left), Some(false));
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, Some(0));

    let mut layout = LayoutMain::default();
    let out = render_library_to_string_sized(&mut app, &mut layout, 100, 20);
    assert!(
        out.contains("Alpha Tales") && out.contains("A\u{2013}C"),
        "focusing chapters must not hide the hero or the right-pane browser:\n{out}"
    );

    assert_eq!(app.handle_key_view_dispatch(right), Some(false));
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, None);
    let out = render_library_to_string_sized(&mut app, &mut layout, 100, 20);
    assert!(
        out.contains("Alpha Tales") && out.contains("A\u{2013}C"),
        "focusing the browser must not hide the hero or the chapter list:\n{out}"
    );
}

/// Eager chapter/audio-file detail fetch: moving the cursor onto a book
/// without a cached detail issues one fetch (`detail_loading` flips),
/// mirroring `fetch_album_tracks`'s eager-fetch-on-cursor-move guard.
#[test]
fn cursor_movement_onto_uncached_book_starts_a_detail_fetch() {
    let mut app = make_audiobookshelf_book_app();
    // "book-m" has no cached detail (only "book-a" does).
    app.select_audiobookshelf_book_bucket(1);
    assert_eq!(
        app.audiobookshelf_book_browse[0].selected_id.as_deref(),
        Some("book-m")
    );
    assert!(
        app.audiobookshelf_book_browse[0].detail_loading,
        "an uncached book's detail fetch must start as soon as the cursor moves onto it"
    );
}

/// A cached book's detail is not re-requested on cursor movement (the
/// existing `detail_cache` guard in `start_audiobookshelf_book_detail`).
#[test]
fn selecting_a_cached_book_does_not_start_a_new_fetch() {
    let mut app = make_audiobookshelf_book_app();
    // "book-a" already has cached detail from `make_audiobookshelf_book_app`.
    app.select_audiobookshelf_book_bucket(0);
    assert!(
        !app.audiobookshelf_book_browse[0].detail_loading,
        "a cached book's detail must not be (re)fetched"
    );
}

/// Moving onto a cached book while a *different* book's fetch is still in
/// flight must not render the cached book as loading -- `detail_loading`
/// must reflect the currently-selected book, not whichever fetch is still
/// outstanding in the background.
#[test]
fn cursor_movement_does_not_show_stale_loading_for_a_cached_book() {
    let mut app = make_audiobookshelf_book_app();
    // Move onto the uncached "book-m" first (starts its fetch).
    app.select_audiobookshelf_book_bucket(1);
    assert!(app.audiobookshelf_book_browse[0].detail_loading);

    // Moving back onto the already-cached "book-a" must show its cached
    // content immediately, not a stale "Loading…" left over from book-m's
    // still-in-flight fetch.
    app.select_audiobookshelf_book_bucket(0);
    assert!(
        !app.audiobookshelf_book_browse[0].detail_loading,
        "a cached book's detail must never render as loading, even while a \
         different book's fetch is still in flight"
    );
}

#[test]
fn in_flight_book_detail_is_not_requested_again_after_cursor_round_trip() {
    let mut app = make_audiobookshelf_book_app();
    app.select_audiobookshelf_book_bucket(1);
    app.select_audiobookshelf_book_bucket(2);
    assert!(app.audiobookshelf_book_browse[0]
        .detail_loading_ids
        .contains("book-m"));
    assert!(app.audiobookshelf_book_browse[0]
        .detail_loading_ids
        .contains("book-z"));

    app.select_audiobookshelf_book_bucket(1);
    assert!(app.audiobookshelf_book_browse[0]
        .detail_loading_ids
        .contains("book-m"));
    assert!(app.audiobookshelf_book_browse[0].detail_loading);
}
