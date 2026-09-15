use super::test_helpers::*;
use super::*;
use crate::app::components::emby_library_content::EmbyLibraryContent as BrowserOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::ComponentId;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, TabSelection};

/// Seed the migrated Movies/HomeVideos/Generic owner's authoritative
/// selection directly (mirrors `set_browser_cursor_for_test`'s old
/// the former BrowserComponent's contract, now against the embedded owner).
fn set_home_video_cursor_for_test(model: &mut crate::app::shell::Model, cursor: usize) {
    model.sync_mounted_surfaces();
    let (_, key, _) = model
        .active_emby_library_owner()
        .expect("a migrated browser owner is active");
    model
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.owner_mut(&key))
        .and_then(|owner| owner.as_any_mut().downcast_mut::<BrowserOwner>())
        .expect("browser owner installed")
        .set_cursor_for_test(cursor);
}

#[test]
fn home_video_library_is_never_album_folders_and_renders_via_original_list_path() {
    let mut model = mounted_model_at(make_home_video_app(), 60, 20);
    let lib_idx = 0;

    assert!(
        !model.app.is_viewing_album_folders(lib_idx),
        "a homevideos library must never satisfy is_viewing_album_folders"
    );
    assert!(model.app.is_home_video_view(lib_idx));

    let out = draw_mounted_frame(&mut model, 60, 20);

    assert!(
        out.contains("Birthday Clip"),
        "expected the embedded EmbyLibraryContent owner to paint the home-video list:\n{out}"
    );
    assert!(
        model.app.album_tracks_cache.is_empty(),
        "home-video rendering must never touch the album-tracks cache added by #145"
    );
}

#[test]
fn narrow_home_video_selected_item_retains_inline_detail() {
    let mut app = make_home_video_app();
    app.libs[0].nav_stack[0].items[1].overview = "The selected home video overview.".into();
    app.libs[0].nav_stack[0].set_resting_cursor(1);
    let mut model = mounted_model_at(app, 70, 30);
    set_home_video_cursor_for_test(&mut model, 1);
    let output = draw_mounted_frame(&mut model, 70, 30);
    let layout = model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("Library panel type")
        .test_narrow_geometry()
        .expect("the panel painted a Narrow skeleton");

    let hero_area = layout.inline_hero.unwrap_or_default();
    assert!(
        hero_area.height > 0,
        "selected Home Video detail disappeared"
    );
    assert!(
        output.contains("Vacation Clip"),
        "selected Home Video title is missing:\n{output}"
    );
}

// wide_home_video_uses_a_left_detail_and_right_rail deleted (task 6.1):
// HomeVideos' Wide hero geometry moved to the embedded `EmbyLibraryContent`
// owner painted through the mounted `LibraryPanel`; the equivalent coverage
// now lives in `tests_library_characterization.rs` against the panel's
// `test_wide_geometry()`.

/// `remove-migrated-surface-underpaint` 3.2 (D4): at the wide Wide hero
/// breakpoint the embedded `EmbyLibraryContent` owner owns the Movies / home-video
/// picture. Post task 3.8 the legacy `render_library` `EmbyLibrary` arm only
/// reserves the destination `left_area` and paints no row, banner, or hero —
/// the `movies_wide_*` split geometry hand-off is now published by the
/// component itself. Mirrors the Home precedent
/// `legacy_base_frame_does_not_paint_home_content_before_the_component`.
#[test]
fn wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows() {
    for (mut app, marker) in [
        (make_movie_app(), "Focused Movie"),
        (make_home_video_app(), "Birthday Clip"),
    ] {
        let mut layout = Rect::default();
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        term.draw(|f| {
            app.reserve_library_area(
                f,
                ratatui::layout::Rect::new(0, 0, 120, 40),
                &mut layout,
                None,
            );
        })
        .unwrap();

        assert!(
            layout.width > 0 && layout.height > 0,
            "wide movies destination area hand-off must still be reserved: {:?}",
            layout
        );
        let output = buffer_to_string(&term);
        assert!(
            !output.contains(marker),
            "legacy base frame must not paint browser rows at the wide breakpoint: {output:?}"
        );
    }
}

#[test]
fn letter_filter_buckets_match_emby_name_range_bounds() {
    let ac = LetterFilter::for_index(0).unwrap();
    assert_eq!(ac.label, "A\u{2013}C");
    assert_eq!(ac.name_ge, Some("A"));
    assert_eq!(ac.name_lt, Some("D"));

    let vz = LetterFilter::for_index(7).unwrap();
    assert_eq!(vz.label, "V\u{2013}Z");
    assert_eq!(vz.name_ge, Some("V"));
    assert_eq!(vz.name_lt, None, "V–Z has no upper bound");

    let hash = LetterFilter::for_index(8).unwrap();
    assert_eq!(hash.label, "#");
    assert_eq!(hash.name_ge, None, "# has no lower bound");
    assert_eq!(hash.name_lt, Some("A"));

    assert!(LetterFilter::for_index(9).is_none());
    assert_eq!(LetterFilter::count(), 9);
    assert_eq!(LetterFilter::labels().len(), 9);
}

#[test]
fn letter_filter_default_is_the_first_bucket() {
    assert_eq!(
        LetterFilter::default_filter(),
        LetterFilter::for_index(0).unwrap()
    );
}

/// Characterization test for the narrow (single-column) Series inline hero
/// (task 2.1/2.2): renders hero content only (title/meta/overview/image) --
/// no "Series:" season pill/count row, no episode table. The wide
/// (Wide hero) presentation is a non-goal here; see `tv_wide_tests.rs`
/// for its unchanged coverage.
#[test]
fn narrow_series_inline_hero_shows_only_hero_content_no_season_or_episode_list() {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "library".into();
    library.collection_type = "tvshows".into();
    library.is_folder = true;

    let mut series = make_item("The Series", "Series");
    series.id = "series".into();
    series.overview = "An overview of the series.".into();

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    season.index_number = 1;
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode".into();
    episode.index_number = 1;
    episode.runtime_ticks = 3600 * mbv_core::api::TICKS_PER_SECOND;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "library".into(),
            title: "Shows".into(),
            items: vec![series],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: Some("Series".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        library_total: Some(1),
        ..LibraryTab::new(library)
    });
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    app.series_detail_cache.insert(
        "series".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes,
        },
    );

    // Below `TWO_COLUMN_THRESHOLD` so the narrow single-column presentation
    // renders instead of `render_wide_tv`. Painted by the embedded
    // content owner (task 3.8).
    let mut model = mounted_model_at(app, 70, 30);
    let output = draw_mounted_frame(&mut model, 70, 30);

    assert!(output.contains("The Series"), "{output}");
    assert!(output.contains("An overview"), "{output}");
    assert!(
        !output.contains("Series:"),
        "narrow inline hero must not show the season pill/count row:\n{output}"
    );
    assert!(
        !output.contains("Pilot"),
        "narrow inline hero must not show the episode table:\n{output}"
    );
}
