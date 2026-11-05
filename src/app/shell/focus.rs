//! The core focus stack's shell seam (design D3 of change
//! `add-gui-front-end`, capability `client-interaction-core`).
//!
//! `mbv_surfaces::FocusStack` is the one focus authority: it owns the mounted
//! set, the Panel focus in effect, and the overlay stack, and it derives the
//! focused surface from them. Shell call sites request a change on the stack
//! instead of calling `TuiRealm` focus directly, and the sync pass applies the
//! derived focus to `TuiRealm` in this module's one [`Model::apply_focus_stack`]
//! step. During the routing rows (3.3-3.6) the not-yet-routed call sites
//! still activate their components directly; the step leaves focus those
//! sites hold alone until their rows route them.

use super::Model;
use mbv_ui_msg::ComponentId;

/// The permanent panel surfaces the derived focus can name. Their mount
/// sites do not route through the stack until row 3.6, so the focus step
/// mirrors their ``TuiRealm`` mount state into the stack each pass.
const PANEL_SURFACE_IDS: [ComponentId; 3] = [
    ComponentId::UiRoot,
    ComponentId::Library,
    ComponentId::Queue,
];

impl Model {
    /// Routes an overlay mount through the core focus stack (design D3): the
    /// stack records the push, and the sync pass's focus step gives the
    /// overlay focus. A mount site that calls this never calls ``TuiRealm``
    /// `active` itself.
    pub(in crate::app) fn push_focus_overlay(&mut self, id: &ComponentId) {
        self.focus_stack.push_overlay(id.clone());
        self.focus_stack_pushed = true;
    }

    /// The sync pass's one focus step: feed the stack the shell-held inputs,
    /// reconcile the mount state the not-yet-routed call sites hold directly,
    /// and apply the stack's derived focus to `TuiRealm`. Rows 3.5/3.6 route
    /// the remaining call sites; from then on no other shell code calls
    /// `TuiRealm` `active`/`blur`.
    pub(in crate::app) fn apply_focus_stack(&mut self) {
        // Prefix-armed capture (design D6, task 6.1) holds focus off outside
        // the stack until the arm and disarm sites route through the stack's
        // hold (row 3.6): the arm blurs `TuiRealm` directly and the disarm
        // restores what it displaced, so this step must not re-derive focus
        // mid-capture.
        if self.app.prefix_armed {
            return;
        }
        // The panel focus in effect decides the focused panel surface; the
        // stored record (mini view, launch restore, the hidden empty-queue
        // hand-off) stays App-owned and feeds the stack here.
        self.focus_stack
            .set_panel_focus(self.app.effective_panel_focus());
        self.reconcile_focus_stack_mounts();
        let desired = self.focus_stack.focused();
        let pushed = std::mem::take(&mut self.focus_stack_pushed);
        if desired.as_ref() == self.application.focus() {
            return;
        }
        // A routed overlay push takes focus even over a surface whose call
        // site still activates its component directly (a Confirm raised from
        // the Playlists sidebar). Any other focus move waits until the holder
        // routes: an untracked overlay's own activation is its call site's
        // decision, not drift the stack corrects.
        let held_outside = self
            .application
            .focus()
            .is_some_and(|focused| self.focus_held_outside_the_stack(focused));
        if held_outside && !pushed {
            return;
        }
        match desired {
            Some(id) => {
                if self.application.mounted(&id) {
                    self.application
                        .active(&id)
                        .expect("apply the core focus stack");
                }
            }
            // Only the hold derives `None` without an unmounted surface under
            // it, so the step blurs only for the hold.
            None => {
                if self.focus_stack.focus_held() && self.application.focus().is_some() {
                    self.application
                        .blur()
                        .expect("apply the core focus-stack hold");
                }
            }
        }
    }

    /// Mirrors the `TuiRealm` mount state the not-yet-routed call sites hold
    /// into the stack: the permanent panel surfaces' mount sites (row 3.6),
    /// and any stack overlay a direct umount removed. Every dismissal routes
    /// through `Model::dismiss_modal`, so the overlay sweep only catches a
    /// not-yet-routed direct umount of a routed modal.
    fn reconcile_focus_stack_mounts(&mut self) {
        for id in PANEL_SURFACE_IDS.iter().cloned() {
            if self.application.mounted(&id) {
                self.focus_stack.mount(id);
            } else {
                self.focus_stack.umount(&id);
            }
        }
        for id in self.focus_stack.overlay_stack().to_vec() {
            if !self.application.mounted(&id) {
                self.focus_stack.dismiss(&id);
            }
        }
    }

    /// Whether `TuiRealm`'s focus sits on an overlay the stack does not know: a
    /// mount site that still activates its component directly (the sidebar,
    /// menu, and remaining row 3.5/3.6 call sites, and components a test
    /// mounted by hand). The step leaves such focus alone; the owning row
    /// routes the site and this check shrinks with it.
    fn focus_held_outside_the_stack(&self, focused: &ComponentId) -> bool {
        matches!(
            focused,
            ComponentId::Overlay(_) | ComponentId::Modal(_) | ComponentId::Popup(_)
        ) && !self.focus_stack.overlay_stack().contains(focused)
    }
}
