// Task 10.2 evidence for the Audiobookshelf Books panel-skeleton migration.
//
// Books now paints through the shared Library panel skeleton
// (`render_wide_skeleton`/`render_narrow_skeleton`) instead of its own
// bespoke painter: the Wide hero exposes the chapter list as the panel's
// Workspace box, and the Narrow inline hero shows the book's facts with no
// chapters at all (the "conforms to Emby" unification -- chapter activation
// only ever opens the selection modal at that breakpoint).

use crate::app::components::audiobookshelf_book::AudiobookshelfBookComponent;
use crate::app::components::library_panel::HeroImageState;
use crate::app::render::components::media_list::{
    INLINE_MEDIA_BROWSER_PAINTS, PLAIN_ROWS_PAINTS, WIDE_MEDIA_LIST_PAINTS,
};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use mbv_core::audiobookshelf::{
    AudiobookshelfBook, AudiobookshelfBookProgress, AudiobookshelfChapter, AudiobookshelfLibrary,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;

fn book_catalog_state(count: usize) -> AudiobookshelfBookBrowseState {
    let library = AudiobookshelfLibrary {
        id: "books".into(),
        name: "Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library);
    state.books = (0..count)
        .map(|index| AudiobookshelfBook {
            library_item_id: format!("book-{index}"),
            title: format!("Book {index:02}"),
            author_display: Some("Author".into()),
            author_sort_key: "Author".into(),
            cover_path: None,
            duration_seconds: 60.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        })
        .collect();
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
    state.selected_id = state.books.first().map(|book| book.library_item_id.clone());
    state
}

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect()
}

/// The Wide hero exposes the selected book's chapters as the panel's
/// Workspace box, painted through the same canonical `WideMediaList` the
/// Browser pane uses.
#[test]
fn book_wide_workspace_box_shows_the_selected_book_chapters() {
    let mut state = book_catalog_state(1);
    let chapter = AudiobookshelfChapter {
        id: 0,
        start: 0.0,
        end: 60.0,
        title: "Chapter One".into(),
    };
    state.books[0].chapters = vec![chapter.clone()];
    state
        .detail_cache
        .insert("book-0".into(), (vec![chapter], Vec::new()));
    state.progress.insert(
        "book-0".into(),
        AudiobookshelfBookProgress {
            library_item_id: "book-0".into(),
            current_time_seconds: 30.0,
            is_finished: false,
        },
    );
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    term.draw(|f| component.view(f, f.area())).unwrap();

    assert!(
        component.geometry().hero_area.is_some(),
        "the Wide hero pane painted"
    );
    assert!(
        component.chapter_content_rect_for_test().is_some(),
        "the chapter Workspace box painted"
    );
    let text = buffer_text(&term);
    assert!(text.contains("Chapter One"));
    assert!(text.contains("Book 00"));
    assert!(text.contains("50%"));
}

/// The Narrow inline hero shows the selected book's facts (title, progress)
/// but never the chapter list -- Books conforms to Emby, and chapter
/// activation only opens the selection modal at this breakpoint.
#[test]
fn book_narrow_inline_hero_omits_chapters() {
    let mut state = book_catalog_state(1);
    state.books[0].author_display = Some("Jane Doe".into());
    let chapter = AudiobookshelfChapter {
        id: 0,
        start: 0.0,
        end: 60.0,
        title: "Chapter One".into(),
    };
    state.books[0].chapters = vec![chapter.clone()];
    state
        .detail_cache
        .insert("book-0".into(), (vec![chapter], Vec::new()));
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    let mut term = Terminal::new(TestBackend::new(60, 24)).unwrap();
    term.draw(|f| component.view(f, f.area())).unwrap();

    assert!(
        component.geometry().hero_area.is_some(),
        "the Narrow inline hero is admitted"
    );
    let text = buffer_text(&term);
    assert!(text.contains("Book 00"));
    assert!(text.contains("Doe"));
    assert!(
        !text.contains("Chapter One"),
        "the narrow inline hero shows no chapters: {text:?}"
    );
    assert!(
        text.contains("A"),
        "the shared surname pill row still renders its label"
    );
}

/// Each breakpoint runs exactly one canonical list painter for the Browser
/// pane; with no chapters the chapter Workspace's list is empty and paints
/// nothing (its `PanelList::view` is a no-op on an empty carrier).
#[test]
fn book_each_breakpoint_runs_exactly_one_canonical_list_painter() {
    let state = book_catalog_state(30);
    let reset = || {
        WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));
        INLINE_MEDIA_BROWSER_PAINTS.with(|c| c.set(0));
        PLAIN_ROWS_PAINTS.with(|c| c.set(0));
    };

    let mut wide = AudiobookshelfBookComponent::new();
    wide.set_content(&state, false);
    wide.set_focused(true);
    reset();
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    term.draw(|f| wide.view(f, f.area())).unwrap();
    assert_eq!(WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get), 1);
    assert_eq!(INLINE_MEDIA_BROWSER_PAINTS.with(std::cell::Cell::get), 0);
    assert_eq!(PLAIN_ROWS_PAINTS.with(std::cell::Cell::get), 0);

    let mut narrow = AudiobookshelfBookComponent::new();
    narrow.set_content(&state, false);
    narrow.set_focused(true);
    reset();
    let mut term = Terminal::new(TestBackend::new(60, 24)).unwrap();
    term.draw(|f| narrow.view(f, f.area())).unwrap();
    assert_eq!(INLINE_MEDIA_BROWSER_PAINTS.with(std::cell::Cell::get), 1);
    assert_eq!(WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get), 0);
    assert_eq!(PLAIN_ROWS_PAINTS.with(std::cell::Cell::get), 0);
}

/// The hero reads only the shell-projected image state (design D9): the
/// panel reserves the policy's box and the shell paints the cached protocol
/// into it, so no paint-time cover fetch exists in the painter.
#[test]
fn book_wide_hero_uses_the_projected_image_state() {
    let mut state = book_catalog_state(1);
    state.books[0].cover_path = Some("cover.jpg".into());
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true);
    component.set_focused(true);
    component.set_hero_image(HeroImageState::Ready {
        cache_key: "book-0:cover".into(),
        decoded: Some((400, 600)),
    });

    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    term.draw(|f| component.view(f, f.area())).unwrap();

    let paint = component
        .take_panel_image_paint()
        .expect("the Wide header reserves the projected image box");
    assert_eq!(paint.cache_key, "book-0:cover");
    assert!(paint.area.width > 0 && paint.area.height > 0);
}

/// While the projection is still loading, the hero keeps its box and paints
/// the shared placeholder -- it neither fetches nor drops the block.
#[test]
fn book_narrow_inline_hero_keeps_the_box_while_the_image_loads() {
    let mut state = book_catalog_state(1);
    state.books[0].cover_path = Some("cover.jpg".into());
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true);
    component.set_focused(true);
    component.set_hero_image(HeroImageState::Loading);

    let mut term = Terminal::new(TestBackend::new(60, 24)).unwrap();
    term.draw(|f| component.view(f, f.area())).unwrap();

    assert!(
        component.take_panel_image_paint().is_none(),
        "a loading image is not painted by the shell yet"
    );
    assert!(
        component.geometry().hero_area.is_some(),
        "the inline hero block still reserves its box"
    );
}
