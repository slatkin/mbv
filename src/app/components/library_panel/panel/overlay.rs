//! The Library-local Hero overlay lifecycle: opening, dismissal, and sync against the active destination owner.

use super::*;

impl LibraryPanel {
    /// Open the Library-local Hero overlay for the active Hero, retaining the
    /// destination owner's workspace focus just as browser Enter does.
    pub(in crate::app) fn open_hero_overlay_for_active(&mut self) -> bool {
        if !self.can_open_hero_overlay() {
            return false;
        }
        if let Some(owner) = self.owners.active_mut() {
            owner.focus_hero_workspace();
            owner.set_hero_overlay_open(true);
        }
        self.hero_overlay_open = true;
        true
    }

    /// Synchronize the compact mini-view episode hero. This overlay is a
    /// presentation of the selected flat episode, not an activation route;
    /// ordinary Enter handling remains owned by the destination.
    pub(in crate::app) fn sync_mini_view_hero_overlay(&mut self, mini_view: bool) {
        let available = mini_view
            && self
                .owners
                .active_mut()
                .is_some_and(|owner| owner.mini_view_hero_available());
        if available {
            if !self.hero_overlay_open && !self.mini_view_hero_auto_open {
                if let Some(owner) = self.owners.active_mut() {
                    owner.focus_hero_workspace();
                    owner.set_hero_overlay_open(true);
                }
                self.hero_overlay_open = true;
                self.mini_view_hero_auto_open = true;
            }
        } else if self.mini_view_hero_auto_open {
            self.dismiss_hero_overlay();
            self.mini_view_hero_auto_open = false;
        }
    }

    /// Open the Library-local Hero overlay for tests that construct a panel
    /// without an active owner.
    #[cfg(test)]
    pub(in crate::app) fn test_open_hero_overlay(&mut self) {
        self.hero_overlay_open = true;
    }

    pub(in crate::app) fn dismiss_hero_overlay(&mut self) {
        if self.hero_overlay_open {
            if let Some(owner) = self.owners.active_mut() {
                owner.set_hero_overlay_open(false);
                owner.clear_hero_workspace_focus();
            }
        }
        self.hero_overlay_open = false;
        self.overlay_geometry = None;
    }

    pub(in crate::app) fn sync_overlay_state(&mut self) {
        let hero_available = self.owners.active_mut().is_some_and(|owner| {
            owner.hero_overlay_available() || owner.hero_overlay_target_available()
        });
        if self.hero_overlay_open && !hero_available {
            self.dismiss_hero_overlay();
        }
        // Re-assert the open flag so it can never drift from the panel's own
        // bit (an owner reinstalled mid-session starts closed).
        let open = self.hero_overlay_open;
        if let Some(owner) = self.owners.active_mut() {
            owner.set_hero_overlay_open(open);
        }
    }

    #[cfg(test)]
    pub(in crate::app) fn test_hero_overlay_open(&self) -> bool {
        self.hero_overlay_open
    }

    #[cfg(test)]
    pub(in crate::app) fn test_overlay_geometry(
        &self,
    ) -> Option<(ratatui::layout::Rect, ratatui::layout::Rect)> {
        self.overlay_geometry
            .as_ref()
            .map(|geometry| (geometry.pane, geometry.frame))
    }

    /// The Library Hero overlay's painted Workspace box (panel, content)
    /// rects, for the overlay-pixel test path.
    #[cfg(test)]
    pub(in crate::app) fn test_overlay_workspace_box(
        &self,
    ) -> Option<(ratatui::layout::Rect, ratatui::layout::Rect)> {
        self.overlay_geometry.as_ref()?.hero.workspace
    }

    fn can_open_hero_overlay(&mut self) -> bool {
        self.owners.active_mut().is_some_and(|owner| {
            !owner.inline_search_active()
                && (owner.hero_overlay_available() || owner.hero_overlay_target_available())
        })
    }
}
