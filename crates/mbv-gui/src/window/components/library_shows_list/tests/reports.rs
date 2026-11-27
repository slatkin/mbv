//! The reports the Shows list sends on its own, apart from the commands the
//! test presses: the row art set for the rows in view, and the open series in
//! view whose seasons load (gui-delineated-rows design D7, gui-list-header-buttons
//! design D5).

use super::*;

impl Fixture {
    /// The `RowsInView` reports the core link received, in send order.
    fn reported_rows_in_view(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(
                    command @ crate::gui_core::GuiLibraryCommand::RowsInView { .. },
                ) => Some(command),
                _ => None,
            })
            .collect()
    }

    /// The `ShowsOpenInView` reports the core link received, in send order.
    fn reported_shows_open_in_view(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(
                    command @ crate::gui_core::GuiLibraryCommand::ShowsOpenInView { .. },
                ) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Contract (gui-delineated-rows design D7): the list reports the rows it
/// paints under the scope its visible view names, with the ids the core
/// resolves — a series row's raw Emby id, and no row key the list itself adds —
/// so the core queues their row art without a coordinate.
#[rstest::rstest]
#[case(crate::gui_core::GuiLibraryView::All, crate::gui_core::GuiRowArtScope::ShowsSeries, &["s1", "s3"])]
#[case(crate::gui_core::GuiLibraryView::New, crate::gui_core::GuiRowArtScope::ShowsNew, &["new:e1"])]
#[case(crate::gui_core::GuiLibraryView::Upcoming, crate::gui_core::GuiRowArtScope::ShowsUpcoming, &["new:e1"])]
fn the_list_reports_the_rows_in_view_under_the_views_scope(
    #[case] view: crate::gui_core::GuiLibraryView,
    #[case] scope: crate::gui_core::GuiRowArtScope,
    #[case] ids: &[&str],
) {
    let frame = GuiShowsList {
        sort: None,
        view,
        rows: new_view_rows(),
        ..shows_frame(tree())
    };
    let mut fixture = built(frame, shows_detail(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.reported_rows_in_view().last(),
        Some(&crate::gui_core::GuiLibraryCommand::RowsInView {
            scope,
            ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        }),
        "the report names the scope and the ids the core resolves"
    );
}

/// The series list's tree with a third letter group past a 300 px viewport:
/// each letter costs a heading row and a series row, so the last series' top
/// edge sits below the window's bottom edge and its row never paints.
fn tree_with_a_series_past_the_window() -> Vec<GuiShowsSeriesFrame> {
    let mut tree = tree();
    tree.extend([
        GuiShowsSeriesFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "s4".to_owned(),
            title: "Chernobyl".to_owned(),
            sort_letter: 'C',
            expandable: true,
            seasons: Vec::new(),
        },
        GuiShowsSeriesFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "s5".to_owned(),
            title: "Dark".to_owned(),
            sort_letter: 'D',
            expandable: true,
            seasons: Vec::new(),
        },
        GuiShowsSeriesFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "s6".to_owned(),
            title: "Euphoria".to_owned(),
            sort_letter: 'E',
            expandable: true,
            seasons: Vec::new(),
        },
    ]);
    tree
}

/// Contract (gui-list-header-buttons design D5, spec "A Shows series that is
/// open and in view loads its seasons when they are not loaded", proposal: "In
/// a tree list, the children of the open parent rows in view also reload, and
/// no load starts for an open row out of view"): the report names the open
/// series of the painted window alone, so a closed series beside it and an
/// open series past the window's edge both stay out.
#[test]
fn the_open_series_in_view_is_reported_and_a_closed_or_out_of_view_one_is_not() {
    let mut fixture = built(
        shows_frame(tree_with_a_series_past_the_window()),
        shows_detail(None),
    );
    fixture.measure(300.0);

    toggle_row(&mut fixture, "shows-series:s1");
    toggle_row(&mut fixture, "shows-series:s6");

    assert_eq!(
        fixture.reported_shows_open_in_view().last(),
        Some(&crate::gui_core::GuiLibraryCommand::ShowsOpenInView {
            lib_idx: 0,
            series: vec!["s1".to_owned()],
            seasons: Vec::new(),
        }),
        "the report names the open series in view and neither the closed series nor the open series past the window"
    );
}
