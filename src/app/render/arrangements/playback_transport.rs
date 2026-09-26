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
    /// The fourth row (row 3), present only when the band is tall enough:
    /// the queue column's expanded title band paints the title's second row
    /// there. The Library strip's three-row band never has it.
    pub(in crate::app) extra_row: Option<Rect>,
}

/// The title row's width decision: whether the stop/next transport buttons
/// fit beside the title and the indicators.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub(in crate::app) struct TransportMeasure {
    /// The play/pause glyph cell width (including its trailing space).
    pub(in crate::app) glyph: u16,
    /// The stop + next buttons' width including their separating spaces.
    pub(in crate::app) buttons: u16,
    /// The indicator span set's width (the elapsed time and the status pill).
    pub(in crate::app) indicators: u16,
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
        extra_row: row(3),
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
        >= measure.glyph as usize
            + measure.indicators as usize
            + measure.buttons as usize
            + title_width as usize
}
