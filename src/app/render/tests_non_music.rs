use super::test_helpers::*;
use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, TabSelection};
use tuirealm::component::Component;

#[test]
fn home_video_library_is_never_album_folders_and_renders_via_original_list_path() {
    let mut app = make_home_video_app();
    let lib_idx = 0;

    assert!(
        !app.is_viewing_album_folders(lib_idx),
        "a homevideos library must never satisfy is_viewing_album_folders"
    );
    assert!(app.is_home_video_view(lib_idx));

    let mut layout = LayoutMain::default();
    let out = render_library_to_string(&mut app, &mut layout);

    assert!(
        out.contains("Birthday Clip"),
        "expected the original single-pane home-video list renderer to fire \
         unchanged:\n{out}"
    );
    assert!(
        app.album_tracks_cache.is_empty(),
        "home-video rendering must never touch the album-tracks cache added by #145"
    );
}

#[test]
fn narrow_home_video_selected_item_retains_inline_detail() {
    let mut app = make_home_video_app();
    app.libs[0].nav_stack[0].items[1].overview = "The selected home video overview.".into();
    app.libs[0].nav_stack[0].cursor = 1;
    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 70, 30);

    assert!(
        layout.hero_area.height > 0,
        "selected Home Video detail disappeared"
    );
    assert!(
        output.contains("Vacation Clip"),
        "selected Home Video title is missing:\n{output}"
    );
}

#[test]
fn wide_home_video_uses_a_left_detail_and_right_rail() {
    let mut app = make_home_video_app();
    let layout = render_view(&mut app, 200, 40);

    assert!(layout.movies_wide_right_area.width > 0);
    assert!(layout.movies_wide_right_area.height > 0);
}

/// `remove-migrated-surface-underpaint` 3.2 (D4): at the wide hero-on-left
/// breakpoint the mounted `BrowserComponent` owns the Movies / home-video
/// picture. `render_list` returns after publishing the `movies_wide_*`
/// geometry hand-off (`src/app/render/components/list.rs:98`) without
/// painting any row, banner, or hero. Mirrors the Home precedent
/// `legacy_base_frame_does_not_paint_home_content_before_the_component`.
#[test]
fn wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows() {
    for (mut app, marker) in [
        (make_movie_app(), "Focused Movie"),
        (make_home_video_app(), "Birthday Clip"),
    ] {
        let mut layout = LayoutMain::default();
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        term.draw(|f| {
            app.render_library(
                f,
                ratatui::layout::Rect::new(0, 0, 120, 40),
                true,
                &mut layout,
                None,
            );
        })
        .unwrap();

        assert!(
            layout.movies_wide_right_area.width > 0 && layout.movies_wide_right_area.height > 0,
            "wide movies geometry hand-off must still be reserved: {:?}",
            layout.movies_wide_right_area
        );
        let output = buffer_to_string(&term);
        assert!(
            !output.contains(marker),
            "legacy base frame must not paint browser rows at the wide breakpoint: {output:?}"
        );
    }
}

/// `remove-migrated-surface-underpaint` 3.8 (D4): while the mounted
/// `InlineSearchComponent` overlays `left_area`, the legacy `render_list`
/// must not underpaint the ordinary browse list. The shell projects
/// `App::inline_search_active` from TuiRealm mount state; `render_list`
/// returns after publishing `left_area` (`src/app/render/components/list.rs`,
/// just before the `n == 0` branch) without painting a row or hero. Mirrors
/// `wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows`.
#[test]
fn inline_search_active_legacy_base_frame_publishes_geometry_but_paints_no_rows() {
    let mut app = make_movie_app();
    app.inline_search_active = true;
    let mut layout = LayoutMain::default();
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(70, 30)).unwrap();
    term.draw(|f| {
        app.render_library(
            f,
            ratatui::layout::Rect::new(0, 0, 70, 30),
            true,
            &mut layout,
            None,
        );
    })
    .unwrap();

    assert!(
        layout.left_area.width > 0 && layout.left_area.height > 0,
        "left_area must still be reserved for the inline-search overlay: {:?}",
        layout.left_area
    );
    let output = buffer_to_string(&term);
    assert!(
        !output.contains("Focused Movie"),
        "legacy base frame must not paint browse rows while inline search is active: {output:?}"
    );
}

#[test]
fn wide_emby_podcast_uses_the_series_workspace_and_right_rail() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "podcasts".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.is_folder = true;
    }

    let layout = render_view(&mut app, 200, 40);

    assert!(layout.tv_wide_left_area.width > 0);
    assert!(layout.tv_wide_right_area.width > 0);
}

#[test]
fn podcast_and_home_video_use_inline_when_wide_height_is_unavailable() {
    let mut podcast = make_movie_app();
    podcast.libs[0].library.collection_type = "podcasts".into();
    let podcast_layout = render_view(&mut podcast, 200, 8);
    assert_eq!(podcast_layout.tv_wide_left_area.width, 0);

    let mut home_video = make_home_video_app();
    let home_video_layout = render_view(&mut home_video, 200, 8);
    assert_eq!(home_video_layout.movies_wide_right_area.width, 0);
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

#[test]
fn tv_series_list_computes_sorted_indices_when_above_threshold() {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.is_folder = true;
    library.collection_type = "tvshows".into();

    let series: Vec<_> = (0..55)
        .map(|i| {
            let letter = (b'A' + (i % 26) as u8) as char;
            let name = format!("{letter}alpha Series {i:02}");
            let mut s = make_item(&name, "Series");
            s.id = format!("series-{i}");
            s
        })
        .collect();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items: series,
            total_count: 55,
            cursor: 0,
            scroll: 0,
            item_types: Some("Series".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        library_total: Some(55),
        ..LibraryTab::new(library)
    });

    let mut layout = LayoutMain::default();
    let _ = render_library_to_terminal(&mut app, &mut layout);

    assert!(
        !layout.left_sorted_indices.is_empty(),
        "sorted indices should be computed for letter-grouped TV list"
    );
    // The first sorted index should map to the alphabetically-first A-series item
    let first_idx = layout.left_sorted_indices[0];
    assert!(
        app.libs[0].nav_stack[0].items[first_idx]
            .name
            .starts_with('A'),
        "first sorted item should start with A, got: {}",
        app.libs[0].nav_stack[0].items[first_idx].name,
    );

    let mut layout = LayoutMain::default();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 20)).unwrap();
    // Wide TV is component-owned (task 5.3d.18d): the App frame only
    // publishes the `tv_wide_*` hand-off geometry, and the mounted
    // `TvWorkspaceComponent` paints the surface pills over it, exactly as
    // the live shell does.
    let mut component = crate::app::components::TvWorkspaceComponent::new();
    component.set_content(app.wide_tv_render_ctx(0, true, None));
    let wide_area = ratatui::layout::Rect::new(0, 0, 120, 20);
    terminal
        .draw(|f| {
            app.render_library(f, wide_area, true, &mut layout, None);
            component.view(f, wide_area);
        })
        .unwrap();
    let component_layout = component.test_layout();
    assert_surface_pills(
        &terminal,
        component_layout,
        ratatui::layout::Rect {
            y: component_layout.selector_tabs[0].0.y,
            height: component_layout
                .tv_wide_right_area
                .bottom()
                .saturating_sub(component_layout.selector_tabs[0].0.y),
            ..component_layout.tv_wide_right_area
        },
        1,
        ratatui::style::Color::Reset,
        &(0..9).collect::<Vec<_>>(),
        &["⌘", "A–C", "D–F", "G–I", "J–L", "M–O", "P–R", "S–U", "V–Z"],
        0,
    );
}

/// Characterization test for the narrow (single-column) Series inline hero
/// (task 2.1/2.2): renders hero content only (title/meta/overview/image) --
/// no "Series:" season pill/count row, no episode table. The wide
/// (hero-on-left) presentation is a non-goal here; see `tv_wide_tests.rs`
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
            cursor: 0,
            scroll: 0,
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

    let mut layout = LayoutMain::default();
    // Below `TWO_COLUMN_THRESHOLD` so the narrow single-column presentation
    // renders instead of `render_wide_tv`.
    let output = render_library_to_string_sized(&mut app, &mut layout, 70, 30);

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
