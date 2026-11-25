//! The one open fact every Library dropdown popover shares.

/// A dropdown popover's interaction state: closed, or open with the filter
/// text the popover narrows its rows by. One value, so a close clears the
/// filter by construction. A popover with no filter field (the Shows view
/// dropdown) never reads the text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DropdownState(Option<String>);

impl DropdownState {
    /// Whether the popover paints.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.0.is_some()
    }

    /// The filter text while open, and the empty text while closed.
    #[must_use]
    pub fn filter(&self) -> &str {
        self.0.as_deref().unwrap_or_default()
    }

    /// Open with an empty filter.
    pub fn open(&mut self) {
        self.0 = Some(String::new());
    }

    /// Close and drop the filter text.
    pub fn close(&mut self) {
        self.0 = None;
    }

    /// Replace the filter text while open. A write while closed changes
    /// nothing, so a late filter event cannot reopen a closed popover.
    pub fn set_filter(&mut self, filter: String) {
        if self.is_open() {
            self.0 = Some(filter);
        }
    }
}
