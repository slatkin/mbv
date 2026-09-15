use super::components::ComponentId;
use super::shell::Model;
use super::{PanelFocus, PanelMode};

impl Model {
    /// Route TuiRealm's native LIFO focus to the active destination's child
    /// component, or back to `UiRoot` when the destination has no mounted
    /// surface component (e.g. a narrow non-wide grouped-Music Emby
    /// library). Idempotent: `active()` on the already-active component is a
    /// no-op. The destination child is derived directly from `App.tab` via
    /// `library_child_id`; there is no Library-parent component or routing
    /// mirror.
    ///
    /// Short-circuits when Queue owns panel focus (and no blocking overlay
    /// is up): `sync_queue` already activated `ComponentId::Queue` a few
    /// lines earlier in the same tick, and we must not stomp it by
    /// re-activating the Library child or `UiRoot` on top. Without this
    /// guard Queue falls back to legacy key routing (issue #610, blocks
    /// the #607 acceptance gate). Mirrors the exact condition
    /// `sync_queue` uses to claim focus.
    pub(super) fn sync_active_destination(&mut self) {
        if self.overlay_holds_focus() {
            return;
        }
        let queue_owns_focus = matches!(self.app.effective_panel_focus(), PanelFocus::Queue)
            && !self.blocking_overlay_active();
        if queue_owns_focus {
            return;
        }
        let target = Some(ComponentId::Library)
            .filter(|child| self.application.mounted(child))
            .unwrap_or(ComponentId::UiRoot);
        if self.application.mounted(&target) {
            self.application
                .active(&target)
                .expect("activate Library child");
        }
    }

    /// The active library surface is always the mounted Library panel.
    pub(super) fn active_surface_id(&self) -> Option<ComponentId> {
        Some(ComponentId::Library)
    }

    /// Whether the right-panel library destination is painted this frame.
    /// The queue-only mode (including the narrow mini view) hides the library
    /// entirely, so its mounted destination must not paint over the queue that
    /// now owns the whole frame.
    pub(super) fn library_panel_visible(&self) -> bool {
        self.app.effective_panel_mode() != PanelMode::QueueOnly
    }

    /// ADR 0024 D2: the mouse-eligible component set for the current frame, a
    /// three-rung ladder derived from the active mounted surfaces the
    /// active-destination pass uses (no second "did I paint" ledger).
    pub(super) fn mouse_eligible_ids(&self) -> Vec<ComponentId> {
        use super::components::{ModalId, OverlayId, PopupId};

        // Rung 1: a mounted blocking overlay/modal is eligible alone.
        const BLOCKING: &[ComponentId] = &[
            ComponentId::Overlay(OverlayId::ContextMenu),
            ComponentId::Modal(ModalId::Confirm),
            ComponentId::Modal(ModalId::DaemonLost),
            ComponentId::Modal(ModalId::SavePlaylist),
            ComponentId::Popup(PopupId::Multiselect),
            ComponentId::Popup(PopupId::LibraryRoutes),
            ComponentId::Popup(PopupId::FeedManage),
        ];
        if let Some(id) = BLOCKING.iter().find(|id| self.application.mounted(id)) {
            return vec![id.clone()];
        }

        // Rung 2: else the topmost mounted panel-covering overlay/popup alone.
        // `OVERLAY_IDS` is canonical bottom-to-top, so the last mounted one is
        // topmost. No blocking overlay is mounted at this point, so every
        // remaining match is a non-blocking panel-covering overlay.
        if let Some(id) = super::components::UiRootComponent::overlay_ids()
            .iter()
            .rev()
            .find(|id| self.application.mounted(id))
        {
            return vec![id.clone()];
        }

        // Rung 3: the components painted this frame — active destination,
        // Queue, and the playback panels (the transport chrome).
        let mut ids = Vec::new();
        let panel_mode = self.app.effective_panel_mode();
        if let Some(child) = self
            .active_surface_id()
            .filter(|child| self.library_panel_visible() && self.application.mounted(child))
        {
            ids.push(child);
        }
        for id in [
            ComponentId::Queue,
            ComponentId::QueuePlaybackPanel,
            ComponentId::QueueBoundary,
        ] {
            if self.application.mounted(&id)
                && (id != ComponentId::QueueBoundary || self.queue_boundary_mouse_eligible())
                && panel_mode != super::PanelMode::LibraryOnly
            {
                ids.push(id);
            }
        }
        // The chrome panels paint only where `RootFrame` places them (tasks
        // 2.1-2.2, 4.1), and they are mounted exactly when a placement exists,
        // so the mounted check is the painted-this-frame check.
        for id in [
            ComponentId::TabPanel,
            ComponentId::StatusBarPanel,
            ComponentId::LibraryPlaybackPanel,
        ] {
            if self.application.mounted(&id) {
                ids.push(id);
            }
        }
        ids
    }

    /// Whether the Queue boundary column may receive mouse input. Shared by
    /// `mouse_eligible_ids` (subscription) and `sync_queue_boundary` (arming)
    /// so the two can never disagree.
    pub(super) fn queue_boundary_mouse_eligible(&self) -> bool {
        self.app.effective_panel_mode() == PanelMode::Both && self.panel_mouse_eligible()
    }

    pub(super) fn sync_queue_boundary(&mut self) {
        let id = ComponentId::QueueBoundary;
        // Task 1.4: the boundary is a two-panel-layout component only.
        // `RootFrame` places it (with its one-column rect) in the Both layout;
        // every other Panel mode unmounts it here so a mounted component
        // never outlives its placement (design D1's mount rule).
        if self.app.effective_panel_mode() != PanelMode::Both {
            if self.application.mounted(&id) {
                let _ = self.application.umount(&id);
            }
            return;
        }
        if !self.application.mounted(&id) {
            self.application
                .mount(
                    id.clone(),
                    Box::new(super::components::QueueBoundaryComponent::new()),
                    vec![],
                )
                .expect("mount QueueBoundary");
        }
        // The boundary reads its rect from `RootFrame` (the previous full
        // frame's placement -- the same paint signal the layout side channel
        // used to carry).
        let area = self
            .app
            .layout
            .root_frame
            .queue_boundary
            .unwrap_or_default();
        let enabled = self.queue_boundary_mouse_eligible() && area.width == 1 && area.height > 0;
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(boundary) = comp
                .as_any_mut()
                .downcast_mut::<super::components::QueueBoundaryComponent>()
            {
                boundary.sync(
                    area,
                    self.app.layout.left_area.x,
                    self.app.terminal_width,
                    self.app.queue_column_width,
                    enabled,
                );
            }
        }
    }

    /// ADR 0024 D2: reconcile the `mouse_sub()` subscription table to
    /// `mouse_eligible_ids()`. Runs in `sync_mounted_surfaces` immediately
    /// after `sync_active_destination`.
    ///
    /// `tuirealm` 4.1's `Application::unsubscribe(id, clause)` retains
    /// `s.target() != id && s.event() != &clause`, so it drops *every*
    /// subscription whose clause equals the mouse clause, not just `id`'s.
    /// Any change therefore wipes the whole mouse table and rebuilds it from
    /// the eligible set; `self.mouse_subscribed` mirrors the result so the
    /// reconciler knows the current state without querying `Application`.
    pub(super) fn sync_mouse_subscriptions(&mut self) {
        let eligible: std::collections::HashSet<ComponentId> =
            self.mouse_eligible_ids().into_iter().collect();
        if eligible == self.mouse_subscribed {
            return;
        }
        // Wipe: one successful unsubscribe clears every mouse subscription.
        if let Some(anchor) = self
            .mouse_subscribed
            .iter()
            .find(|id| self.application.mounted(id))
            .cloned()
        {
            let _ = self
                .application
                .unsubscribe(&anchor, super::components::mouse_event_clause());
        }
        self.mouse_subscribed.clear();
        for id in eligible {
            if self.application.mounted(&id)
                && self
                    .application
                    .subscribe(&id, super::components::mouse_sub())
                    .is_ok()
            {
                self.mouse_subscribed.insert(id);
            }
        }
    }

    /// Whether any sidebar, modal, or popup overlay is mounted. Every
    /// focus-management sync pass (`sync_active_destination`, `sync_queue`)
    /// consults this before re-activating its own surface, so a mounted
    /// overlay that just took focus is never stolen back on the next tick.
    pub(super) fn overlay_holds_focus(&self) -> bool {
        [
            ComponentId::Overlay(super::components::OverlayId::Search),
            ComponentId::Overlay(super::components::OverlayId::Settings),
            ComponentId::Overlay(super::components::OverlayId::Sessions),
            ComponentId::Overlay(super::components::OverlayId::Playlists),
            ComponentId::Overlay(super::components::OverlayId::Help),
            ComponentId::Overlay(super::components::OverlayId::ContextMenu),
            ComponentId::Modal(super::components::ModalId::Confirm),
            ComponentId::Modal(super::components::ModalId::DaemonLost),
            ComponentId::Modal(super::components::ModalId::SavePlaylist),
            ComponentId::Popup(super::components::PopupId::Multiselect),
            ComponentId::Popup(super::components::PopupId::LibraryRoutes),
            ComponentId::Popup(super::components::PopupId::FeedManage),
        ]
        .iter()
        .any(|id| self.application.mounted(id))
    }
}

#[cfg(test)]
#[path = "shell_library_tests.rs"]
mod tests;
