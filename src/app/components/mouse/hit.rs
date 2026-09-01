use ratatui::layout::{Position, Rect};

/// Paint-time hit regions. Later regions take precedence when rectangles overlap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitRegions<Tag> {
    regions: Vec<(Rect, Tag)>,
}

impl<Tag> HitRegions<Tag> {
    pub fn clear(&mut self) {
        self.regions.clear();
    }
    pub fn push(&mut self, rect: Rect, tag: Tag) {
        self.regions.push((rect, tag));
    }
    pub fn resolve(&self, point: Position) -> Option<&Tag> {
        self.regions
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(point))
            .map(|(_, tag)| tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn last_pushed_region_wins() {
        let mut h = HitRegions::default();
        h.push(Rect::new(0, 0, 4, 4), 1);
        h.push(Rect::new(1, 1, 2, 2), 2);
        assert_eq!(h.resolve(Position::new(1, 1)), Some(&2));
        assert_eq!(h.resolve(Position::new(9, 9)), None);
    }
}
