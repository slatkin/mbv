//! The ordered, provider-neutral row flow shared by list shapes.

/// A row in paint order. Structural rows occupy flow positions but cannot be
/// selected or addressed by a stable target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row<Target> {
    /// A selectable row addressed by its stable target.
    Selectable(Target),
    /// A painted row with no selectable identity (for example a heading or
    /// spacer).
    Structural,
}

impl<Target> Row<Target> {
    /// Construct a selectable row.
    pub fn selectable(target: Target) -> Self {
        Self::Selectable(target)
    }

    /// Construct a structural row.
    pub const fn structural() -> Self {
        Self::Structural
    }

    /// Borrow the stable target, if this row is selectable.
    pub fn target(&self) -> Option<&Target> {
        match self {
            Self::Selectable(target) => Some(target),
            Self::Structural => None,
        }
    }

    /// Whether this row is structural rather than selectable.
    pub fn is_structural(&self) -> bool {
        matches!(self, Self::Structural)
    }
}

/// One ordered flow of rows in the order in which they paint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowFlow<Target> {
    rows: Vec<Row<Target>>,
}

impl<Target> RowFlow<Target> {
    /// Create a flow from rows already ordered in paint order.
    pub fn new(rows: Vec<Row<Target>>) -> Self {
        Self { rows }
    }

    /// An empty flow.
    pub const fn empty() -> Self {
        Self { rows: Vec::new() }
    }

    /// Number of rows, including structural rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the flow has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Borrow a row by its flow position.
    pub fn row_at(&self, position: usize) -> Option<&Row<Target>> {
        self.rows.get(position)
    }

    /// Borrow all rows in paint order.
    pub fn rows(&self) -> &[Row<Target>] {
        &self.rows
    }

    /// Find a target's flow position. Structural rows never match.
    pub fn position_of(&self, target: &Target) -> Option<usize>
    where
        Target: Eq,
    {
        self.rows
            .iter()
            .position(|row| row.target().is_some_and(|candidate| candidate == target))
    }

    /// Count selectable rows.
    pub fn selectable_len(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.target().is_some())
            .count()
    }

    /// Borrow the selectable target at a flow position.
    pub(crate) fn target_at(&self, position: usize) -> Option<&Target> {
        self.row_at(position).and_then(Row::target)
    }

    /// Find the flow position of the `ordinal`th selectable row.
    pub(crate) fn position_of_selectable(&self, ordinal: usize) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(position, row)| row.target().map(|_| position))
            .nth(ordinal)
    }
}

impl<Target> From<Vec<Row<Target>>> for RowFlow<Target> {
    fn from(rows: Vec<Row<Target>>) -> Self {
        Self::new(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::{Row, RowFlow};

    #[test]
    fn structural_rows_occupy_positions_without_being_addressable() {
        let flow = RowFlow::new(vec![
            Row::selectable(10),
            Row::structural(),
            Row::selectable(20),
        ]);

        assert_eq!(flow.len(), 3);
        assert!(matches!(flow.row_at(1), Some(Row::Structural)));
        assert_eq!(flow.position_of(&10), Some(0));
        assert_eq!(flow.position_of(&20), Some(2));
        assert_eq!(flow.position_of(&30), None);
        assert_eq!(flow.row_at(1).and_then(Row::target), None);
    }

    #[test]
    fn selectable_count_and_position_ignore_structural_rows() {
        let flow = RowFlow::new(vec![
            Row::structural(),
            Row::selectable('a'),
            Row::structural(),
            Row::selectable('b'),
        ]);

        assert_eq!(flow.selectable_len(), 2);
        assert_eq!(flow.position_of_selectable(0), Some(1));
        assert_eq!(flow.position_of_selectable(1), Some(3));
        assert_eq!(flow.position_of_selectable(2), None);
    }
}
