#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::tests::make_item;
use crate::app::{
    LibEvent, SelectionModalListState, SelectionModalRow, SelectionModalSource, TabSelection,
};

#[test]
fn audiobookshelf_podcast_detail_success_refreshes_matching_open_modal() {
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "podcasts".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut browse = AudiobookshelfBrowseState::new(library.clone());
    browse.append_page(
        0,
        20,
        1,
        vec![mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-1".into(),
            title: "Show 1".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    browse.selected_id = Some("show-1".into());
    browse.detail_loading = true;
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(browse);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Podcast {
            library_item_id: "show-1".into(),
        },
        title: "Show 1".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });

    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: Default::default(),
        library_item_id: "show-1".into(),
        result: Ok(vec![
            mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
                library_item_id: "show-1".into(),
                episode_id: "episode-1".into(),
                title: "Episode 1".into(),
                published_at: None,
                duration_seconds: Some(60.0),
            },
        ]),
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert!(matches!(modal.state, SelectionModalListState::Ready(_)));
    assert_eq!(modal.state.rows()[0].item_id(), Some("episode-1"));
}

#[test]
fn audiobookshelf_podcast_completion_projects_event_show_when_browse_selection_differs() {
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "podcasts".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut browse = AudiobookshelfBrowseState::new(library.clone());
    browse.append_page(
        0,
        20,
        2,
        vec![
            mbv_core::audiobookshelf::AudiobookshelfShow {
                library_item_id: "show-1".into(),
                title: "Show 1".into(),
                author: None,
                description: None,
                cover_path: None,
            },
            mbv_core::audiobookshelf::AudiobookshelfShow {
                library_item_id: "show-2".into(),
                title: "Show 2".into(),
                author: None,
                description: None,
                cover_path: None,
            },
        ],
    );
    browse.selected_id = Some("show-2".into());
    browse.episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-2".into(),
            episode_id: "episode-from-selected-show".into(),
            title: "Wrong episode".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(browse);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Podcast {
            library_item_id: "show-1".into(),
        },
        title: "Show 1".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });

    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: Default::default(),
        library_item_id: "show-1".into(),
        result: Ok(vec![
            mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
                library_item_id: "show-1".into(),
                episode_id: "episode-from-event-show".into(),
                title: "Correct episode".into(),
                published_at: None,
                duration_seconds: Some(60.0),
            },
        ]),
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert_eq!(
        modal.state.rows()[0].item_id(),
        Some("episode-from-event-show")
    );
}

#[test]
fn audiobookshelf_book_detail_success_refreshes_matching_open_modal() {
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "books".into(),
        name: "Books".into(),
        media_type: "book".into(),
    };
    let mut browse = AudiobookshelfBookBrowseState::new(library.clone());
    browse
        .books
        .push(mbv_core::audiobookshelf::AudiobookshelfBook {
            library_item_id: "book-1".into(),
            title: "Book 1".into(),
            author_display: None,
            author_sort_key: String::new(),
            cover_path: None,
            duration_seconds: 60.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        });
    browse.selected_id = Some("book-1".into());
    browse.detail_loading_ids.insert("book-1".into());
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(browse);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Book {
            book_id: "book-1".into(),
        },
        title: "Book 1".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });

    app.handle_lib_event(LibEvent::AudiobookshelfBookDetailFetched {
        generation: Default::default(),
        library_item_id: "book-1".into(),
        result: Ok((
            vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
                id: 1,
                start: 0.0,
                end: 60.0,
                title: "Chapter 1".into(),
            }],
            Vec::new(),
        )),
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert!(matches!(modal.state, SelectionModalListState::Ready(_)));
    assert_eq!(modal.state.rows()[0].item_id(), Some("chapter:1"));
}

#[test]
fn audiobookshelf_podcast_detail_error_ends_matching_modal_loading() {
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "podcasts".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut browse = AudiobookshelfBrowseState::new(library.clone());
    browse.append_page(
        0,
        20,
        1,
        vec![mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-1".into(),
            title: "Show 1".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    browse.detail_loading = true;
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(browse);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Podcast {
            library_item_id: "show-1".into(),
        },
        title: "Show 1".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });

    app.handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
        generation: Default::default(),
        library_item_id: "show-1".into(),
        result: Err(mbv_core::audiobookshelf::AudiobookshelfError {
            class: mbv_core::audiobookshelf::AudiobookshelfFailureClass::Connectivity,
        }),
    });

    assert!(matches!(
        app.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Empty
    ));
}

#[test]
fn audiobookshelf_book_detail_error_ends_matching_modal_loading() {
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "books".into(),
        name: "Books".into(),
        media_type: "book".into(),
    };
    let mut browse = AudiobookshelfBookBrowseState::new(library.clone());
    browse
        .books
        .push(mbv_core::audiobookshelf::AudiobookshelfBook {
            library_item_id: "book-1".into(),
            title: "Book 1".into(),
            author_display: None,
            author_sort_key: String::new(),
            cover_path: None,
            duration_seconds: 60.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        });
    browse.detail_loading_ids.insert("book-1".into());
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(browse);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Book {
            book_id: "book-1".into(),
        },
        title: "Book 1".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });

    app.handle_lib_event(LibEvent::AudiobookshelfBookDetailFetched {
        generation: Default::default(),
        library_item_id: "book-1".into(),
        result: Err(mbv_core::audiobookshelf::AudiobookshelfError {
            class: mbv_core::audiobookshelf::AudiobookshelfFailureClass::Connectivity,
        }),
    });

    assert!(matches!(
        app.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Empty
    ));
}

#[test]
fn audiobookshelf_progress_refreshes_matching_podcast_modal() {
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.open_podcast_selection_modal();
    let update = mbv_core::player::AudiobookshelfProgressUpdate {
        generation: app.audiobookshelf_runtime.generation(),
        library_item_id: "show-a".into(),
        episode_id: "episode-a".into(),
        current_time_seconds: 60.0,
        duration_seconds: 60.0,
        is_finished: true,
    };

    app.handle_lib_event(LibEvent::AudiobookshelfProgressAcknowledged(update));

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert!(modal.state.rows().iter().any(|row| {
        matches!(row, SelectionModalRow::Item(item) if item.meta.contains("Played"))
    }));
}
