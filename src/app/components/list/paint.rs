//! Retained geometry for the most recently completed list paint.
//!
//! A painter hands the carrier its completed target-bearing row geometry.  The
//! geometry is deliberately write-only at this boundary: callers can ask
//! whether a point is claimed or resolve it to a stable target, but cannot
//! inspect the retained row map or reconstruct one from stale state.

use ratatui::layout::{Position, Rect};

struct CompletedPaint<Target> {
    claim_rect: Rect,
    rows: Vec<(Rect, Target)>,
    selected_row_rect: Option<Rect>,
}

/// State carrier used by a list shape's retained-paint adapter.
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
    pub fn finish<I>(&mut self, claim_rect: Rect, rows: I, selected_row_rect: Option<Rect>)
    where
        I: IntoIterator<Item = (Rect, Target)>,
    {
        self.completed = Some(CompletedPaint {
            claim_rect,
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
}

/// Shared retained-paint operations supplied by a list shape's state owner.
pub trait PaintRetained<Target> {
    /// Borrow the private retained-paint carrier.
    fn paint_retained(&self) -> &PaintRetainedState<Target>;

    /// Borrow the private retained-paint carrier mutably for the default
    /// transition operations.
    fn paint_retained_mut(&mut self) -> &mut PaintRetainedState<Target>;

    /// Begin a frame and invalidate the previous geometry.
    fn begin(&mut self) {
        self.paint_retained_mut().begin();
    }

    /// Finish a frame by consuming its target-bearing geometry.
    fn finish<I>(&mut self, claim_rect: Rect, rows: I, selected_row_rect: Option<Rect>)
    where
        I: IntoIterator<Item = (Rect, Target)>,
    {
        self.paint_retained_mut()
            .finish(claim_rect, rows, selected_row_rect);
    }

    /// Whether a completed frame is currently retained.
    fn has_paint(&self) -> bool {
        self.paint_retained().is_valid()
    }

    /// The latest completed frame's claim rectangle.
    fn claim_rect(&self) -> Option<Rect> {
        self.paint_retained().claim_rect()
    }

    /// Explicitly invalidate retained geometry after content or geometry
    /// changes.
    fn invalidate(&mut self) {
        self.paint_retained_mut().invalidate();
    }

    /// Whether the latest completed frame claims `point`.
    fn claims_point(&self, point: Position) -> bool {
        self.paint_retained().claims_point(point)
    }

    /// Resolve `point` to a stable target from the latest completed frame.
    fn resolve_point(&self, point: Position) -> Option<&Target> {
        self.paint_retained().resolve_point(point)
    }

    /// The selected row's rectangle from the latest completed frame.
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
            [(Rect::new(2, 3, 4, 1), 7)],
            Some(Rect::new(2, 3, 4, 1)),
        );
        assert!(paint.claims_point(Position { x: 2, y: 3 }));
        assert!(paint.selected_row_rect().is_some());

        paint.begin();
        assert!(!paint.claims_point(Position { x: 2, y: 3 }));
        assert_eq!(paint.resolve_point(Position { x: 2, y: 3 }), None);
        assert_eq!(paint.selected_row_rect(), None);

        paint.finish(
            claim,
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
            [(Rect::new(0, 0, 8, 1), 11), (Rect::new(0, 2, 8, 1), 29)],
            None,
        );

        assert_eq!(paint.resolve_point(Position { x: 4, y: 0 }), Some(&11));
        assert_eq!(paint.resolve_point(Position { x: 4, y: 1 }), None);
        assert_eq!(paint.resolve_point(Position { x: 4, y: 2 }), Some(&29));
        assert_eq!(paint.resolve_point(Position { x: 9, y: 0 }), None);
    }
}
