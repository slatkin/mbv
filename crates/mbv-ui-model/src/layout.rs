//! Layout widths shared by both Clients' frame composition.
//!
//! The narrow/wide rules themselves are terminal-cell presentation and stay in
//! `mbv-render`; only the configured widths every Client resets and steps by
//! live here.

/// Default width, in terminal cells, of the queue column and of the wide
/// hero's left pane.
pub const LEFT_WIDTH_DEFAULT: u16 = 40;
/// One resize step, in terminal cells, for the queue column width.
pub const LEFT_WIDTH_STEP: u16 = 5;
