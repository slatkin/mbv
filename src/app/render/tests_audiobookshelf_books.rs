use crate::app::components::audiobookshelf_book::AudiobookshelfBookComponent;
use crate::app::render::components::media_list::{
    INLINE_MEDIA_BROWSER_PAINTS, PLAIN_ROWS_PAINTS, WIDE_MEDIA_LIST_PAINTS,
};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfLibrary};
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

/// §3.2 one-painter proof: the bespoke `render_book_browser` inline flow is
/// gone. Each Book breakpoint runs exactly one canonical list painter -- the
/// wide `WideMediaList` rail or the narrow persistent `InlineMediaBrowser` --
/// and never the plain-rows path. In particular the wide right rail runs no
/// `render_inline_media_browser`, so no Wide selected-row replacement remains.
#[test]
fn book_each_breakpoint_runs_exactly_one_canonical_list_painter() {
    let state = book_catalog_state(30);
    let reset = || {
        WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));
        INLINE_MEDIA_BROWSER_PAINTS.with(|c| c.set(0));
        PLAIN_ROWS_PAINTS.with(|c| c.set(0));
    };

    let mut wide = AudiobookshelfBookComponent::new();
    wide.set_content(&state, true, false);
    reset();
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    term.draw(|f| wide.view(f, f.area())).unwrap();
    assert_eq!(WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get), 1);
    assert_eq!(INLINE_MEDIA_BROWSER_PAINTS.with(std::cell::Cell::get), 0);
    assert_eq!(PLAIN_ROWS_PAINTS.with(std::cell::Cell::get), 0);

    let mut narrow = AudiobookshelfBookComponent::new();
    narrow.set_content(&state, true, false);
    reset();
    let mut term = Terminal::new(TestBackend::new(60, 24)).unwrap();
    term.draw(|f| narrow.view(f, f.area())).unwrap();
    assert_eq!(INLINE_MEDIA_BROWSER_PAINTS.with(std::cell::Cell::get), 1);
    assert_eq!(WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get), 0);
    assert_eq!(PLAIN_ROWS_PAINTS.with(std::cell::Cell::get), 0);
}
