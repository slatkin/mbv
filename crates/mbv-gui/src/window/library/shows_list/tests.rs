//! Headless tests of the Shows list's row model (D16): the contracts of
//! `shows-list.test.ts` — the series list's Music tree over the TUI's TV node
//! kinds with the letter headings, the closed nodes, the filter's auto-open,
//! the A–Z rail, and the flat modes' two-line row height. The GUI Shows
//! destination paints this list alone (gui-collection-dropdown design D8).
//! The model is plain data, so the geometry is asserted without a layout.

use super::*;
use crate::bridge::frame::library::GuiHomeRowStateFrame;

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

/// The series fixture: two `A` series (the first with a season and its
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

// The series list paints the tree at the Music heights.
#[test]
fn the_series_list_paints_the_tree_at_the_music_heights() {
    let open = vec![series_row_id("s1"), season_row_id("s1", "se1")];
    let closed: Vec<String> = Vec::new();
    let painted = build_shows_rows(
        GuiShowsView::All,
        &tree(),
        &[],
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
        GuiShowsView::All,
        &tree(),
        &[],
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

// The filter keeps a series whose season matches and opens it.
#[test]
fn the_filter_keeps_a_series_whose_season_matches_and_opens_it() {
    let painted = build_shows_rows(
        GuiShowsView::All,
        &tree(),
        &[],
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

// The rail holds the first series row under each letter and disables the
// rest.
#[test]
fn the_rail_holds_the_first_series_row_under_each_letter_and_disables_the_rest() {
    let rail = shows_letter_rail(&build_shows_rows(
        GuiShowsView::All,
        &tree(),
        &[],
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

/// One flat New or Upcoming row, as the frame carries it.
fn flat_row(id: &str, title: &str) -> GuiShowsRowFrame {
    GuiShowsRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some("The Beginning".to_owned()),
        date: Some("18 Nov".to_owned()),
        sort_letter: None,
        display_only: false,
    }
}

// The flat New and Upcoming rows paint at the two-line height their format
// was built for (gui-collection-dropdown D9): the old Home item rows' 48 px,
// whose two title lines fit — not the one-line track row's 28 px, which
// clipped them illegibly (the live defect of 2026-11-18).
#[test]
fn the_flat_rows_paint_at_the_two_line_height() {
    let rows = vec![flat_row("n1", "Alfred"), flat_row("n2", "Bridgerton")];
    let painted = build_shows_rows(GuiShowsView::New, &[], &rows, "", ShowsTreeState::default());

    let heights: Vec<f32> = painted.iter().map(GeometryRow::height).collect();
    assert_eq!(heights, [SHOWS_FLAT_ROW_HEIGHT, SHOWS_FLAT_ROW_HEIGHT]);
    assert_eq!(painted[0].top(), 0.0);
    assert_eq!(painted[1].top(), SHOWS_FLAT_ROW_HEIGHT);
}
