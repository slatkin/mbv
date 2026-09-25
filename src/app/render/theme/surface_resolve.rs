//! Resolves a `Surface` plus the call site's own focus bit to its
//! `SurfaceColors`.
//!
//! This is the table's only resolver (design D1/D3): a production paint site
//! names a `Surface` and hands in the focus input it already computes. The
//! focused side is level-driven; the resting side is the row's own value (see
//! `surface_table::RESTING_DEVIATIONS`).
//!
//! The table governs colour *identity*, not the bit's provenance: the same
//! bool may be a mounted component's focus, a pane's `held` bit, a panel focus
//! bool, or (for the few cursor-driven sites) the site's cursor predicate,
//! exactly as the site computes it today. A `Fixed` row ignores the bool and
//! pins focused == resting (design D3(a)); no row branches on anything else.

use super::palette::Palette;
use super::surface::{FocusSource, Surface};
use super::surface_table::{row, RESTING_DEVIATIONS};
use ratatui::style::Color;

/// The resolved colour of one rendered surface for one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct SurfaceColors {
    pub(in crate::app) fill: Color,
}

impl SurfaceColors {
    const fn fill(fill: Color) -> Self {
        Self { fill }
    }
}

/// Resolves a surface's fill from the call site's own focus bit
/// (design D1/D2/D3).
pub(in crate::app) fn surface_colors(surface: Surface, focused: bool) -> SurfaceColors {
    let row = row(surface);
    // The table is closed: `row` matches every variant, and every row's
    // resting value is its level's default or an explicitly declared
    // deviation. This holds it at runtime in debug builds.
    debug_assert!(
        surface.level().resting_default().is_none()
            || surface.level().resting_default() == Some(row.resting)
            || RESTING_DEVIATIONS
                .iter()
                .any(|(declared, _)| *declared == surface),
        "{surface:?} rests at a value that is neither its level default nor a declared deviation"
    );
    let follow_focus = focused && row.focus != FocusSource::Fixed;
    if follow_focus {
        SurfaceColors::fill(if row.soft {
            // The soft content-body surface fill: its own value, not the
            // scrollbar's `SCROLLBAR`, whose `Palette::Green2` value it
            // shares today — the two are equal today and independently
            // editable, so a scrollbar edit moves the scrollbar alone
            // (unify-surface-colour-neutral task 4.2).
            Palette::Green2.color()
        } else {
            row.level.focused_fill()
        })
    } else {
        SurfaceColors::fill(row.resting)
    }
}
