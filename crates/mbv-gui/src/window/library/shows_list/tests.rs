//! Headless tests of the Shows list's row model (D16): the contracts of
//! `shows-list.test.ts` — the All series mode's Music tree over the TUI's TV
//! node kinds with the letter headings, the closed nodes, the flat modes'
//! date-gutter rows with no heading, the filter's auto-open, the active mode's
//! resolution, and the A–Z rail. The model is plain data, so the geometry is
//! asserted without a layout.

use super::*;
use crate::bridge::frame::library::{GuiHomeRowStateFrame, GuiShowsPillFrame};

/// One episode row of a season, as the frame carries it.
fn episode(id: &str, name: &str) -> GuiShowsEpisodeRowFrame {
    GuiShowsEpisodeRowFrame {
        episode_id: id.to_owned(),
        number: 1,
        name: name.to_owned(),
        duration: "24:00".to_owned(),
        state: GuiHomeRowStateFrame::Ordinary,
    }
}

/// One series node: its rail letter is the heading it paints under.
fn series(
    id: &str,
    title: &str,
    sort_letter: char,
    seasons: Vec<crate::bridge::frame::library::GuiShowsSeasonFrame>,
) -> GuiShowsSeriesFrame {
    GuiShowsSeriesFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        sort_letter,
        expandable: true,
        seasons,
    }
}

/// One flat row: the Latest mode's newest episode, or Upcoming's premiere.
fn flat(id: &str, title: &str, date: &str) -> GuiShowsRowFrame {
    GuiShowsRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some("S3E4 · Sabrosito".to_owned()),
        date: Some(date.to_owned()),
        sort_letter: None,
    }
}

/// The pills the frame carries, with `mode` selected.
fn pills(mode: GuiShowsModeFrame) -> Vec<GuiShowsPillFrame> {
    [
        (GuiShowsModeFrame::Latest, "New"),
        (GuiShowsModeFrame::Upcoming, "Upcoming"),
        (GuiShowsModeFrame::All, "All"),
    ]
    .into_iter()
    .map(|(pill_mode, name)| GuiShowsPillFrame {
        mode: pill_mode,
        name: name.to_owned(),
        selected: pill_mode == mode,
        collection: false,
    })
    .collect()
}

/// One season node, with its episodes or `None` while the load has not
/// landed.
fn season(
    id: &str,
    name: &str,
    episodes: Option<Vec<GuiShowsEpisodeRowFrame>>,
) -> crate::bridge::frame::library::GuiShowsSeasonFrame {
    crate::bridge::frame::library::GuiShowsSeasonFrame {
        id: id.to_owned(),
        name: name.to_owned(),
        episodes,
    }
}

/// The All series fixture: two `A` series (the first with a season and its
/// episodes), one `B` series, and one digit-led, which paints under `#`.
fn tree() -> Vec<GuiShowsSeriesFrame> {
    vec![
        series(
            "s1",
            "Atlanta",
            'A',
            vec![
                season(
                    "se1",
                    "Season 1",
                    Some(vec![episode("ep1", "The Big Bang")]),
                ),
                season("se2", "Season 2", None),
            ],
        ),
        series("s2", "The Bear", 'A', Vec::new()),
        series("s3", "Better Call Saul", 'B', Vec::new()),
        series("s4", "1899", '#', Vec::new()),
    ]
}

/// The flat newest-episode rows the Latest mode paints.
fn episodes() -> Vec<GuiShowsRowFrame> {
    vec![
        flat("e1", "Atlanta", "12 May"),
        flat("e2", "The Bear", "9 May"),
    ]
}

/// A frame over `tree` and `rows`, in `mode`.
fn shows_frame(
    mode: GuiShowsModeFrame,
    series: Vec<GuiShowsSeriesFrame>,
    rows: Vec<GuiShowsRowFrame>,
) -> GuiShowsList {
    GuiShowsList {
        lib_idx: 0,
        pills: pills(mode),
        collection_names: vec!["All".to_owned()],
        loading: false,
        series,
        upcoming_failed: false,
        rows,
    }
}

/// The painted row's kind, as the TypeScript test's `kind` names it.
fn kind_of(row: &ShowsListRow) -> &'static str {
    match row {
        ShowsListRow::Heading(_) => "heading",
        ShowsListRow::Series(_) => "series",
        ShowsListRow::Season(_) => "season",
        ShowsListRow::Episode(_) => "episode",
        ShowsListRow::Flat(_) => "flat",
    }
}

/// The row's heading letter, `None` on every other kind.
fn heading_letter(row: &ShowsListRow) -> Option<char> {
    match row {
        ShowsListRow::Heading(heading) => Some(heading.letter),
        _ => None,
    }
}

// The All series mode paints the tree at the Music heights.
#[test]
fn the_all_series_mode_paints_the_tree_at_the_music_heights() {
    let open = vec![series_row_id("s1"), season_row_id("s1", "se1")];
    let closed: Vec<String> = Vec::new();
    let painted = build_shows_rows(
        &shows_frame(GuiShowsModeFrame::All, tree(), episodes()),
        GuiShowsModeFrame::All,
        "",
        ShowsTreeState {
            open: &open,
            closed: &closed,
        },
    );

    let kinds: Vec<&str> = painted.iter().map(kind_of).collect();
    assert_eq!(
        kinds,
        [
            "heading", "series", "season", "episode", "season", "series", "heading", "series",
            "heading", "series",
        ]
    );
    let heights: Vec<f32> = painted.iter().map(GeometryRow::height).collect();
    assert_eq!(
        heights,
        [
            SHOWS_HEADING_ROW_HEIGHT,
            SHOWS_SERIES_ROW_HEIGHT,
            SHOWS_SEASON_ROW_HEIGHT,
            SHOWS_EPISODE_ROW_HEIGHT,
            SHOWS_SEASON_ROW_HEIGHT,
            SHOWS_SERIES_ROW_HEIGHT,
            SHOWS_HEADING_ROW_HEIGHT,
            SHOWS_SERIES_ROW_HEIGHT,
            SHOWS_HEADING_ROW_HEIGHT,
            SHOWS_SERIES_ROW_HEIGHT,
        ]
    );
    assert_eq!(painted[0].top(), 0.0);
    assert_eq!(painted[1].top(), SHOWS_HEADING_ROW_HEIGHT);
    let letters: Vec<Option<char>> = painted.iter().map(heading_letter).collect();
    assert_eq!(
        letters,
        [
            Some('A'),
            None,
            None,
            None,
            None,
            None,
            Some('B'),
            None,
            Some('#'),
            None,
        ]
    );
}

// A closed series paints no season and a closed season no episode.
#[test]
fn a_closed_series_paints_no_season_and_a_closed_season_no_episode() {
    let closed = build_shows_rows(
        &shows_frame(GuiShowsModeFrame::All, tree(), episodes()),
        GuiShowsModeFrame::All,
        "",
        ShowsTreeState::default(),
    );

    let kinds: Vec<&str> = closed.iter().map(kind_of).collect();
    assert_eq!(
        kinds,
        [
            "heading", "series", "series", "heading", "series", "heading", "series"
        ]
    );
}

// The flat modes paint no heading and keep each row its date.
#[test]
fn the_flat_modes_paint_no_heading_and_keep_each_row_its_date() {
    let latest = build_shows_rows(
        &shows_frame(GuiShowsModeFrame::Latest, Vec::new(), episodes()),
        GuiShowsModeFrame::Latest,
        "",
        ShowsTreeState::default(),
    );
    let upcoming = build_shows_rows(
        &shows_frame(
            GuiShowsModeFrame::Upcoming,
            Vec::new(),
            vec![flat("p1", "The Bear", "20 May")],
        ),
        GuiShowsModeFrame::Upcoming,
        "",
        ShowsTreeState::default(),
    );

    let kinds: Vec<&str> = latest.iter().map(kind_of).collect();
    assert_eq!(kinds, ["flat", "flat"]);
    let heights: Vec<f32> = latest.iter().map(GeometryRow::height).collect();
    assert_eq!(heights, [SHOWS_FLAT_ROW_HEIGHT, SHOWS_FLAT_ROW_HEIGHT]);
    let dates: Vec<Option<&str>> = latest
        .iter()
        .map(|row| match row {
            ShowsListRow::Flat(flat) => flat.row.date.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(dates, [Some("12 May"), Some("9 May")]);
    assert_eq!(
        upcoming.iter().map(kind_of).collect::<Vec<&str>>(),
        ["flat"]
    );
}

// The filter keeps a series whose season matches and opens it.
#[test]
fn the_filter_keeps_a_series_whose_season_matches_and_opens_it() {
    let painted = build_shows_rows(
        &shows_frame(GuiShowsModeFrame::All, tree(), episodes()),
        GuiShowsModeFrame::All,
        "season 2",
        ShowsTreeState::default(),
    );

    let kinds: Vec<&str> = painted.iter().map(kind_of).collect();
    assert_eq!(kinds, ["heading", "series", "season"]);
    let season_title = match &painted[2] {
        ShowsListRow::Season(season) => season.title.as_str(),
        _ => "",
    };
    assert_eq!(season_title, "Season 2");
}

// The active mode is the pill the core marked selected, and a frame that
// marks none falls back to the rows' own shape.
#[test]
fn the_active_mode_is_the_pill_the_core_marked_selected() {
    let shows = shows_frame(GuiShowsModeFrame::Upcoming, Vec::new(), episodes());

    assert_eq!(active_shows_mode(&shows), GuiShowsModeFrame::Upcoming);
    let unmarked = GuiShowsList {
        pills: Vec::new(),
        ..shows_frame(GuiShowsModeFrame::Upcoming, Vec::new(), episodes())
    };
    assert_eq!(active_shows_mode(&unmarked), GuiShowsModeFrame::Latest);
    let tree_unmarked = GuiShowsList {
        pills: Vec::new(),
        ..shows_frame(GuiShowsModeFrame::All, tree(), episodes())
    };
    assert_eq!(active_shows_mode(&tree_unmarked), GuiShowsModeFrame::All);
}

// The rail holds the first series row under each letter and disables the
// rest.
#[test]
fn the_rail_holds_the_first_series_row_under_each_letter_and_disables_the_rest() {
    let rail = shows_letter_rail(&build_shows_rows(
        &shows_frame(GuiShowsModeFrame::All, tree(), episodes()),
        GuiShowsModeFrame::All,
        "",
        ShowsTreeState::default(),
    ));
    let letter = |letter: char| {
        rail.iter()
            .find(|entry| entry.letter == letter)
            .and_then(|entry| entry.row_id.clone())
    };

    assert_eq!(letter('A'), Some(series_row_id("s1")));
    assert_eq!(letter('B'), Some(series_row_id("s3")));
    assert_eq!(letter('C'), None);
    assert_eq!(episode_row_id("ep1"), "shows-episode:ep1");
}
