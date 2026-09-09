use crate::app::components::audiobookshelf_book::AudiobookshelfBookComponent;
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

/// §3.2 one-painter proof: the bespoke `render_book_browser` inline flow is
/// gone. Each Book breakpoint runs exactly one canonical list painter -- the
/// wide `WideMediaList` rail or the narrow persistent `InlineMediaBrowser` --
/// and never the plain-rows path. In particular the wide right rail runs no
/// `render_inline_media_browser`, so no Wide selected-row replacement remains.
#[test]
fn book_narrow_replacement_contains_chapters_and_shared_hero_evidence() {
    let mut state = book_catalog_state(2);
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
    state.books[1].author_display = Some("Ada Adams".into());
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
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
    let mut term = Terminal::new(TestBackend::new(60, 24)).unwrap();
    term.draw(|f| component.view(f, f.area())).unwrap();
    let text: String = term
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Chapter One"));
    assert!(text.contains("50%"));
    assert!(text.contains("Doe"));
    assert!(
        text.contains("A"),
        "shared surname pill row should render its label"
    );
}

#[test]
fn book_loading_cover_reserves_shared_series_image_slot() {
    let mut state = book_catalog_state(1);
    state.books[0].cover_path = Some("cover.jpg".into());
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true);
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    term.draw(|f| component.view(f, f.area())).unwrap();
    let Some(crate::app::render::HomeImagePaint::AudiobookshelfBookCover {
        area,
        show_placeholder,
        ..
    }) = component.take_image_paint()
    else {
        panic!("book cover paint request expected");
    };
    assert!(show_placeholder);
    assert_eq!(
        area.width,
        crate::app::render::components::detail_series_view::SERIES_IMAGE_COLS
    );
    assert_eq!(
        area.height,
        crate::app::render::components::detail_series_view::SERIES_IMAGE_ROWS
    );
}

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
