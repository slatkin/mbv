//! Persisted Wide hero list-pane width override.
//!
//! Mirrors `queue_column_width.rs`: one clamp against the shared arrangement's
//! minimum pane width, applied on every read. Unlike the queue column the
//! valid range depends on the *active surface's* content width, so the
//! override is stored raw and normalized where the geometry is consumed
//! rather than clamped in place — a terminal resize or a switch to a narrower
//! wide surface needs no dedicated event hook.

use crate::app::render::arrangements::wide_hero::{WIDE_HERO_MIN_PANE_WIDTH, WIDE_HERO_PANE_GAP};

/// Clamps a Wide hero list-pane width override to the shared arrangement's
/// valid range against `content_width`: the list pane and the hero pane each
/// stay at or above `WIDE_HERO_MIN_PANE_WIDTH`, separated by the
/// `WIDE_HERO_PANE_GAP` gutter. `None` passes through (default ratio). An
/// empty range — content too narrow for two minimum panes plus the gap —
/// yields `None` so the default ratio applies.
pub(in crate::app) fn normalize_list_pane_width(
    override_width: Option<u16>,
    content_width: u16,
) -> Option<u16> {
    let override_width = override_width?;
    let max = content_width
        .saturating_sub(WIDE_HERO_MIN_PANE_WIDTH)
        .saturating_sub(WIDE_HERO_PANE_GAP);
    (max >= WIDE_HERO_MIN_PANE_WIDTH).then(|| override_width.clamp(WIDE_HERO_MIN_PANE_WIDTH, max))
}
