//! The shared breakpoint classes (add-gui-front-end design D10, row 6.6).
//!
//! Each Client maps its own size to a [`BreakpointClass`] with its own
//! threshold set: the TUI counts terminal columns and the GUI counts window
//! logical points. The core owns what each class allows, so both Clients
//! give the same class the same layout behaviour.

/// The layout breakpoint class of a Client's size.
///
/// `Mini` shows one panel. `Narrow` shows both panels, and the hero opens as
/// an overlay. `Wide` shows both panels, and the hero sits beside its browser
/// when [`hero_side_by_side`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointClass {
    /// Only one of Library or Queue shows.
    Mini,
    /// Both panels show. The hero opens as an overlay on demand.
    Narrow,
    /// Both panels show. The hero may sit beside its browser.
    Wide,
}

/// The width thresholds that one Client supplies to [`classify`], in its own
/// unit: terminal columns for the TUI, window logical points for the GUI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreakpointThresholds<W> {
    /// Widths below this are `Mini`.
    pub mini_below: W,
    /// Widths at or above this are `Wide`.
    pub wide_from: W,
}

/// Maps a Client's `width` to the breakpoint class that the width forces.
///
/// A width below `mini_below` is `Mini`, even when `wide_from` is lower. A
/// width at or above `wide_from` is `Wide`. Every other width is `Narrow`.
#[must_use]
pub fn classify<W: Copy + PartialOrd>(
    width: W,
    thresholds: BreakpointThresholds<W>,
) -> BreakpointClass {
    if width < thresholds.mini_below {
        BreakpointClass::Mini
    } else if width >= thresholds.wide_from {
        BreakpointClass::Wide
    } else {
        BreakpointClass::Narrow
    }
}

/// Whether the Library hero sits beside its browser (the Wide hero).
///
/// Only `Wide` qualifies, and only when the Client's minimum-height guard
/// holds. The Client computes `min_height_ok` from its own geometry.
#[must_use]
pub fn hero_side_by_side(class: BreakpointClass, min_height_ok: bool) -> bool {
    class == BreakpointClass::Wide && min_height_ok
}
