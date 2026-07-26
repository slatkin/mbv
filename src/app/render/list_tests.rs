use super::*;
use crate::app::layout::LayoutMain;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{AlbumIndexState, BrowseLevel, LibSearch, LibraryTab, SeriesDetail};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::collections::HashMap;

fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn render_power_list_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
    let backend = TestBackend::new(60, 8);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_list(f, Rect::new(0, 0, 60, 8), true, layout);
    })
    .unwrap();
    buffer_to_string(&term)
}

fn render_power_list_to_string_sized(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> String {
    buffer_to_string(&render_power_list_to_terminal_sized(
        app, layout, width, height,
    ))
}

fn render_power_list_to_terminal_sized(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_list(f, Rect::new(0, 0, width, height), true, layout);
    })
    .unwrap();
    term
}

fn make_power_movie_list_app(titles: Vec<&str>) -> App {
    let mut app = make_app_stub();
    app.library_tab = 1;

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let items: Vec<_> = titles
        .into_iter()
        .enumerate()
        .map(|(i, title)| {
            let mut m = make_item(title, "Movie");
            m.id = format!("movie-{i}");
            if title.contains("Selected") {
                m.overview = "This is the compact movie banner overview text.".into();
            }
            m
        })
        .collect();
    let total = items.len();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items,
            total_count: total,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

// #287: nearby movies' poster images should be prefetched before the
// cursor reaches them, mirroring render_power_card's existing home-card
// -carousel prefetch window (PREFETCH_AHEAD = 3 / PREFETCH_BEHIND = 1).
#[test]
fn compact_banner_prefetches_nearby_movies_but_not_beyond_the_window() {
    // 6 movies, cursor on item 0. PREFETCH_AHEAD = 3 / PREFETCH_BEHIND = 1
    // means the window covers indices 0..=3 (cursor has no items behind
    // it here, so PREFETCH_BEHIND has nothing to reach). Item 0 itself is
    // excluded from the prefetch loop (it's covered by its own eager
    // fetch), so movies 1-3 should be prefetched and movie 4 should not.
    let titles: Vec<&str> = vec![
        "Movie 0", "Movie 1", "Movie 2", "Movie 3", "Movie 4", "Movie 5",
    ];
    let mut app = make_power_movie_list_app(titles);
    app.image_protocol_enabled = true;

    let mut layout = LayoutMain::default();
    let _ = render_power_list_to_string(&mut app, &mut layout);

    let fetch_triggered = |app: &App, key: &str| {
        app.card_image_loading.contains(key) || app.card_image_states.contains_key(key)
    };

    // The currently-selected item's own eager fetch (unchanged existing
    // behavior, from compact_banner_layout).
    let selected_key = compact_banner_image_cache_key("movie-0");
    assert!(
        fetch_triggered(&app, &selected_key),
        "expected the selected movie's own image fetch to still be triggered"
    );

    // Prefetch window: movies 1-3 (within PREFETCH_AHEAD = 3) should be
    // prefetched.
    for i in 1..=3 {
        let key = compact_banner_image_cache_key(&format!("movie-{i}"));
        assert!(
            fetch_triggered(&app, &key),
            "expected movie-{i} to be prefetched (within the prefetch window)"
        );
    }

    // Movie 4 sits just outside the PREFETCH_AHEAD = 3 window and should
    // not be prefetched.
    let outside_key = compact_banner_image_cache_key("movie-4");
    assert!(
        !fetch_triggered(&app, &outside_key),
        "movie-4 is outside the prefetch window and should not have been fetched"
    );
}

#[test]
fn recursive_album_search_loading_message_is_explicit() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.music_levels = vec!["group".into(), "album".into()];
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "music-lib".into();
    library.collection_type = "music".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: Some(LibSearch {
            query: "record".into(),
            items: Vec::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: true,
        }),
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );

    let out = render_power_list_to_string(&mut app, &mut LayoutMain::default());

    assert!(out.contains("Indexing music library..."), "{out}");

    app.music_levels.clear();
    let out = render_power_list_to_string(&mut app, &mut LayoutMain::default());
    assert!(!out.contains("Indexing music library..."), "{out}");
}

#[test]
fn compact_banner_appears_inline_in_letter_grouped_movie_list() {
    // 60 movies triggers use_letter_groups (total_count >= 50, collection_type
    // != "music"). Titles are spread across many starting letters (cycling
    // A..Z) so the selected item's letter bucket is followed by several more
    // buckets -- this is what exercises the riskiest part of the interleaving
    // logic: filler rows must land between the selected item and the NEXT
    // bucket's header, not get scattered or appended after the whole list.
    let titles: Vec<String> = (0..60)
        .map(|i| {
            let letter = (b'A' + (i % 26) as u8) as char;
            format!("{letter} Movie {i:02}")
        })
        .collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let mut app = make_power_movie_list_app(title_refs);

    // Select an early-alphabet item (letter 'K') so later letter buckets --
    // e.g. the 'Z' item -- must sort, and therefore render, after it.
    let selected_idx = 10; // letter (b'A' + 10) as char == 'K'
    {
        let lvl = app.libs[0].nav_stack.last_mut().unwrap();
        lvl.items[selected_idx].overview = "This is the compact movie banner overview text.".into();
        lvl.cursor = selected_idx;
    }
    let selected_title = titles[selected_idx].clone();
    let later_title = titles[25].clone(); // letter (b'A' + 25) as char == 'Z'

    let mut layout = LayoutMain::default();
    let out = render_power_list_to_string_sized(&mut app, &mut layout, 60, 60);

    let selected_pos = out
        .find(selected_title.as_str())
        .expect("selected item's row should render");
    let banner_pos = out
        .find("compact movie banner")
        .expect("expected banner overview text to appear in letter-grouped list render");
    assert!(
        selected_pos < banner_pos,
        "banner should render after the selected row, not before it:\n{out}"
    );
    if let Some(later_pos) = out.find(later_title.as_str()) {
        assert!(
            banner_pos < later_pos,
            "banner must land inline between the selected item and later alphabet \
             buckets, not scattered after the whole list:\n{out}"
        );
    }
}

// Regression test for a bug where scrolling back to the very top of a
// letter-grouped library list could strand the first letter header (and
// the compact movie banner's top padding) just out of view, even though
// the cursor was genuinely on item 0. When the first item is itself a
// leaf movie (banner-eligible) that also starts the first letter
// bucket, `push_selected_detail_fillers_before` inserts 2 BannerFiller
// rows immediately before that item's row, so its display row lands at
// index 3, not 0. A stale `stored_scroll` >= 3 (carried over from
// scrolling down and back up) used to clamp to offset 3, and the old
// fixed-offset recovery checks didn't cover this row pattern
// ([LetterHeader, BannerFiller, BannerFiller, Item]), so the header and
// banner padding stayed permanently hidden above the viewport.
#[test]
fn scrolling_to_top_reveals_letter_header_when_first_item_has_a_banner() {
    // "{letter}Z" (not just "{letter}") prefix avoids `strip_article`
    // treating the first item's title as starting with the English
    // article "A ", which would silently re-bucket it under "M-O"
    // (from "Movie...").
    let titles: Vec<String> = (0..60)
        .map(|i| {
            let letter = (b'A' + (i % 26) as u8) as char;
            format!("{letter}Z Movie {i:02}")
        })
        .collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let mut app = make_power_movie_list_app(title_refs);

    // Item 0 ("AZ Movie 00") sorts first alphabetically, so it starts
    // the first letter bucket. Select it and give it an overview so it
    // also reserves compact-banner filler rows -- the combination that
    // triggers the bug.
    {
        let lvl = app.libs[0].nav_stack.last_mut().unwrap();
        lvl.items[0].overview = "This is the compact movie banner overview text.".into();
        lvl.cursor = 0;
        // Simulate a stale scroll offset carried over from scrolling
        // down earlier and back up -- >= 3 is enough to land past the
        // header and banner filler rows pre-fix.
        lvl.scroll = 6;
    }

    let mut layout = LayoutMain::default();
    let out = render_power_list_to_string(&mut app, &mut layout);

    let final_scroll = app.libs[0].nav_stack.last().unwrap().scroll;
    assert_eq!(
        final_scroll, 0,
        "scrolling to the top item should land at display offset 0 so \
         the letter header and banner padding stay visible, not \
         stranded above the viewport:\n{out}"
    );

    let header_pos = out.find("A\u{2013}C").expect("letter header should render");
    let banner_pos = out
        .find("compact movie banner")
        .expect("banner overview text should render");
    assert!(
        header_pos < banner_pos,
        "letter header must appear before the banner content in the \
         rendered output:\n{out}"
    );
}

// Letter-range pills (large libraries): a scoped "A–C" fetch is a small
// slice (well under the 50-item in-list header threshold), but should
// still show headers -- gated on the true `library_total`, not the
// filtered slice's own count (plan §6) -- and those headers should be
// individual letters (A, B, C) within the range, not a single "A–C"
// range header re-derived from the small slice.
#[test]
fn active_letter_filter_forces_per_letter_headers_even_for_a_small_slice() {
    let titles = vec!["Apple Movie", "Banana Movie", "Cherry Movie"];
    let mut app = make_power_movie_list_app(titles);
    // Simulate a scoped A–C fetch: the level's own total_count is small
    // (3, the size of the filtered slice), but the library's true total
    // is large -- exactly the state a selected letter pill produces.
    app.libs[0].library_total = Some(1000);
    {
        let lvl = app.libs[0].nav_stack.last_mut().unwrap();
        lvl.letter_filter = super::super::LetterFilter::for_index(0); // "A–C"
    }

    let mut layout = LayoutMain::default();
    let out = render_power_list_to_string_sized(&mut app, &mut layout, 60, 20);
    let trimmed_lines: Vec<&str> = out.lines().map(str::trim).collect();

    for letter in ["A", "B", "C"] {
        assert!(
            trimmed_lines.contains(&letter),
            "expected a standalone '{letter}' header row within the A–C range:\n{out}"
        );
    }
    assert!(
        !trimmed_lines.contains(&"A\u{2013}C"),
        "a small filtered slice must not fall back to a single range header:\n{out}"
    );
}

// #263: the banner's reserved row budget must track the selected movie's
// actual overview length, not a fixed constant -- a longer overview
// should reserve more rows (and so push the rest of the list down
// further) than a shorter one.
#[test]
fn compact_banner_rows_grows_with_a_longer_overview() {
    let mut app = make_power_movie_list_app(vec!["First", "Second Selected", "Third"]);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    let panel_width = 40u16
        .saturating_sub(1)
        .saturating_sub(COMPACT_BANNER_INDENT);

    app.libs[0].nav_stack.last_mut().unwrap().items[1].overview = "Short.".into();
    let short_rows = app.compact_banner_rows(0, panel_width);

    app.libs[0].nav_stack.last_mut().unwrap().items[1].overview = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. ".repeat(6);
    let long_rows = app.compact_banner_rows(0, panel_width);

    assert!(
        long_rows > short_rows,
        "long overview ({long_rows} rows) should reserve more rows than short overview ({short_rows} rows)"
    );
}

// Regression test for a bug where the plain (non letter-grouped) list
// branch called `render_selected_block_borders` unconditionally instead
// of gating it on `banner_rows > 0` like the letter-grouped branch does.
// With no movie banner, `banner_rule_top`/`banner_rule_bottom` collapse
// to near-zero, so it painted a stray full-width `▔` row right where the
// series inline detail's title/metadata should be.
#[test]
fn series_inline_detail_has_no_stray_banner_border_in_plain_list_branch() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.is_folder = true;
    library.collection_type = "tvshows".into();

    let mut show = make_item("Test Show", "Series");
    show.id = "series-1".into();
    show.series_name = "Test Show".into();
    show.production_year = 2020;
    show.end_year = 2022;
    show.genre = "drama".into();
    show.overview = "A short overview.".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items: vec![show],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    season.index_number = 1;
    let episodes: Vec<_> = (1..=8)
        .map(|i| {
            let mut ep = make_item(&format!("Episode {i}"), "Episode");
            ep.id = format!("episode-{i}");
            ep.index_number = i;
            ep.runtime_ticks = 23 * 60 * TICKS_PER_SECOND;
            ep
        })
        .collect();
    let active_episode = episodes[1].clone();
    app.series_detail_cache.insert(
        "series-1".into(),
        SeriesDetail {
            seasons: vec![season],
            episodes: HashMap::from([("season-1".into(), episodes)]),
        },
    );

    let mut layout = LayoutMain::default();
    let inactive = render_power_list_to_string_sized(&mut app, &mut layout, 60, 40);
    assert!(
        inactive.contains("Series: 1"),
        "inactive series detail should show season count:\n{inactive}"
    );
    assert!(
        !inactive.contains("Episode 1"),
        "inactive series detail should hide episodes:\n{inactive}"
    );

    app.libs[0].series_selection = Some(0);
    app.player_tab.set_items(vec![active_episode], 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.paused = false;
    }
    let term = render_power_list_to_terminal_sized(&mut app, &mut layout, 60, 40);
    let out = buffer_to_string(&term);

    let title_pos = out.find("Test Show  ").or_else(|| out.find("Test Show\n"));
    let meta_pos = out.find("2020-2022  DRAMA");
    let title_pos = title_pos.expect("series title should render");
    let meta_pos = meta_pos.expect("year/genre metadata should render");
    let between = &out[title_pos..meta_pos];
    assert!(
        !between.contains('\u{2594}'),
        "no stray banner-border glyph should appear between the series title \
         and its year/genre metadata row:\n{out}"
    );

    let lines: Vec<&str> = out.lines().collect();
    let selected_row = lines
        .iter()
        .position(|line| line.contains("Test Show"))
        .expect("selected series row should render");
    assert!(
        selected_row >= 2,
        "selected row should have room for top border and spacer:\n{out}"
    );
    assert!(
        lines[selected_row - 2].contains('\u{2581}'),
        "top border should be two rows above the selected series title:\n{out}"
    );
    assert!(
        lines[selected_row - 1].trim().is_empty(),
        "one spacer row should sit between top border and selected title:\n{out}"
    );
    assert!(
        lines[selected_row + 1].contains("2020-2022  DRAMA"),
        "metadata should render directly below the selected title:\n{out}"
    );

    let active_episode_line = lines
        .iter()
        .find(|line| line.contains("Episode 2"))
        .copied()
        .expect("active episode row should render");
    let icon = crate::app::render::play_icon(app.use_nerd_fonts);
    assert!(
        active_episode_line.contains(&format!("2. {icon} Episode 2")),
        "expected the active episode icon and following space after its number:\n{out}"
    );
    let icon_byte_x = active_episode_line
        .find(icon)
        .expect("expected active episode icon");
    let icon_x = active_episode_line[..icon_byte_x].chars().count() as u16;
    let title_byte_x = active_episode_line
        .find("Episode 2")
        .expect("expected active episode title");
    let title_x = active_episode_line[..title_byte_x].chars().count() as u16;
    let active_episode_y = lines
        .iter()
        .position(|line| line.contains("Episode 2"))
        .unwrap() as u16;
    let buffer = term.backend().buffer();
    assert_eq!(
        buffer[(icon_x, active_episode_y)].fg,
        super::super::palette::AQUA
    );
    assert_eq!(
        buffer[(title_x, active_episode_y)].fg,
        super::super::palette::WHITE
    );

    let last_episode_row = lines
        .iter()
        .position(|line| line.contains("8. Episode 8"))
        .expect("last visible episode row should render");
    assert!(
        lines[last_episode_row + 1].trim().is_empty(),
        "one spacer row should sit below the episode list:\n{out}"
    );
    assert!(
        lines[last_episode_row + 2].contains('\u{2594}'),
        "bottom border should follow exactly one spacer row after episodes:\n{out}"
    );
}

#[test]
fn letter_grouped_series_detail_keeps_headers_and_episode_borders() {
    let mut app = make_app_stub();
    app.library_tab = 1;

    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.is_folder = true;
    library.collection_type = "tvshows".into();

    let items: Vec<_> = (0..60)
        .map(|i| {
            let letter = (b'A' + (i % 26) as u8) as char;
            let name = if i == 0 {
                "Alpha Series 00".to_string()
            } else {
                format!("{letter} Series {i:02}")
            };
            let mut series = make_item(&name, "Series");
            series.id = format!("series-{i}");
            if i == 0 {
                series.overview = "Letter-grouped selected series overview.".into();
            }
            series
        })
        .collect();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items,
            total_count: 60,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: Some(0),
        series_season_cursor: 0,
        library_total: Some(250),
    });

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    season.index_number = 1;
    let episodes: Vec<_> = (1..=8)
        .map(|i| {
            let mut ep = make_item(&format!("Letter Episode {i}"), "Episode");
            ep.id = format!("episode-{i}");
            ep.index_number = i;
            ep
        })
        .collect();
    app.series_detail_cache.insert(
        "series-0".into(),
        SeriesDetail {
            seasons: vec![season],
            episodes: HashMap::from([("season-1".into(), episodes)]),
        },
    );

    let mut layout = LayoutMain::default();
    let out = render_power_list_to_string_sized(&mut app, &mut layout, 60, 200);
    let lines: Vec<&str> = out.lines().collect();
    let header = lines
        .iter()
        .position(|line| line.trim() == "A")
        .unwrap_or_else(|| panic!("letter-grouped series header should render:\n{out}"));
    let title = lines
        .iter()
        .position(|line| line.contains("Alpha Series 00"))
        .expect("selected series title should render");
    assert!(
        header < title,
        "header should precede selected series:\n{out}"
    );
    assert!(
        out.contains("Letter-grouped selected series overview."),
        "selected series detail should render:\n{out}"
    );

    let episode = lines
        .iter()
        .position(|line| line.contains("8. Letter Episode 8"))
        .expect("last episode should render");
    assert!(
        lines[episode + 1].trim().is_empty(),
        "episode list should retain its trailing spacer:\n{out}"
    );
    assert!(
        lines[episode + 2].contains('\u{2594}'),
        "series detail should retain its bottom border:\n{out}"
    );
    assert!(
        layout.left_row_map.iter().any(Option::is_none),
        "letter headers and detail filler rows should remain non-selectable"
    );
}
