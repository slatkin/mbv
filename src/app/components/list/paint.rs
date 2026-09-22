//! Retained geometry for the most recently completed list paint.
//!
//! A painter hands the carrier its completed target-bearing row geometry.  The
//! geometry is deliberately write-only at this boundary: callers can ask
//! whether a point is claimed or resolve it to a stable target, but cannot
//! inspect the retained row map or reconstruct one from stale state.

use ratatui::layout::{Position, Rect};

struct CompletedPaint<Target> {
    claim_rect: Rect,
    content_rect: Rect,
    flow_offset: usize,
    rows: Vec<(Rect, Target)>,
    selected_row_rect: Option<Rect>,
}

/// State carrier used by a list shape's retained-paint adapter.
///
/// The carrier owns the single completed-paint snapshot: claim and content
/// rectangles, the flow offset, target-bearing row geometry, and the selected
/// row rectangle. A shape keeps no second paint-result carrier.
pub struct PaintRetainedState<Target> {
    completed: Option<CompletedPaint<Target>>,
}

impl<Target> Default for PaintRetainedState<Target> {
    fn default() -> Self {
        Self { completed: None }
    }
}

impl<Target> PaintRetainedState<Target> {
    /// Create an empty retained-paint carrier.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a paint.  Until [`Self::finish`] completes, the previous frame is
    /// not eligible for hit testing.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn begin(&mut self) {
        self.completed = None;
    }

    /// Discard the completed frame's geometry.
    pub fn invalidate(&mut self) {
        self.completed = None;
    }

    /// Publish one completed frame's target-bearing row geometry.
    ///
    /// `rows` is consumed into private retained state.  No public operation
    /// returns the row rectangles or their map; point resolution is the only
    /// read path.  A row that is structural can simply be omitted from the
    /// target-bearing geometry and will resolve to `None`.
    ///
    /// `content_rect` is the row-flow rectangle this frame painted and
    /// `flow_offset` the display-row index parked at its top; both are the
    /// shape's own completed-paint facts, retained here so no shape keeps a
    /// second paint-result carrier.
    pub fn store_completed<I>(
        &mut self,
        claim_rect: Rect,
        content_rect: Rect,
        flow_offset: usize,
        rows: I,
        selected_row_rect: Option<Rect>,
    ) where
        I: IntoIterator<Item = (Rect, Target)>,
    {
        self.completed = Some(CompletedPaint {
            claim_rect,
            content_rect,
            flow_offset,
            rows: rows.into_iter().collect(),
            selected_row_rect,
        });
    }

    /// Whether a completed frame is currently retained.
    pub fn is_valid(&self) -> bool {
        self.completed.is_some()
    }

    /// The latest completed frame's claim rectangle.
    pub fn claim_rect(&self) -> Option<Rect> {
        self.completed.as_ref().map(|paint| paint.claim_rect)
    }

    /// The latest completed frame's row-flow rectangle.
    pub fn content_rect(&self) -> Option<Rect> {
        self.completed.as_ref().map(|paint| paint.content_rect)
    }

    /// The latest completed frame's display-row offset at the viewport top.
    pub fn flow_offset(&self) -> Option<usize> {
        self.completed.as_ref().map(|paint| paint.flow_offset)
    }

    /// Whether the latest completed paint claims `point`.
    pub fn claims_point(&self, point: Position) -> bool {
        self.completed
            .as_ref()
            .is_some_and(|paint| paint.claim_rect.contains(point))
    }

    /// Resolve `point` against the latest completed paint's target geometry.
    pub fn resolve_point(&self, point: Position) -> Option<&Target> {
        let paint = self.completed.as_ref()?;
        if !paint.claim_rect.contains(point) {
            return None;
        }
        paint
            .rows
            .iter()
            .find_map(|(rect, target)| rect.contains(point).then_some(target))
    }

    /// The latest completed paint's selected-row rectangle, if any.
    pub fn selected_row_rect(&self) -> Option<Rect> {
        self.completed
            .as_ref()
            .and_then(|paint| paint.selected_row_rect)
    }

    /// A target's one-line row rectangle from the latest completed paint,
    /// clipped to the frame's content area (so a full-width selected row's
    /// claim band is not part of the row rect), if it painted one. This is a
    /// read-only stable-target look-up: the retained row map itself stays
    /// private.
    pub fn row_rect_for(&self, target: &Target) -> Option<Rect>
    where
        Target: PartialEq,
    {
        let paint = self.completed.as_ref()?;
        paint.rows.iter().find_map(|(rect, row_target)| {
            (row_target == target).then(|| rect.intersection(paint.content_rect))
        })
    }
}

/// Shared retained-paint operations supplied by a list shape's state owner.
pub trait PaintRetained<Target> {
    /// Borrow the private retained-paint carrier.
    fn paint_retained(&self) -> &PaintRetainedState<Target>;

    /// Borrow the private retained-paint carrier mutably for the default
    /// transition operations.
    fn paint_retained_mut(&mut self) -> &mut PaintRetainedState<Target>;

    /// Begin a frame and invalidate the previous geometry.
    #[cfg_attr(not(test), allow(dead_code))]
    fn begin(&mut self) {
        self.paint_retained_mut().begin();
    }

    /// Finish a frame by consuming its target-bearing geometry.
    fn finish<I>(
        &mut self,
        claim_rect: Rect,
        content_rect: Rect,
        flow_offset: usize,
        rows: I,
        selected_row_rect: Option<Rect>,
    ) where
        I: IntoIterator<Item = (Rect, Target)>,
    {
        self.paint_retained_mut().store_completed(
            claim_rect,
            content_rect,
            flow_offset,
            rows,
            selected_row_rect,
        );
    }

    /// Whether a completed frame is currently retained.
    #[allow(dead_code)]
    fn has_paint(&self) -> bool {
        self.paint_retained().is_valid()
    }

    /// The latest completed frame's claim rectangle.
    #[allow(dead_code)]
    fn claim_rect(&self) -> Option<Rect> {
        self.paint_retained().claim_rect()
    }

    /// The latest completed frame's row-flow rectangle.
    #[cfg_attr(not(test), allow(dead_code))]
    fn content_rect(&self) -> Option<Rect> {
        self.paint_retained().content_rect()
    }

    /// The latest completed frame's display-row offset at the viewport top.
    #[cfg_attr(not(test), allow(dead_code))]
    fn flow_offset(&self) -> Option<usize> {
        self.paint_retained().flow_offset()
    }

    /// Explicitly invalidate retained geometry after content or geometry
    /// changes.
    #[cfg_attr(not(test), allow(dead_code))]
    fn invalidate(&mut self) {
        self.paint_retained_mut().invalidate();
    }

    /// Whether the latest completed frame claims `point`.
    #[cfg_attr(not(test), allow(dead_code))]
    fn claims_point(&self, point: Position) -> bool {
        self.paint_retained().claims_point(point)
    }

    /// Resolve `point` to a stable target from the latest completed frame.
    #[cfg_attr(not(test), allow(dead_code))]
    fn resolve_point(&self, point: Position) -> Option<&Target> {
        self.paint_retained().resolve_point(point)
    }

    /// The selected row's rectangle from the latest completed frame.
    #[cfg_attr(not(test), allow(dead_code))]
    fn selected_row_rect(&self) -> Option<Rect> {
        self.paint_retained().selected_row_rect()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::{Position, Rect};

    use super::{PaintRetained, PaintRetainedState};

    #[derive(Default)]
    struct TestPaint {
        retained: PaintRetainedState<u8>,
    }

    impl PaintRetained<u8> for TestPaint {
        fn paint_retained(&self) -> &PaintRetainedState<u8> {
            &self.retained
        }

        fn paint_retained_mut(&mut self) -> &mut PaintRetainedState<u8> {
            &mut self.retained
        }
    }

    #[test]
    fn invalid_geometry_claims_no_point_or_selected_row() {
        let mut paint = TestPaint::default();
        let claim = Rect::new(2, 3, 4, 2);
        paint.finish(
            claim,
            claim,
            0,
            [(Rect::new(2, 3, 4, 1), 7)],
            Some(Rect::new(2, 3, 4, 1)),
        );
        assert!(paint.claims_point(Position { x: 2, y: 3 }));
        assert!(paint.selected_row_rect().is_some());

        paint.begin();
        assert!(!paint.claims_point(Position { x: 2, y: 3 }));
        assert_eq!(paint.resolve_point(Position { x: 2, y: 3 }), None);
        assert_eq!(paint.selected_row_rect(), None);
        assert_eq!(paint.content_rect(), None);
        assert_eq!(paint.flow_offset(), None);

        paint.finish(
            claim,
            claim,
            0,
            [(Rect::new(2, 3, 4, 1), 7)],
            Some(Rect::new(2, 3, 4, 1)),
        );
        paint.invalidate();
        assert!(!paint.claims_point(Position { x: 2, y: 3 }));
        assert_eq!(paint.selected_row_rect(), None);
    }

    #[test]
    fn completed_geometry_resolves_stable_targets_without_exposing_rows() {
        let mut paint = TestPaint::default();
        paint.finish(
            Rect::new(0, 0, 8, 3),
            Rect::new(0, 0, 8, 3),
            1,
            [(Rect::new(0, 0, 8, 1), 11), (Rect::new(0, 2, 8, 1), 29)],
            None,
        );

        assert_eq!(paint.resolve_point(Position { x: 4, y: 0 }), Some(&11));
        assert_eq!(paint.resolve_point(Position { x: 4, y: 1 }), None);
        assert_eq!(paint.resolve_point(Position { x: 4, y: 2 }), Some(&29));
        assert_eq!(paint.resolve_point(Position { x: 9, y: 0 }), None);
        assert_eq!(paint.content_rect(), Some(Rect::new(0, 0, 8, 3)));
        assert_eq!(paint.flow_offset(), Some(1));
    }
}
