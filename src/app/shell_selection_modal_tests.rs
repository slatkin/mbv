#[cfg(test)]
mod selection_modal_tests {
    use super::super::components::{ComponentId, OverlayId, SelectionModalComponent};
    use super::super::shell::Model;
    use super::super::tests::make_app_stub;
    use super::super::types_audiobookshelf_browse::{
        AudiobookshelfBookBrowseState, AudiobookshelfBrowseState,
    };
    use super::super::types_overlay::OverlayRequest;
    use super::super::types_selection_modal::{
        SelectionModal, SelectionModalFilter, SelectionModalListState, SelectionModalSource,
    };
    use super::super::{App, LibEvent, TabSelection};
    use mbv_core::api::EmbyItem;
    use mbv_core::audiobookshelf::{
        AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfDownloadedEpisode,
        AudiobookshelfError, AudiobookshelfFailureClass, AudiobookshelfLibrary, AudiobookshelfShow,
    };
    use std::collections::HashMap;

    fn item(id: &str, name: &str, item_type: &str) -> EmbyItem {
        let mut item = super::super::tests::make_item(name, item_type);
        item.id = id.into();
        item
    }

    fn mount_selection_modal(
        model: &mut Model,
        source: SelectionModalSource,
        state: SelectionModalListState,
        filter: Option<SelectionModalFilter>,
    ) {
        model.app.pending_overlay = Some(OverlayRequest::SelectionModal(SelectionModal {
            source,
            title: "Selection".into(),
            state,
            cursor: 0,
            filter,
        }));
        model.sync_modal_requests();
    }

    fn selection_modal(model: &Model) -> &SelectionModalComponent {
        model
            .application
            .get_component(&ComponentId::Overlay(OverlayId::SelectionModal))
            .expect("Selection modal mounted")
            .as_any()
            .downcast_ref::<SelectionModalComponent>()
            .expect("Selection modal type")
    }

    fn sync_series_refresh(model: &mut Model) {
        model.sync_modal_requests();
        model.sync_modal_requests();
    }

    fn show(id: &str) -> AudiobookshelfShow {
        AudiobookshelfShow {
            library_item_id: id.into(),
            title: id.into(),
            author: None,
            description: None,
            cover_path: None,
        }
    }

    fn podcast_app(shows: Vec<AudiobookshelfShow>, selected_id: &str) -> App {
        let library = AudiobookshelfLibrary {
            id: "podcasts".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        };
        let mut browse = AudiobookshelfBrowseState::new(library.clone());
        browse.append_page(0, 20, shows.len(), shows);
        browse.selected_id = Some(selected_id.into());
        browse.detail_loading = true;
        let mut app = make_app_stub();
        app.audiobookshelf_libraries.push(library);
        app.audiobookshelf_browse.push(browse);
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        app
    }

    fn book(id: &str) -> AudiobookshelfBook {
        AudiobookshelfBook {
            library_item_id: id.into(),
            title: id.into(),
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
        }
    }

    fn book_app(id: &str) -> App {
        let library = AudiobookshelfLibrary {
            id: "books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        };
        let mut browse = AudiobookshelfBookBrowseState::new(library.clone());
        browse.books.push(book(id));
        browse.selected_id = Some(id.into());
        browse.detail_loading_ids.insert(id.into());
        let mut app = make_app_stub();
        app.audiobookshelf_libraries.push(library);
        app.audiobookshelf_book_browse.push(browse);
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        app
    }

    fn podcast_filter() -> Option<SelectionModalFilter> {
        Some(SelectionModalFilter {
            labels: vec!["All".into(), "Played".into(), "Unplayed".into()],
            selected: 0,
        })
    }

    #[test]
    fn album_tracks_fetched_event_populates_cache_and_clears_loading() {
        let mut app = make_app_stub();
        app.album_tracks_loading.insert("album-1".into());
        app.handle_lib_event(LibEvent::AlbumTracksFetched {
            album_id: "album-1".into(),
            tracks: vec![item("track-1", "Opening Track", "Audio")],
        });

        assert!(!app.album_tracks_loading.contains("album-1"));
        assert_eq!(app.album_tracks_cache["album-1"][0].id, "track-1");
    }

    #[test]
    fn album_tracks_completion_refreshes_matching_open_modal() {
        let mut model = Model::new(make_app_stub());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Album {
                album_id: "album-1".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        model.app.handle_lib_event(LibEvent::AlbumTracksFetched {
            album_id: "album-1".into(),
            tracks: vec![item("track-1", "Opening Track", "Audio")],
        });
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), Some("track-1"));
    }

    #[test]
    fn album_tracks_completion_does_not_refresh_a_different_open_modal() {
        let mut model = Model::new(make_app_stub());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Album {
                album_id: "album-2".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        model.app.handle_lib_event(LibEvent::AlbumTracksFetched {
            album_id: "album-1".into(),
            tracks: vec![item("track-1", "Track", "Audio")],
        });
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), None);
    }

    #[test]
    fn series_detail_completion_refreshes_matching_open_modal() {
        let mut model = Model::new(make_app_stub());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Series {
                series_id: "series-1".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        let season = item("season-1", "Season 1", "Season");
        let episode = item("episode-1", "Episode 1", "Episode");
        let mut episodes = HashMap::new();
        episodes.insert("season-1".into(), vec![episode]);
        model.app.handle_lib_event(LibEvent::SeriesDetailFetched {
            series_id: "series-1".into(),
            seasons: vec![season],
            episodes,
        });
        sync_series_refresh(&mut model);

        assert_eq!(selection_modal(&model).selected_id(), Some("episode-1"));
    }

    #[test]
    fn series_detail_completion_does_not_refresh_a_different_open_modal() {
        let mut model = Model::new(make_app_stub());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Series {
                series_id: "series-2".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        model.app.handle_lib_event(LibEvent::SeriesDetailFetched {
            series_id: "series-1".into(),
            seasons: Vec::new(),
            episodes: HashMap::new(),
        });
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), None);

        let season = item("season-2", "Season 2", "Season");
        let episode = item("episode-2", "Episode 2", "Episode");
        let mut episodes = HashMap::new();
        episodes.insert("season-2".into(), vec![episode]);
        model.app.handle_lib_event(LibEvent::SeriesDetailFetched {
            series_id: "series-2".into(),
            seasons: vec![season],
            episodes,
        });
        sync_series_refresh(&mut model);

        assert_eq!(selection_modal(&model).selected_id(), Some("episode-2"));
    }

    #[test]
    fn series_detail_completion_for_zero_seasons_shows_empty_modal() {
        let mut model = Model::new(make_app_stub());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Series {
                series_id: "series-1".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        model.app.handle_lib_event(LibEvent::SeriesDetailFetched {
            series_id: "series-1".into(),
            seasons: Vec::new(),
            episodes: HashMap::new(),
        });
        model.sync_modal_requests();

        assert!(matches!(
            model.app.pending_overlay.as_ref(),
            Some(OverlayRequest::RefreshSelectionModal {
                state: SelectionModalListState::Empty,
                ..
            })
        ));
        model.sync_modal_requests();
        assert!(matches!(
            selection_modal(&model).list_state(),
            Some(SelectionModalListState::Empty)
        ));
    }

    #[test]
    fn series_detail_completion_keeps_uncached_seasons_loading() {
        let mut model = Model::new(make_app_stub());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Series {
                series_id: "series-1".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        let mut season_one = item("season-1", "Season 1", "Season");
        season_one.index_number = 1;
        let mut season_two = item("season-2", "Season 2", "Season");
        season_two.index_number = 2;
        model.app.handle_lib_event(LibEvent::SeriesDetailFetched {
            series_id: "series-1".into(),
            seasons: vec![season_one, season_two],
            episodes: HashMap::new(),
        });
        model.sync_modal_requests();

        assert!(matches!(
            model.app.pending_overlay.as_ref(),
            Some(OverlayRequest::RefreshSelectionModal {
                state: SelectionModalListState::Loading,
                ..
            })
        ));
    }

    #[test]
    fn album_modal_opens_loading_while_existing_track_fetch_is_in_flight() {
        let mut app = make_app_stub();
        app.album_tracks_loading.insert("album-1".into());
        app.open_album_selection_modal(&item("album-1", "Album", "MusicAlbum"));

        assert!(matches!(
            app.pending_overlay,
            Some(OverlayRequest::SelectionModal(SelectionModal {
                state: SelectionModalListState::Loading,
                ..
            }))
        ));
    }

    #[test]
    fn album_tracks_completion_replaces_loading_with_sorted_rows() {
        let mut model = Model::new(make_app_stub());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Album {
                album_id: "album-1".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        let mut second = item("track-2", "Second", "Audio");
        second.index_number = 2;
        let mut first = item("track-1", "First", "Audio");
        first.index_number = 1;
        model.app.handle_lib_event(LibEvent::AlbumTracksFetched {
            album_id: "album-1".into(),
            tracks: vec![second, first],
        });
        model.sync_modal_requests();
        assert_eq!(
            model.app.album_tracks_cache["album-1"]
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            vec!["track-1", "track-2"]
        );
        assert_eq!(selection_modal(&model).selected_id(), Some("track-1"));

        model.app.handle_lib_event(LibEvent::AlbumTracksFetched {
            album_id: "album-1".into(),
            tracks: Vec::new(),
        });
        model.sync_modal_requests();
        assert!(model.app.album_tracks_cache["album-1"].is_empty());
        assert_eq!(selection_modal(&model).selected_id(), None);
    }

    #[test]
    fn series_season_completion_refreshes_matching_open_modal() {
        let mut model = Model::new(make_app_stub());
        let season = item("season-1", "Season 1", "Season");
        model.app.series_detail_cache.insert(
            "series-1".into(),
            super::super::SeriesDetail {
                seasons: vec![season],
                episodes: HashMap::new(),
            },
        );
        model
            .app
            .series_season_loading
            .insert(("series-1".into(), "season-1".into()));
        model.app.series_detail_loading.insert("series-1".into());
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Series {
                series_id: "series-1".into(),
            },
            SelectionModalListState::Loading,
            Some(SelectionModalFilter {
                labels: vec!["01".into()],
                selected: 0,
            }),
        );
        model
            .app
            .handle_lib_event(LibEvent::SeriesSeasonEpisodesFetched {
                series_id: "series-1".into(),
                season_id: "season-1".into(),
                episodes: vec![item("episode-1", "Episode 1", "Episode")],
            });
        sync_series_refresh(&mut model);

        assert_eq!(selection_modal(&model).selected_id(), Some("episode-1"));
        assert!(!model
            .app
            .series_season_loading
            .contains(&("series-1".into(), "season-1".into())));
        assert!(!model.app.series_detail_loading.contains("series-1"));
    }

    #[test]
    fn series_season_completion_does_not_refresh_a_different_open_modal() {
        let mut model = Model::new(make_app_stub());
        model.app.series_detail_cache.insert(
            "series-1".into(),
            super::super::SeriesDetail {
                seasons: vec![item("season-1", "Season 1", "Season")],
                episodes: HashMap::new(),
            },
        );
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Series {
                series_id: "series-2".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        model
            .app
            .handle_lib_event(LibEvent::SeriesSeasonEpisodesFetched {
                series_id: "series-1".into(),
                season_id: "season-1".into(),
                episodes: Vec::new(),
            });
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), None);
    }

    #[test]
    fn series_season_fetch_suppresses_duplicate_in_flight_requests() {
        let mut app = make_app_stub();
        app.series_detail_cache.insert(
            "series-1".into(),
            super::super::SeriesDetail {
                seasons: vec![item("season-1", "Season 1", "Season")],
                episodes: HashMap::new(),
            },
        );
        app.series_season_loading
            .insert(("series-1".into(), "season-1".into()));

        app.fetch_series_season_episodes("series-1".into(), "season-1".into());

        assert!(app
            .series_season_loading
            .contains(&("series-1".into(), "season-1".into())));
        assert!(matches!(
            app.lib_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn audiobookshelf_podcast_detail_success_refreshes_matching_open_modal() {
        let mut model = Model::new(podcast_app(vec![show("show-1")], "show-1"));
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Podcast {
                library_item_id: "show-1".into(),
            },
            SelectionModalListState::Loading,
            podcast_filter(),
        );
        model
            .app
            .handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
                generation: Default::default(),
                library_item_id: "show-1".into(),
                result: Ok(vec![AudiobookshelfDownloadedEpisode {
                    library_item_id: "show-1".into(),
                    episode_id: "episode-1".into(),
                    title: "Episode 1".into(),
                    published_at: None,
                    duration_seconds: Some(60.0),
                }]),
            });
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), Some("episode-1"));
    }

    #[test]
    fn audiobookshelf_podcast_completion_projects_event_show_when_browse_selection_differs() {
        let mut model = Model::new(podcast_app(vec![show("show-1"), show("show-2")], "show-2"));
        model.app.audiobookshelf_browse[0].episodes = Some(vec![AudiobookshelfDownloadedEpisode {
            library_item_id: "show-2".into(),
            episode_id: "wrong-episode".into(),
            title: "Wrong episode".into(),
            published_at: None,
            duration_seconds: None,
        }]);
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Podcast {
                library_item_id: "show-1".into(),
            },
            SelectionModalListState::Loading,
            podcast_filter(),
        );
        model
            .app
            .handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
                generation: Default::default(),
                library_item_id: "show-1".into(),
                result: Ok(vec![AudiobookshelfDownloadedEpisode {
                    library_item_id: "show-1".into(),
                    episode_id: "event-episode".into(),
                    title: "Correct episode".into(),
                    published_at: None,
                    duration_seconds: Some(60.0),
                }]),
            });
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), Some("event-episode"));
    }

    #[test]
    fn audiobookshelf_book_detail_success_refreshes_matching_open_modal() {
        let mut model = Model::new(book_app("book-1"));
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Book {
                book_id: "book-1".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        model
            .app
            .handle_lib_event(LibEvent::AudiobookshelfBookDetailFetched {
                generation: Default::default(),
                library_item_id: "book-1".into(),
                result: Ok((
                    vec![AudiobookshelfChapter {
                        id: 1,
                        start: 0.0,
                        end: 60.0,
                        title: "Chapter 1".into(),
                    }],
                    Vec::new(),
                )),
            });
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), Some("chapter:1"));
    }

    #[test]
    fn audiobookshelf_podcast_detail_error_ends_matching_modal_loading() {
        let mut model = Model::new(podcast_app(vec![show("show-1")], "show-1"));
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Podcast {
                library_item_id: "show-1".into(),
            },
            SelectionModalListState::Loading,
            podcast_filter(),
        );
        model
            .app
            .handle_lib_event(LibEvent::AudiobookshelfDetailFetched {
                generation: Default::default(),
                library_item_id: "show-1".into(),
                result: Err(AudiobookshelfError {
                    class: AudiobookshelfFailureClass::Connectivity,
                }),
            });

        assert!(matches!(
            model.app.pending_overlay.as_ref(),
            Some(OverlayRequest::RefreshSelectionModal {
                state: SelectionModalListState::Empty,
                ..
            })
        ));
        model.sync_modal_requests();
        assert!(!model.app.audiobookshelf_browse[0].detail_loading);
    }

    #[test]
    fn audiobookshelf_book_detail_error_ends_matching_modal_loading() {
        let mut model = Model::new(book_app("book-1"));
        mount_selection_modal(
            &mut model,
            SelectionModalSource::Book {
                book_id: "book-1".into(),
            },
            SelectionModalListState::Loading,
            None,
        );
        model
            .app
            .handle_lib_event(LibEvent::AudiobookshelfBookDetailFetched {
                generation: Default::default(),
                library_item_id: "book-1".into(),
                result: Err(AudiobookshelfError {
                    class: AudiobookshelfFailureClass::Connectivity,
                }),
            });

        assert!(matches!(
            model.app.pending_overlay.as_ref(),
            Some(OverlayRequest::RefreshSelectionModal {
                state: SelectionModalListState::Empty,
                ..
            })
        ));
        model.sync_modal_requests();
        assert!(model.app.audiobookshelf_book_browse[0]
            .detail_loading_ids
            .is_empty());
    }

    #[test]
    fn audiobookshelf_progress_refreshes_matching_podcast_modal() {
        let mut model = Model::new(super::super::tests_podcast::audiobookshelf_app());
        // Mount/project the podcast component so the modal action reads/writes
        // the episode filter through it (task 5.3d.11 U3).
        model.sync_audiobookshelf_podcast();
        model.open_podcast_selection_modal();
        model.sync_modal_requests();
        let update = mbv_core::player::AudiobookshelfProgressUpdate {
            generation: model.app.audiobookshelf_runtime.generation(),
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 60.0,
            duration_seconds: 60.0,
            is_finished: true,
        };
        model
            .app
            .handle_lib_event(LibEvent::AudiobookshelfProgressAcknowledged(update));
        model.sync_modal_requests();

        assert_eq!(selection_modal(&model).selected_id(), Some("episode-a"));
    }
}
