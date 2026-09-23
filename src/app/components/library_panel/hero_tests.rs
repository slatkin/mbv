use super::*;

use crate::app::images::series_image_cache_key;
use mbv_core::audiobookshelf::AudiobookshelfBook;
use rstest::rstest;
use serde_json::json;

// Policy table (task 5.4): parsed `EmbyItem`s from recorded item JSON
// (the task 5.3 fixtures), ABS and feed content types as they exist.

fn emby_item(raw: serde_json::Value) -> EmbyItem {
    mbv_core::api::parse_item(&raw)
}

fn shape(artwork: &HeroArtwork) -> ArtworkShape {
    artwork.shape
}

fn source(artwork: &HeroArtwork) -> &ArtworkSource {
    artwork.source.as_ref().expect("expected a source")
}

#[test]
fn movie_with_backdrop_and_poster_is_landscape() {
    let item = emby_item(json!({
        "Id": "m1", "Name": "Dune", "Type": "Movie",
        "ImageTags": { "Primary": "poster", "Logo": "logo" },
        "BackdropImageTags": ["backdrop-a"], "UserData": {}
    }));
    let artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Landscape);
    match source(&artwork) {
        ArtworkSource::Emby {
            item_id,
            series_id,
            image_types,
            cache_key,
        } => {
            assert_eq!(item_id, "m1");
            assert_eq!(series_id, "");
            assert_eq!(image_types, &["Backdrop", "Primary"]);
            assert_eq!(cache_key, "m1:Backdrop,Primary");
        }
        _ => panic!("expected Emby source"),
    }
    assert_eq!(
        artwork.decoration,
        Some(ArtworkSource::Emby {
            item_id: "m1".into(),
            series_id: String::new(),
            image_types: vec!["Logo".into()],
            cache_key: "m1:Logo:logo".into(),
        })
    );
}

#[test]
fn movie_with_poster_only_is_portrait() {
    let item = emby_item(json!({
        "Id": "m2", "Name": "Poster only", "Type": "Movie",
        "ImageTags": { "Primary": "poster", "Logo": "logo" }, "UserData": {}
    }));
    let artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Portrait);
    assert_eq!(
        source(&artwork),
        &ArtworkSource::Emby {
            item_id: "m2".into(),
            series_id: String::new(),
            image_types: vec!["Primary".into(), "Backdrop".into()],
            cache_key: "m2:Primary,Backdrop".into(),
        }
    );
    assert!(artwork.decoration.is_none());
}

#[test]
fn logo_decoration_requires_a_movie_and_is_absent_when_undeclared() {
    let movie = emby_item(json!({
        "Id": "m-logo", "Type": "Movie",
        "ImageTags": { "Primary": "poster", "Logo": "logo" },
        "BackdropImageTags": ["backdrop"], "UserData": {}
    }));
    assert!(emby_artwork_policy(&movie).decoration.is_some());

    let movie_without_logo = emby_item(json!({
        "Id": "m-no-logo", "Type": "Movie",
        "ImageTags": { "Primary": "poster" }, "UserData": {}
    }));
    assert!(emby_artwork_policy(&movie_without_logo)
        .decoration
        .is_none());

    let landscape_movie_without_logo = emby_item(json!({
        "Id": "m-no-logo-landscape", "Type": "Movie",
        "ImageTags": { "Primary": "poster" },
        "BackdropImageTags": ["backdrop"], "UserData": {}
    }));
    assert!(emby_artwork_policy(&landscape_movie_without_logo)
        .decoration
        .is_none());

    let series = emby_item(json!({
        "Id": "s-logo", "Type": "Series",
        "ImageTags": { "Thumb": "thumb", "Logo": "logo" }, "UserData": {}
    }));
    assert!(emby_artwork_policy(&series).decoration.is_none());
}

#[test]
fn album_is_always_square() {
    // Music is Square regardless of what else is declared (an album with
    // a thumb included).
    let item = emby_item(json!({
        "Id": "al1", "Name": "Album", "Type": "MusicAlbum",
        "ImageTags": { "Thumb": "thumb", "Primary": "cover" }, "UserData": {}
    }));
    let artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Square);
    match source(&artwork) {
        ArtworkSource::Emby {
            image_types,
            cache_key,
            ..
        } => {
            assert_eq!(image_types, &["AudioChild"]);
            assert_eq!(cache_key, "al1:P");
        }
        _ => panic!("expected Emby source"),
    }
}

#[test]
fn album_folder_row_uses_the_album_arm_not_the_type_dispatch() {
    // Grouped Music's album rows in a folder-view music library arrive as
    // Emby `Folder` items (no `MusicAlbum`/audio type, no image tags), so
    // the type dispatch cannot recognise them: the Music owner asks for
    // the album arm, which is Square with the shared album chain under the
    // album's `{id}:P` key (the pre-panel music painters' contract).
    let item = emby_item(json!({
        "Id": "525079", "Name": "Aaliyah (2000) Try Again", "Type": "Folder",
        "IsFolder": true, "UserData": {}
    }));
    // The type dispatch alone yields no source at all.
    assert!(emby_artwork_policy(&item).source.is_none());

    let artwork = music_album_artwork(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Square);
    match source(&artwork) {
        ArtworkSource::Emby {
            item_id,
            image_types,
            cache_key,
            ..
        } => {
            assert_eq!(item_id, "525079");
            assert_eq!(image_types, &["AudioChild"]);
            assert_eq!(cache_key, "525079:P");
        }
        _ => panic!("expected Emby source"),
    }
}

#[test]
fn music_album_producer_overrides_the_type_dispatch_artwork() {
    let item = emby_item(json!({
        "Id": "525081", "Name": "Aaliyah (2002) I Care 4 U", "Type": "Folder",
        "IsFolder": true,
        "ImageTags": { "Primary": "cover" }, "UserData": {}
    }));
    let data = hero_content_music_album(&item);
    assert_eq!(shape(&data.facts.artwork), ArtworkShape::Square);
    // The type dispatch would have painted this folder Portrait under the
    // `{id}:Primary,Backdrop,Logo` key.
    assert_eq!(shape(&emby_artwork_policy(&item)), ArtworkShape::Portrait);
    match source(&data.facts.artwork) {
        ArtworkSource::Emby { cache_key, .. } => assert_eq!(cache_key, "525081:P"),
        _ => panic!("expected Emby source"),
    }
}

#[test]
fn episode_with_series_landscape_tags_is_landscape() {
    let item = emby_item(json!({
        "Id": "e1", "Name": "Pilot", "Type": "Episode", "SeriesName": "Lost",
        "SeriesId": "s1",
        "ParentThumbImageTag": "series-thumb", "UserData": {}
    }));
    let artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Landscape);
    match source(&artwork) {
        ArtworkSource::Emby {
            series_id,
            image_types,
            cache_key,
            ..
        } => {
            assert_eq!(series_id, "s1");
            assert_eq!(
                image_types,
                &SERIES_LANDSCAPE_IMAGE_TYPES
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            );
            // Episodes do not use the `:ser:` Series namespace.
            assert_eq!(cache_key, "e1:Thumb,Primary,Backdrop,Logo");
        }
        _ => panic!("expected Emby source"),
    }
}

#[test]
fn series_landscape_keeps_the_series_cache_key_namespace() {
    let item = emby_item(json!({
        "Id": "s1", "Name": "Lost", "Type": "Series", "UserData": {}
    }));
    // No tags declared: the Landscape fallback with no source.
    let artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Landscape);
    assert!(artwork.source.is_none());

    let item = emby_item(json!({
        "Id": "s1", "Name": "Lost", "Type": "Series",
        "ImageTags": { "Thumb": "thumb" }, "UserData": {}
    }));
    let artwork = emby_artwork_policy(&item);
    match source(&artwork) {
        ArtworkSource::Emby {
            image_types,
            cache_key,
            ..
        } => {
            assert_eq!(
                image_types,
                &SERIES_LANDSCAPE_IMAGE_TYPES
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                cache_key,
                &series_image_cache_key("s1", SERIES_LANDSCAPE_IMAGE_TYPES)
            );
        }
        _ => panic!("expected Emby source"),
    }
}

/// The painted-shape agreement (the user's side-by-side fix): a series whose
/// policy pins Landscape from a declared thumb but whose fetch chain resolves
/// the portrait `Primary` poster re-arms Portrait — the artwork that will
/// actually paint governs the layout. Loading/None keep the declared arm
/// stable before the image loads.
#[test]
fn landscape_declared_series_with_a_portrait_poster_painted_agrees_on_portrait() {
    let item = emby_item(json!({
        "Id": "s1", "Name": "Lost", "Type": "Series",
        "ImageTags": { "Thumb": "thumb", "Primary": "poster" }, "UserData": {}
    }));
    let mut artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Landscape);
    // Before the projection resolves, the declared arm stands.
    assert_eq!(artwork.painted_shape(), ArtworkShape::Landscape);
    // The chain's landscape image is unserveable and the fetch resolves the
    // poster: the decoded portrait aspect re-arms the painted shape.
    artwork.image = HeroImageState::Ready {
        cache_key: "k".into(),
        decoded: Some((200, 300)),
    };
    assert_eq!(artwork.painted_shape(), ArtworkShape::Portrait);
    // A genuinely 16:9 thumb keeps the Landscape arm.
    artwork.image = HeroImageState::Ready {
        cache_key: "k".into(),
        decoded: Some((1600, 900)),
    };
    assert_eq!(artwork.painted_shape(), ArtworkShape::Landscape);
    artwork.image = HeroImageState::Loading;
    assert_eq!(artwork.painted_shape(), ArtworkShape::Landscape);
}

#[test]
fn item_without_declared_artwork_falls_back_to_landscape_placeholder() {
    let item = emby_item(json!({ "Id": "v1", "Name": "Video", "Type": "Movie", "UserData": {} }));
    let artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Landscape);
    assert!(artwork.source.is_none());
}

/// A home-video (`Type: "Video"`) declared landscape only by its `Thumb`
/// must request that `Thumb`: the default landscape arm used to omit it and
/// fetch `Backdrop`/`Primary`/`Logo` only, so a `Thumb`-only item resolved
/// empty and painted the placeholder while its artwork existed.
#[test]
fn thumb_only_video_requests_the_declared_thumb() {
    let item = emby_item(json!({
        "Id": "hv1", "Name": "Bean Bag", "Type": "Video",
        "ImageTags": { "Thumb": "thumb" }, "UserData": {}
    }));
    let artwork = emby_artwork_policy(&item);
    assert_eq!(shape(&artwork), ArtworkShape::Landscape);
    match source(&artwork) {
        ArtworkSource::Emby {
            image_types,
            cache_key,
            ..
        } => {
            assert_eq!(image_types.first().map(String::as_str), Some("Thumb"));
            assert_eq!(cache_key, "hv1:Thumb,Backdrop,Primary,Logo");
        }
        _ => panic!("expected Emby source"),
    }
}

#[test]
fn podcast_episode_is_square() {
    let artwork = abs_episode_artwork_policy(&episode_item(Some("cover")));
    assert_eq!(shape(&artwork), ArtworkShape::Square);
    assert_eq!(
        artwork.source,
        Some(ArtworkSource::AudiobookshelfCover {
            library_item_id: "lib-1".into(),
            book: false,
        })
    );
    // No cover path: Square placeholder.
    let artwork = abs_episode_artwork_policy(&episode_item(None));
    assert_eq!(shape(&artwork), ArtworkShape::Square);
    assert!(artwork.source.is_none());
}

#[test]
fn book_is_portrait() {
    let book = book_item("book-1");
    let artwork = abs_book_artwork_policy(&book);
    assert_eq!(shape(&artwork), ArtworkShape::Portrait);
    assert_eq!(
        artwork.source,
        Some(ArtworkSource::AudiobookshelfCover {
            library_item_id: "book-1".into(),
            book: true,
        })
    );
    // No cover: Portrait placeholder.
    let mut no_cover = book_item("book-1");
    no_cover.cover_path = None;
    let artwork = abs_book_artwork_policy(&no_cover);
    assert_eq!(shape(&artwork), ArtworkShape::Portrait);
    assert!(artwork.source.is_none());
}

#[test]
fn feed_entry_is_landscape_placeholder_and_podcast_feed_square() {
    // Legacy entries carry no kind and default to the video arm.
    let artwork = feed_artwork_policy(&feed_entry(None));
    assert_eq!(shape(&artwork), ArtworkShape::Landscape);
    assert!(artwork.source.is_none());
    let artwork = feed_artwork_policy(&feed_entry(Some(mbv_core::config::FeedKind::Audio)));
    assert_eq!(shape(&artwork), ArtworkShape::Square);
    assert!(artwork.source.is_none());
}

// ── Producers ──────────────────────────────────────────────────────

#[test]
fn emby_producer_fills_plain_meta_rows_and_overview() {
    let item = emby_item(json!({
        "Id": "s1", "Name": "Lost", "Type": "Series",
        "ProductionYear": 2004, "EndDate": "2010-05-23", "Genres": ["Adventure"],
        "Overview": "<b>The</b> survivors", "UserData": {}
    }));
    let produced = hero_content_emby(&item);
    assert_eq!(produced.facts.title, "Lost");
    assert_eq!(produced.facts.meta_rows, vec!["2004-2010  ADVENTURE"]);
    // The overview is trimmed plain text (clean_overview strips URLs, not
    // markup).
    assert_eq!(produced.overview.as_deref(), Some("<b>The</b> survivors"));
}

#[test]
fn movie_meta_rows_append_links_then_genres_after_runtime() {
    let item = emby_item(json!({
        "Id": "m1", "Name": "Dune", "Type": "Movie",
        "PremiereDate": "2021-10-22", "RunTimeTicks": 7_200_000_000i64,
        "Genres": ["Action", "Drama"],
        "ExternalUrls": [
            {"Name": "IMDb", "Url": "https://imdb.test/dune"},
            {"Name": "TheMovieDb", "Url": "https://tmdb.test/dune"}
        ],
        "UserData": {}
    }));
    let data = hero_content_emby(&item);
    assert_eq!(
        data.facts.meta_rows,
        vec!["22 Oct 2021", "12:00", "IMDb", "Action/Drama"]
    );
    assert_eq!(
        data.facts.links,
        vec![HeroLink {
            name: "IMDb".into(),
            url: "https://imdb.test/dune".into()
        }]
    );
}

#[test]
fn movie_without_genres_or_links_keeps_only_existing_rows() {
    let item = emby_item(json!({
        "Id": "m1", "Name": "Dune", "Type": "Movie", "PremiereDate": "2021-10-22",
        "RunTimeTicks": 7_200_000_000i64, "UserData": {}
    }));
    let data = hero_content_emby(&item);
    assert_eq!(data.facts.meta_rows, vec!["22 Oct 2021", "12:00"]);
    assert!(data.facts.links.is_empty());
}

#[rstest]
#[case(vec![("Director", "", "A")], vec![("A", "Director")])]
#[case(vec![("Director", "", "Director"), ("Actor", "Actor 0", "Actor 0"), ("Actor", "Actor 1", "Actor 1"), ("Actor", "Actor 2", "Actor 2"), ("Actor", "Actor 3", "Actor 3"), ("Actor", "Actor 4", "Actor 4"), ("Actor", "Actor 5", "Actor 5"), ("Actor", "Actor 6", "Actor 6"), ("Actor", "Actor 7", "Actor 7"), ("Actor", "Actor 8", "Actor 8"), ("Actor", "Actor 9", "Actor 9"), ("Actor", "Actor 10", "Actor 10"), ("Actor", "Actor 11", "Actor 11"), ("Actor", "Actor 12", "Actor 12"), ("Actor", "Actor 13", "Actor 13")], vec![("Director", "Director"), ("Actor 0", "Actor 0"), ("Actor 1", "Actor 1"), ("Actor 2", "Actor 2"), ("Actor 3", "Actor 3"), ("Actor 4", "Actor 4"), ("Actor 5", "Actor 5"), ("Actor 6", "Actor 6"), ("Actor 7", "Actor 7"), ("Actor 8", "Actor 8"), ("Actor 9", "Actor 9"), ("Actor 10", "Actor 10"), ("Actor 11", "Actor 11"), ("Actor 12", "Actor 12"), ("Actor 13", "Actor 13")])]
#[case(vec![("Director", "", "D"), ("Actor", "A", "A"), ("Writer", "W", "W"), ("Producer", "P", "P"), ("Composer", "C", "C")], vec![("D", "Director"), ("A", "A"), ("W", "W"), ("P", "P"), ("C", "C")])]
#[case(vec![("Director", "", "A"), ("Director", "Dir", "B"), ("Actor", "C1", "C"), ("Actor", "C2", "D")], vec![("A", "Director"), ("B", "Dir"), ("C", "C1"), ("D", "C2")])]
#[case(vec![("Actor", "C1", "C")], vec![("C", "C1")])]
#[case(vec![("Director", "Dir", "A"), ("Actor", "C1", "B"), ("Actor", "C2", "C")], vec![("A", "Dir"), ("B", "C1"), ("C", "C2")])]
#[case(Vec::<(&str, &str, &str)>::new(), Vec::<(&str, &str)>::new())]
fn movie_credits_are_grouped_without_cap(
    #[case] people: Vec<(&str, &str, &str)>,
    #[case] expected: Vec<(&str, &str)>,
) {
    let people_json: Vec<_> = people
        .iter()
        .map(|(kind, role, name)| json!({"Name": name, "Role": role, "Type": kind}))
        .collect();
    let item = emby_item(
        json!({"Id": "m1", "Name": "Dune", "Type": "Movie", "People": people_json, "UserData": {}}),
    );
    let data = hero_content_emby(&item);
    let actual: Vec<_> = data
        .credits
        .unwrap_or_default()
        .into_iter()
        .map(|credit| (credit.name, credit.role))
        .collect::<Vec<(String, String)>>();
    let expected: Vec<_> = expected
        .into_iter()
        .map(|(name, role)| (name.to_string(), role.to_string()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn movie_credits_reach_library_panel_content_through_browser_owner() {
    use crate::app::components::emby_library_content::{BrowserOwnerPush, EmbyLibraryContent};
    use crate::app::components::library_panel::LibraryContentOwner;
    use crate::app::components::library_panel::LibraryKind;

    let movie = emby_item(json!({
        "Id": "m1", "Name": "Dune", "Type": "Movie",
        "People": [{"Name": "Denis Villeneuve", "Type": "Director"}],
        "UserData": {}
    }));
    let mut owner = EmbyLibraryContent::new(LibraryKind::Movies);
    owner.set_content(BrowserOwnerPush {
        items: vec![movie],
        latest_items: Vec::new(),
        total_count: 1,
        library_total: None,
        letter_filter: None,
        loading: false,
        group_pills: false,
        show_letter_pills: false,
        feed_groups: Vec::new(),
        feed_group_ids: Vec::new(),
        feed_group_cursor: 0,
    });

    let content = owner.content();
    let credits = content.hero.expect("movie hero").credits.expect("credits");
    assert_eq!(credits[0].name, "Denis Villeneuve");
    assert_eq!(credits[0].role, "Director");
}

#[test]
fn series_and_music_album_keep_their_existing_metadata_rows() {
    let series = emby_item(
        json!({"Name": "Lost", "Type": "Series", "ProductionYear": 2004, "Genres": ["Adventure"], "UserData": {}}),
    );
    assert_eq!(
        hero_content_emby(&series).facts.meta_rows,
        vec!["2004  ADVENTURE"]
    );
    let album = emby_item(
        json!({"Name": "Album", "Type": "MusicAlbum", "Genres": ["Rock"], "UserData": {}}),
    );
    assert!(hero_content_music_album(&album).facts.meta_rows.is_empty());
}

#[test]
fn queue_producer_dispatches_by_kind() {
    let book = book_item("book-9");
    let produced = hero_content_queue(&QueueItem::AudiobookshelfBook(book.clone()));
    assert_eq!(produced, hero_content_abs_book(&book));
}

/// The episode producer's facts are what the episode payload actually has
/// (design D7): title = episode title, meta rows = the parent show's name,
/// duration and publish date — no author row, and the overview is the
/// cleaned plain text.
#[test]
fn abs_episode_producer_rows_show_show_duration_and_publish_date() {
    let mut episode = episode_item(Some("cover"));
    episode.show_title = Some("The Show".into());
    episode.author = Some("Host".into());
    episode.duration_ticks = Some(1_800 * TICKS_PER_SECOND as u64);
    episode.pub_date_secs = Some(1_752_883_200); // 2025-07-19 UTC
    let produced = hero_content_abs_episode(&episode);
    assert_eq!(produced.facts.title, "Ep 1");
    assert_eq!(
        produced.facts.meta_rows,
        vec!["The Show", "30:00", "19 Jul 2025"]
    );
    // The overview is the cleaned plain text.
    assert_eq!(
        produced.overview.as_deref(),
        Some("<i>About</i> this episode".trim())
    );
    assert_eq!(produced.facts.artwork.shape, ArtworkShape::Square);
    // No credits block: the producer never invents one.
    assert_eq!(produced.credits, None);
}

/// Missing metadata collapses without inventing rows (design D7): no show
/// name, duration, or publish date leaves the hero with the title and
/// overview alone.
#[test]
fn abs_episode_producer_collapses_missing_metadata() {
    let mut episode = episode_item(None);
    episode.duration_ticks = None;
    let produced = hero_content_abs_episode(&episode);
    assert_eq!(produced.facts.title, "Ep 1");
    assert!(produced.facts.meta_rows.is_empty());
    assert!(produced.overview.is_some());
    // The parent show's cover is addressed by the show's provider-native
    // identity through the Square artwork policy; without a cover path the
    // placeholder stands in (images-disabled budgeting reads the same None
    // image state).
    assert_eq!(produced.facts.artwork.shape, ArtworkShape::Square);
    assert_eq!(produced.facts.artwork.source, None);
    // With a cover, the fetch is keyed by the parent show's library item id.
    let with_cover = hero_content_abs_episode(&episode_item(Some("cover")));
    assert_eq!(
        with_cover.facts.artwork.source,
        Some(ArtworkSource::AudiobookshelfCover {
            library_item_id: "lib-1".into(),
            book: false,
        })
    );
}

/// Images-disabled budgeting on the episode hero path (row 3.5), explicit
/// rather than implied by the missing-cover case: the episode's Square cover
/// source exists, but with the image protocol disabled the projection returns
/// `HeroImageState::None` — the placeholder is final and no cover fetch is
/// keyed by anything.
#[test]
fn images_disabled_budgets_no_cover_fetch_for_the_episode_hero() {
    use crate::app::tests::make_app_stub;

    let produced = hero_content_abs_episode(&episode_item(Some("cover")));
    assert!(produced.facts.artwork.source.is_some());
    assert_eq!(
        produced.facts.artwork.painted_shape(),
        ArtworkShape::Square,
        "the Square placeholder stands in"
    );

    let mut app = make_app_stub();
    app.image_protocol_enabled = false;
    let area = ratatui::layout::Rect::new(0, 0, 40, 12);
    let state = app.project_hero_image(&produced.facts, false, area, None, None);
    assert!(matches!(state, HeroImageState::None));
    assert!(app.card_image_loading.is_empty());
    assert!(
        app.card_image_states.is_empty(),
        "no cover fetch is keyed by anything"
    );
}

#[test]
fn feed_producer_rows_and_policy_shape() {
    let entry = feed_entry(Some(mbv_core::config::FeedKind::Video));
    let produced = hero_content_feed(&entry);
    assert_eq!(produced.facts.title, "Entry 1");
    assert!(produced.overview.is_none());
    assert_eq!(produced.facts.artwork.shape, ArtworkShape::Landscape);
    assert!(produced.facts.artwork.source.is_none());
}

/// The Home path and the Books tab produce identical `HeroContent` for
/// the same ABS book (task 5.4): the Books tab's selection resolves
/// through `audiobookshelf_book_queue_item` (its queue-item conversion),
/// and Home holds the same snapshot — both call the one ABS-book
/// producer, so their `HeroContentData` is equal.
#[test]
fn home_path_and_books_tab_produce_identical_content_for_one_book() {
    use crate::app::audiobookshelf_browse_actions::audiobookshelf_book_queue_item;
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
    use mbv_core::audiobookshelf::{
        AudiobookshelfAudioFile, AudiobookshelfBookProgress, AudiobookshelfLibrary,
    };

    let mut state = AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Books".into(),
        media_type: "book".into(),
    });
    state.books.push(catalog_book("book-9"));
    state.selected_id = Some("book-9".into());
    state.detail_cache.insert(
        "book-9".into(),
        (
            Vec::new(),
            vec![AudiobookshelfAudioFile {
                index: 0,
                ino: "ino".into(),
                duration: 3_600.0,
            }],
        ),
    );
    state.progress.insert(
        "book-9".into(),
        AudiobookshelfBookProgress {
            library_item_id: "book-9".into(),
            current_time_seconds: 1_800.0,
            is_finished: false,
        },
    );

    // Books-tab path: the tab's own selection conversion.
    let tab_item = audiobookshelf_book_queue_item(&state).expect("selected book");
    let books_tab = hero_content_queue(&tab_item);

    // Home path: the same book snapshot Home holds as a queue item.
    let home_item = book_item("book-9");
    let home = hero_content_queue(&QueueItem::AudiobookshelfBook(home_item));

    assert_eq!(books_tab, home);
    // And the content is the one producer's Portrait book facts.
    assert_eq!(books_tab.facts.title, "Book book-9");
    assert_eq!(
        books_tab.facts.meta_rows,
        vec!["Author book-9", "01:00:00", "50%"]
    );
    assert_eq!(books_tab.facts.duration_row, Some(1));
    assert_eq!(
        books_tab.facts.progress_row,
        Some(2),
        "the percentage row is flagged so the painter can colour it"
    );
    assert_eq!(books_tab.facts.artwork.shape, ArtworkShape::Portrait);
}

/// A finished book shows the `Finished` word instead of a percentage, so it
/// flags no progress row — the word must not paint in the progress colour.
#[test]
fn finished_book_flags_no_progress_row() {
    let mut book = book_item("book-9");
    book.is_finished = true;
    let content = hero_content_abs_book(&book);
    assert!(content.facts.meta_rows.iter().any(|row| row == "Finished"));
    assert_eq!(content.facts.progress_row, None);
}

// Fixtures.

/// The catalog book the Books tab holds (mirrors `book_item`'s fields so
/// the two paths describe the same book).
fn catalog_book(id: &str) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: format!("Book {id}"),
        author_display: Some(format!("Author {id}")),
        author_sort_key: format!("Author {id}"),
        cover_path: Some(format!("{id}-cover")),
        duration_seconds: 3_600.0,
        narrator: None,
        published_year: None,
        genres: Vec::new(),
        description: None,
        series_name: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

fn book_item(id: &str) -> AudiobookshelfBookQueueItem {
    AudiobookshelfBookQueueItem {
        library_item_id: id.into(),
        title: format!("Book {id}"),
        author: Some(format!("Author {id}")),
        duration_ticks: Some(3_600 * TICKS_PER_SECOND as u64),
        position_ticks: 1_800 * TICKS_PER_SECOND,
        played: false,
        is_finished: false,
        cover_path: Some(format!("{id}-cover")),
    }
}

fn episode_item(cover: Option<&str>) -> AudiobookshelfQueueItem {
    AudiobookshelfQueueItem {
        library_item_id: "lib-1".into(),
        episode_id: "ep-1".into(),
        title: "Ep 1".into(),
        show_title: None,
        author: None,
        description: Some("<i>About</i> this episode".into()),
        duration_ticks: Some(1_800 * TICKS_PER_SECOND as u64),
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: cover.map(|c| c.to_string()),
    }
}

fn feed_entry(kind: Option<mbv_core::config::FeedKind>) -> FeedEntry {
    FeedEntry {
        guid: "g".into(),
        title: "Entry 1".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: Some(600 * TICKS_PER_SECOND as u64),
        pub_date_secs: None,
        feed_kind: kind,
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}
