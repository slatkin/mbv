//! Session-only Wide hero list-pane width override.
//!
//! Mirrors `queue_column_width.rs`: one clamp against the shared arrangement's
//! minimum pane width, applied on every read. Unlike the queue column the
//! valid range depends on the *active surface's* content width, so the
//! override is stored raw and normalized where the geometry is consumed
//! rather than clamped in place — a terminal resize or a switch to a narrower
//! wide surface needs no dedicated event hook.

use super::render::arrangements::wide_hero::{WIDE_HERO_MIN_PANE_WIDTH, WIDE_HERO_PANE_GAP};

/// Clamps a Wide hero list-pane width override to the shared arrangement's
/// valid range against `content_width`: the list pane and the hero pane each
/// stay at or above `WIDE_HERO_MIN_PANE_WIDTH`, separated by the
/// `WIDE_HERO_PANE_GAP` gutter. `None` passes through (default ratio). An
/// empty range — content too narrow for two minimum panes plus the gap —
/// yields `None` so the default ratio applies.
pub(super) fn normalize_list_pane_width(
    override_width: Option<u16>,
    content_width: u16,
) -> Option<u16> {
    let override_width = override_width?;
    let max = content_width
        .saturating_sub(WIDE_HERO_MIN_PANE_WIDTH)
        .saturating_sub(WIDE_HERO_PANE_GAP);
    (max >= WIDE_HERO_MIN_PANE_WIDTH).then(|| override_width.clamp(WIDE_HERO_MIN_PANE_WIDTH, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_passes_through_as_default() {
        assert_eq!(normalize_list_pane_width(None, 200), None);
        assert_eq!(normalize_list_pane_width(None, 0), None);
    }

    #[test]
    fn clamps_to_both_pane_minimums() {
        assert_eq!(
            normalize_list_pane_width(Some(10), 100),
            Some(WIDE_HERO_MIN_PANE_WIDTH)
        );
        assert_eq!(
            normalize_list_pane_width(Some(200), 100),
            Some(100 - WIDE_HERO_MIN_PANE_WIDTH - WIDE_HERO_PANE_GAP)
        );
        assert_eq!(normalize_list_pane_width(Some(55), 100), Some(55));
    }

    #[test]
    fn empty_range_at_the_threshold_yields_default() {
        // One column short of two minimum panes plus the gap: no valid width.
        assert_eq!(normalize_list_pane_width(Some(60), 81), None);
        // Exactly the two-column threshold: a single valid width.
        assert_eq!(
            normalize_list_pane_width(Some(60), 82),
            Some(WIDE_HERO_MIN_PANE_WIDTH)
        );
    }

    #[test]
    fn terminal_resize_reclamps_without_a_resize_event_hook() {
        // The stored override is never mutated; each application against a new
        // content width re-clamps to the new bounds.
        let stored = Some(120u16);
        assert_eq!(normalize_list_pane_width(stored, 200), Some(120));
        assert_eq!(normalize_list_pane_width(stored, 140), Some(98));
        assert_eq!(
            normalize_list_pane_width(stored, 82),
            Some(WIDE_HERO_MIN_PANE_WIDTH)
        );
    }
}
