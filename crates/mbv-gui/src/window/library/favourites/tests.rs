//! Headless tests of the Favourites list's row model (emby-favourites design
//! D6): the rows stack at the shared top-level row height with no letter
//! heading, and the list's own filter text narrows them by title. The row model
//! is plain data over fixed heights, so the geometry is asserted without a
//! layout and no test opens a window.

use crate::bridge::frame::library::{
    GuiEmbyRowFrame, GuiEmbyRowKindFrame, GuiFavouritesList, GuiHomeRowStateFrame, GuiRowArtFrame,
    GuiRowMediaKindFrame,
};
use crate::window::library::favourites::favourites_rows;
use crate::window::library::list::LIBRARY_ROW_HEIGHT;

/// One carried favourites row with no art and no played state.
fn row(id: &str, title: &str) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        date: None,
        sort_letter: None,
        kind: GuiEmbyRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The carried list part over `rows`, landed and with no pick.
fn part(rows: Vec<GuiEmbyRowFrame>) -> GuiFavouritesList {
    GuiFavouritesList {
        loading: false,
        rows,
        selected: None,
        hero: None,
    }
}

/// emby-favourites design D6: the list paints no letter heading and no rail, so
/// every row is an item row at the shared top-level height, and the rows stack
/// from the list's first pixel.
#[test]
fn the_rows_stack_at_the_top_level_row_height() {
    let part = part(vec![row("m1", "Alien"), row("m2", "Brazil")]);

    let rows = favourites_rows(&part, "");

    assert_eq!(
        rows.iter()
            .map(|row| (row.top, row.height))
            .collect::<Vec<_>>(),
        [
            (0.0, LIBRARY_ROW_HEIGHT),
            (LIBRARY_ROW_HEIGHT, LIBRARY_ROW_HEIGHT)
        ]
    );
}

/// emby-favourites design D6: the list's own filter text narrows its rows by
/// title, without case.
#[test]
fn the_filter_text_narrows_the_rows_by_title() {
    let part = part(vec![row("m1", "Alien"), row("m2", "Brazil")]);

    let rows = favourites_rows(&part, "BRA");

    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["m2"]
    );
}
