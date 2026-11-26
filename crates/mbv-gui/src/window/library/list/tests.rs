//! Headless tests of the Library list's row model (D16): the contracts of
//! `list.test.ts` — the canvas tree of artists, albums, and tracks under its
//! 24 px letter headings at the canvas's row heights, the filter that narrows
//! the rows without case, the rows in view, the cursor's movement over them,
//! and the A–Z rail that disables a letter no painted row carries. The model
//! is plain data over fixed row heights, so the geometry is asserted without
//! a layout.

use super::*;

/// The track Brand Nubian's album holds.
fn all_for_one() -> GuiMusicTrackFrame {
    GuiMusicTrackFrame {
        track_id: "t3".to_owned(),
        name: "All For One".to_owned(),
        number: 1,
        duration: "4:00".to_owned(),
    }
}

/// The A Tribe Called Quest artist the fixture carries: closed, with one
/// album whose tracks the content does not hold.
fn tribe() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: GuiArtistKey::Service("a3".to_owned()),
        name: "A Tribe Called Quest".to_owned(),
        sort_letter: 'A',
        albums: vec![GuiMusicAlbumFrame {
            album_id: "al4".to_owned(),
            name: "The Low End Theory".to_owned(),
            year: "1991".to_owned(),
            tracks: None,
            total: None,
        }],
    }
}

/// The Beastie Boys artist the fixture carries: two albums, the first with a
/// cached tracklist.
fn beastie() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: GuiArtistKey::Service("a1".to_owned()),
        name: "Beastie Boys".to_owned(),
        sort_letter: 'B',
        albums: vec![
            GuiMusicAlbumFrame {
                album_id: "al1".to_owned(),
                name: "Licensed To Ill".to_owned(),
                year: "1986".to_owned(),
                tracks: Some(vec![
                    GuiMusicTrackFrame {
                        track_id: "t1".to_owned(),
                        name: "Rhymin & Stealin".to_owned(),
                        number: 1,
                        duration: "3:27".to_owned(),
                    },
                    GuiMusicTrackFrame {
                        track_id: "t2".to_owned(),
                        name: "Paul Revere".to_owned(),
                        number: 9,
                        duration: "3:40".to_owned(),
                    },
                ]),
                total: None,
            },
            GuiMusicAlbumFrame {
                album_id: "al2".to_owned(),
                name: "Paul's Boutique".to_owned(),
                year: "1989".to_owned(),
                tracks: None,
                total: None,
            },
        ],
    }
}

/// The Brand Nubian artist the fixture carries: one album whose tracklist the
/// content holds.
fn brand() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: GuiArtistKey::Service("a2".to_owned()),
        name: "Brand Nubian".to_owned(),
        sort_letter: 'B',
        albums: vec![GuiMusicAlbumFrame {
            album_id: "al3".to_owned(),
            name: "One For All".to_owned(),
            year: "1990".to_owned(),
            tracks: Some(vec![all_for_one()]),
            total: None,
        }],
    }
}

/// The carried union the fixture builds: the artists in sort-name order, with
/// Beastie Boys and its first album open.
fn rows() -> Vec<LibraryRow> {
    build_library_rows(LibraryTree {
        artists: &[tribe(), beastie(), brand()],
        open: &["artist:service:a1".to_owned(), "album:al1".to_owned()],
        closed: &[],
        filter: "",
    })
}

/// Contract (gui-delineated-rows design D7): an artist row reports the id the
/// core's own Music encoder names — the Service identity, or the fallback
/// name — and the nested rows and letter headings report none.
#[test]
fn an_artist_row_reports_the_cores_row_art_id() {
    // A fallback artist's identity is its display name, as the Music grouping
    // builds it.
    let mut artists = vec![tribe()];
    artists[0].artist_key = GuiArtistKey::Fallback(artists[0].name.clone());
    let painted = build_library_rows(LibraryTree {
        artists: &artists,
        open: &[],
        closed: &[],
        filter: "",
    });
    let ids: Vec<Option<String>> = painted.iter().map(GeometryRow::rows_in_view_id).collect();

    assert_eq!(
        ids,
        [None, Some("artist-name:A Tribe Called Quest".to_owned())],
        "the fallback identity reports its name and the heading none"
    );
    let service = rows();
    let artist_ids: Vec<Option<String>> = service
        .iter()
        .filter(|row| matches!(row, LibraryRow::Artist { .. }))
        .map(GeometryRow::rows_in_view_id)
        .collect();
    assert_eq!(
        artist_ids,
        [
            Some("artist:a3".to_owned()),
            Some("artist:a1".to_owned()),
            Some("artist:a2".to_owned()),
        ],
        "a Service-named artist reports the Service id"
    );
}

// The canvas tree paints letter headings and the canvas row heights.
#[test]
fn the_canvas_tree_paints_letter_headings_and_the_canvas_row_heights() {
    let painted = rows();
    let kinds: Vec<&str> = painted
        .iter()
        .map(|row| match row {
            LibraryRow::Letter(_) => "letter",
            LibraryRow::Artist { .. } => "artist",
            LibraryRow::Album { .. } => "album",
            LibraryRow::Track { .. } => "track",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "letter", "artist", "letter", "artist", "album", "track", "track", "album", "artist"
        ]
    );
    let heights: Vec<f32> = painted.iter().map(|row| row.base().height).collect();
    assert_eq!(
        heights,
        [24.0, 56.0, 24.0, 56.0, 30.0, 28.0, 28.0, 30.0, 56.0]
    );
    let tops: Vec<f32> = painted.iter().map(|row| row.base().top).collect();
    assert_eq!(
        tops,
        [0.0, 24.0, 80.0, 104.0, 160.0, 190.0, 218.0, 246.0, 276.0]
    );
}

// The filter narrows the rows to the names that contain its text.
#[test]
fn the_filter_narrows_the_rows_to_the_names_that_contain_its_text() {
    let painted = build_library_rows(LibraryTree {
        artists: &[tribe(), beastie(), brand()],
        open: &[],
        closed: &[],
        filter: "ALL FOR",
    });
    let ids: Vec<&str> = painted.iter().map(LibraryRow::id).collect();
    assert_eq!(
        ids,
        ["letter:B", "artist:service:a2", "album:al3", "track:t3"]
    );
}

// The rail disables the letters no painted row carries.
#[test]
fn the_rail_disables_the_letters_no_painted_row_carries() {
    let rail = letter_rail(&rows());
    assert_eq!(rail.len(), 26);
    let letter = |letter: char| {
        rail.iter()
            .find(|entry| entry.letter == letter)
            .and_then(|entry| entry.row_id.clone())
    };
    assert_eq!(letter('A'), Some("artist:service:a3".to_owned()));
    assert_eq!(letter('B'), Some("artist:service:a1".to_owned()));
    assert_eq!(letter('C'), None);
}

// The window paints only the rows in view.
#[test]
fn the_window_paints_only_the_rows_in_view() {
    assert_eq!(
        list_window(&rows(), 54.0, 60.0),
        ListWindow { start: 1, end: 4 }
    );
}

// The scroll offset that shows a cursor below the viewport puts its bottom
// edge there.
#[test]
fn the_scroll_offset_that_shows_a_cursor_below_the_viewport_puts_its_bottom_edge_there() {
    assert_eq!(
        scroll_top_to_show_cursor(&rows(), Some("artist:service:a2"), 0.0, 60.0),
        272.0
    );
}

/// A view of the fixture list at `scroll_top`, 60 px of rows in view.
fn view(scroll_top: f32) -> crate::window::library::scroll::ListScroll {
    crate::window::library::scroll::ListScroll {
        scroll_top,
        viewport_height: 60.0,
    }
}

/// A view of the fixture list at `scroll_top` with `viewport_height` of rows in
/// view: the view a resize hands the next measurement.
fn view_at(scroll_top: f32, viewport_height: f32) -> crate::window::library::scroll::ListScroll {
    crate::window::library::scroll::ListScroll {
        scroll_top,
        viewport_height,
    }
}

// The resize's reveal (spec "A resize keeps the selected row in view"): a
// measurement that narrows the viewport scrolls the selected row, which the
// narrower viewport pushed below the fold, back into view.
#[test]
fn a_narrowed_viewport_scrolls_the_selected_row_back_into_view() {
    assert_eq!(
        measure_scroll_top(
            &rows(),
            Some("artist:service:a2"),
            view_at(0.0, 400.0),
            60.0
        ),
        272.0
    );
}

// A measurement that widens the viewport keeps the scroll: the selected row
// that now shows in full pulls nothing.
#[test]
fn a_widened_viewport_keeps_the_scroll() {
    assert_eq!(
        measure_scroll_top(
            &rows(),
            Some("artist:service:a3"),
            view_at(0.0, 60.0),
            400.0
        ),
        0.0
    );
}

// A resize never pulls the list back to a selection the user had already
// scrolled out of view: the measurement holds the offset inside the range and
// scrolls nothing.
#[test]
fn a_resize_leaves_a_selection_scrolled_out_of_view_alone() {
    assert_eq!(
        measure_scroll_top(
            &rows(),
            Some("artist:service:a3"),
            view_at(200.0, 60.0),
            40.0
        ),
        200.0
    );
}

// The first measurement has no viewport to compare with, so it takes the
// cursor's row only through its own reveal (design D9): the measurement itself
// scrolls nothing.
#[test]
fn the_first_measurement_scrolls_nothing_to_the_selection() {
    assert_eq!(
        measure_scroll_top(&rows(), Some("artist:service:a2"), view_at(0.0, 0.0), 60.0),
        0.0
    );
}

// The row step moves by the height of the row at the top edge.
#[test]
fn the_row_step_moves_by_the_height_of_the_top_row() {
    assert_eq!(scroll_step(&rows(), view(0.0), ListMove::Down), 24.0);
    assert_eq!(scroll_step(&rows(), view(24.0), ListMove::Up), 0.0);
}

// The page step moves by one viewport height.
#[test]
fn the_page_step_moves_by_the_viewport_height() {
    assert_eq!(scroll_step(&rows(), view(0.0), ListMove::PageDown), 60.0);
    assert_eq!(scroll_step(&rows(), view(100.0), ListMove::PageUp), 40.0);
}

// Home and End go to the list's ends.
#[test]
fn home_and_end_go_to_the_lists_ends() {
    assert_eq!(scroll_step(&rows(), view(100.0), ListMove::Home), 0.0);
    assert_eq!(scroll_step(&rows(), view(0.0), ListMove::End), 272.0);
}

// The step stays inside the list's scrollable range.
#[test]
fn the_step_stays_inside_the_lists_scrollable_range() {
    assert_eq!(scroll_step(&rows(), view(0.0), ListMove::Up), 0.0);
    assert_eq!(scroll_step(&rows(), view(272.0), ListMove::Down), 272.0);
}
