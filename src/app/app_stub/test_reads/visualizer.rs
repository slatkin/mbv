//! The visual slot's test facts and seeds (`split-tui-gui-clients` design
//! D4b rule 5): which content owns the slot, and the slot a test puts under
//! the player's active status.
//!
//! The slot's own context-menu tests drove these shapes by writing the
//! fields directly; each seam here names the shape instead — the content the
//! slot shows, the playing status an item rests under — so those tests keep
//! their claims and reach no field.

use super::super::App;

impl App {
    /// Whether the visualizer owns the visual slot rather than the item's
    /// artwork: the content the slot's own menu names as the alternative.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn visualizer_owns_the_slot_for_test(&self) -> bool {
        self.visualizer_enabled
    }

    /// Hold `enabled` as whether the visualizer owns the visual slot, as a
    /// settled mode change leaves it: the artwork then carries no title.
    #[cfg(any(test, feature = "test"))]
    pub fn set_visualizer_enabled_for_test(&mut self, enabled: bool) {
        self.visualizer_enabled = enabled;
    }

    /// Whether the persisted visual-slot hide is on, as a TUI run's prefs
    /// leave it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn visual_slot_hidden_for_test(&self) -> bool {
        self.visual_slot_hidden
    }

    /// Hold `hidden` as the persisted visual-slot hide, as a TUI run's prefs
    /// leave it for the next launch. It is the stored fact, not the `h`
    /// action: no Client capability gates it and no capture reconciles.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_visual_slot_hidden_for_test(&mut self, hidden: bool) {
        self.visual_slot_hidden = hidden;
    }
}
