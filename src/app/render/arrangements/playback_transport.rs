//! The width-driven transport arrangement (task 3.5, design D10): which
//! transport rows show at a given width and height, and whether the title
//! row's buttons fit. Both playback panels consume it — the Queue playback
//! panel's queue-column transport (task 3.5) and the Library playback
//! panel's right-column strip (task 4.1) — so the two presentations cannot
//! drift in what they show at the same width. Placement only: the leaf
//! painters (`render/components/chrome_player.rs`) paint the rows it returns.

use ratatui::layout::Rect;

/// The transport's rows for one panel rect: the seekbar, the title row that
/// carries the transport controls, and a blank trailing row. A row shows
/// only when the panel has rows to spend on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct TransportRows {
    pub(in crate::app) seekbar: Option<Rect>,
    pub(in crate::app) title: Option<Rect>,
    /// The blank row below the title (row 2).
    pub(in crate::app) indicator_row: Option<Rect>,
}

/// The title row's width decision: whether the stop/next transport buttons
/// fit beside the title and the indicators.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub(in crate::app) struct TransportMeasure {
    /// The play/pause glyph cell width (including its trailing space).
    pub(in crate::app) glyph_w: u16,
    /// The stop + next buttons' width including their separating spaces.
    pub(in crate::app) buttons_w: u16,
    /// The indicator span set's width (the elapsed time and the status pill).
    pub(in crate::app) indicators_w: u16,
}

/// Which transport rows render in a panel rect of `rows` available height.
/// Pure placement — no painting, no app state.
pub(in crate::app) fn transport_rows(area: Rect, rows: u16) -> TransportRows {
    let row = |n: u16| {
        (rows > n && area.width > 0).then_some(Rect {
            y: area.y + n,
            height: 1,
            ..area
        })
    };
    TransportRows {
        seekbar: row(0),
        title: row(1),
        indicator_row: row(2),
    }
}

/// Whether the title row's stop/next buttons fit: only when the glyph, the
/// buttons, the indicators and the title all fit side by side.
pub(in crate::app) fn transport_buttons_fit(
    available: u16,
    title_width: u16,
    measure: TransportMeasure,
) -> bool {
    available as usize
        >= measure.glyph_w as usize
            + measure.indicators_w as usize
            + measure.buttons_w as usize
            + title_width as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure() -> TransportMeasure {
        TransportMeasure {
            glyph_w: 3,
            buttons_w: 6,
            indicators_w: 14,
        }
    }

    fn panel(width: u16, rows: u16) -> Rect {
        Rect::new(4, 7, width, rows)
    }

    /// Rows gate on the available height; every row is one cell tall and
    /// stacks downward from the panel's top.
    #[test]
    fn rows_show_by_height() {
        let full = transport_rows(panel(60, 3), 3);
        assert!(full.seekbar.is_some() && full.title.is_some());
        assert!(full.indicator_row.is_some());
        assert_eq!(full.seekbar.unwrap().y, 7);
        assert_eq!(full.indicator_row.unwrap().y, 9);

        let short = transport_rows(panel(60, 2), 2);
        assert!(short.seekbar.is_some() && short.title.is_some());
        assert!(short.indicator_row.is_none());
    }

    /// A panel wide enough for the glyph, the buttons, the indicators and
    /// the title shows the buttons.
    #[test]
    fn wide_panels_show_the_transport_buttons() {
        assert!(transport_buttons_fit(80, 10, measure()));
    }

    /// A panel too narrow to fit the buttons beside the title drops them
    /// rather than overlapping the title.
    #[test]
    fn narrow_panels_drop_the_transport_buttons() {
        // 32 columns: the glyph, indicators and title fit, the buttons do not.
        assert!(!transport_buttons_fit(32, 10, measure()));
    }

    /// A zero-width panel places no rows.
    #[test]
    fn degenerate_panel_places_no_rows() {
        let a = transport_rows(panel(0, 3), 3);
        assert_eq!(
            a,
            TransportRows {
                seekbar: None,
                title: None,
                indicator_row: None,
            }
        );
        let b = transport_rows(panel(60, 0), 0);
        assert!(b.seekbar.is_none() && b.title.is_none());
    }
}
