//! The core focus stack's shell seam (design D3 of change
//! `extract-client-core`, capability `client-interaction-core`).
//!
//! `mbv_surfaces::FocusStack` is the one focus authority: it owns the mounted
//! set, the Panel focus in effect, and the overlay stack, and it derives the
//! focused surface from them. Shell call sites request a change on the stack
//! ([`Model::push_focus_overlay`] and [`Model::dismiss_focus_overlay`], plus a
//! plain mount record for the panel surfaces) instead of calling `TuiRealm`
//! focus directly, and [`Model::apply_focus_stack`] — the sync pass's one
//! focus step — applies the derived focus to `TuiRealm`. No other shell code
//! calls `TuiRealm` `active`/`blur`.

use super::Model;
use mbv_ui_msg::ComponentId;

/// The permanent panel surfaces the derived focus can name. Their mount sites
/// route a plain mount record to the stack; the focus step still mirrors their
/// `TuiRealm` mount state each pass, so a mount or umount any site missed
/// cannot leave the stack stale.
const PANEL_SURFACE_IDS: [ComponentId; 3] = [
    ComponentId::UiRoot,
    ComponentId::Library,
    ComponentId::Queue,
];

/// Every overlay identity the shell mounts — the sidebars, the context menu,
/// the blocking modals, and the Settings popups — in the order
/// [`Model::overlay_holds_focus`] checks them.
pub(in crate::app) fn overlay_surface_ids() -> [ComponentId; 13] {
    use mbv_ui_msg::{ModalId, OverlayId, PopupId};
    [
        ComponentId::Overlay(OverlayId::Search),
        ComponentId::Overlay(OverlayId::Settings),
        ComponentId::Overlay(OverlayId::Sessions),
        ComponentId::Overlay(OverlayId::Playlists),
        ComponentId::Overlay(OverlayId::Help),
        ComponentId::Overlay(OverlayId::ContextMenu),
        ComponentId::Modal(ModalId::Confirm),
        ComponentId::Modal(ModalId::DaemonLost),
        ComponentId::Modal(ModalId::SavePlaylist),
        ComponentId::Modal(ModalId::TwitchUrl),
        ComponentId::Popup(PopupId::Multiselect),
        ComponentId::Popup(PopupId::LibraryRoutes),
        ComponentId::Popup(PopupId::FeedManage),
    ]
}

impl Model {
    /// Routes an overlay mount through the core focus stack (design D3): the
    /// stack records the push and the focus step applies the derived focus —
    /// the pushed overlay — to `TuiRealm` right away, where the pre-routing
    /// call sites activated their component directly. A mount site that calls
    /// this never calls `TuiRealm` `active` itself.
    pub(in crate::app) fn push_focus_overlay(&mut self, id: &ComponentId) {
        self.focus_stack.push_overlay(id.clone());
        self.apply_focus_stack();
    }

    /// Routes an overlay dismissal through the core focus stack (design D3):
    /// the stack drops the overlay so the derived focus returns to the surface
    /// beneath it, `TuiRealm`'s own LIFO focus restore moves the focus at the
    /// umount, and the focus step re-derives. An id the stack does not know (a
    /// component a test mounted by hand) dismisses from `TuiRealm` alone; the
    /// next focus step's reconcile drops such a component from the stack too.
    pub(in crate::app) fn dismiss_focus_overlay(&mut self, id: &ComponentId) {
        self.focus_stack.dismiss(id);
        if self.application.mounted(id) {
            let _ = self.application.umount(id);
        }
        self.apply_focus_stack();
    }

    /// The sync pass's one focus step: feed the stack the shell-held inputs,
    /// reconcile the stack against `TuiRealm`'s mount state, and apply the
    /// stack's derived focus to `TuiRealm`. The pass calls it through
    /// `sync_active_destination`; no other shell code calls `TuiRealm`
    /// `active`/`blur`.
    pub(in crate::app) fn apply_focus_stack(&mut self) {
        // The panel focus in effect decides the focused panel surface; the
        // stored record (mini view, launch restore, the hidden empty-queue
        // hand-off) stays App-owned and feeds the stack here.
        self.focus_stack
            .set_panel_focus(self.app.effective_panel_focus());
        self.reconcile_focus_stack_mounts();
        let desired = self.focus_stack.focused();
        if desired.as_ref() == self.application.focus() {
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

    /// Reconciles the stack against `TuiRealm`'s mount state, both ways: the
    /// permanent panel surfaces follow their `TuiRealm` mount state, a stack
    /// overlay a direct umount removed leaves the stack, and a mounted overlay
    /// the stack does not know (a component a test mounted by hand) is adopted
    /// so the derivation keeps it focused as the top overlay.
    fn reconcile_focus_stack_mounts(&mut self) {
        for id in PANEL_SURFACE_IDS.iter().cloned() {
            if self.application.mounted(&id) {
                self.focus_stack.mount(id);
            } else {
                self.focus_stack.umount(&id);
            }
        }
        for id in overlay_surface_ids() {
            if self.application.mounted(&id) {
                self.focus_stack.push_overlay(id);
            } else {
                self.focus_stack.dismiss(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests;
