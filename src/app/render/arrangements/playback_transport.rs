//! The width-driven transport arrangement (task 3.5, design D10): which
//! transport rows and indicators show at a given width and height. Both
//! playback panels consume it — the Queue playback panel's queue-column
//! transport (task 3.5) and the Library playback panel's right-column strip
//! (task 4.1) — so the two presentations cannot drift in what they show at
//! the same width. Placement only: the leaf painters
//! (`render/components/chrome_player.rs`) paint the rows it returns.

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

/// How much of the title row's right side the indicators take at a given
/// width: the full span set, or the elapsed time alone when the panel is
/// too narrow to fit the full set alongside the title.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum TransportIndicators {
    Full,
    ElapsedOnly,
}

/// The title row's width decision: which indicator set shows, and whether
/// the stop/next transport buttons fit beside the title.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct TransportPlan {
    pub(in crate::app) indicators: TransportIndicators,
    pub(in crate::app) transport_buttons: bool,
}

/// Measured title-row facts the width decision consumes (built by the
/// painter's `title_row_facts`, task 3.5).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct TransportMeasure {
    /// The play/pause glyph cell width (including its trailing space).
    pub(in crate::app) glyph_w: u16,
    /// The stop + next buttons' width including their separating spaces.
    pub(in crate::app) buttons_w: u16,
    /// The full indicator span set's width (progress + status + time).
    pub(in crate::app) full_w: u16,
    /// The elapsed-only fallback's width.
    pub(in crate::app) elapsed_w: u16,
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

/// The title row's width decision for a panel `available` columns wide with
/// a title of `title_width` columns: the transport buttons show only when
/// the full indicator set, the buttons and the title all fit; otherwise the
/// buttons drop and, if even the full set plus title cannot fit, the
/// indicators collapse to the elapsed time alone.
pub(in crate::app) fn transport_title_plan(
    available: u16,
    title_width: u16,
    measure: TransportMeasure,
) -> TransportPlan {
    let available = available as usize;
    let title_w = title_width as usize;
    let glyph_w = measure.glyph_w as usize;
    let full_w = measure.full_w as usize;
    let buttons_w = measure.buttons_w as usize;
    let transport_buttons = available >= glyph_w + full_w + buttons_w + title_w;
    let indicators = if transport_buttons || available >= glyph_w + full_w + title_w {
        TransportIndicators::Full
    } else {
        TransportIndicators::ElapsedOnly
    };
    TransportPlan {
        indicators,
        transport_buttons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure() -> TransportMeasure {
        TransportMeasure {
            glyph_w: 3,
            buttons_w: 6,
            full_w: 20,
            elapsed_w: 6,
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

    /// Wide panels show the full indicator set and the transport buttons.
    #[test]
    fn wide_panels_show_full_indicators_and_buttons() {
        let plan = transport_title_plan(80, 10, measure());
        assert_eq!(plan.indicators, TransportIndicators::Full);
        assert!(plan.transport_buttons);
    }

    /// A panel too narrow for buttons plus the full set beside the title
    /// drops the buttons but keeps the full indicators when they still fit.
    #[test]
    fn narrow_panels_drop_buttons_before_indicators() {
        // 36 columns: the full set + title fits, the buttons do not.
        let a = transport_title_plan(36, 10, measure());
        assert!(!a.transport_buttons);
        assert_eq!(a.indicators, TransportIndicators::Full);

        // 20 columns: even the full set + title no longer fits.
        let b = transport_title_plan(20, 10, measure());
        assert!(!b.transport_buttons);
        assert_eq!(b.indicators, TransportIndicators::ElapsedOnly);
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
