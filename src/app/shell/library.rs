use super::Model;
use super::PanelMode;
use mbv_ui_msg::ComponentId;

impl Model {
    /// The sync pass's focus step (design D3 of `add-gui-front-end`): the
    /// core focus stack derives the active destination from the Panel focus
    /// in effect and the overlay stack, and [`Model::apply_focus_stack`]
    /// applies it to `TuiRealm`. The pass calls the step here — after every
    /// mount/projection step above has settled the mounted set, and before
    /// the mouse-eligibility pass below derives off the same active
    /// destination.
    pub(in crate::app) fn sync_active_destination(&mut self) {
        self.apply_focus_stack();
    }

    /// The active library surface is always the mounted Library panel.
    pub(in crate::app) fn active_surface_id() -> ComponentId {
        ComponentId::Library
    }

    /// Whether the right-panel library destination is painted this frame.
    /// The queue-only mode (including the narrow mini view) hides the library
    /// entirely, so its mounted destination must not paint over the queue that
    /// now owns the whole frame.
    pub(in crate::app) fn library_panel_visible(&self) -> bool {
        self.app.effective_panel_mode() != PanelMode::QueueOnly
    }

    /// ADR 0024 D2: the mouse-eligible component set for the current frame, a
    /// three-rung ladder derived from the active mounted surfaces the
    /// active-destination pass uses (no second "did I paint" ledger).
    pub(in crate::app) fn mouse_eligible_ids(&self) -> Vec<ComponentId> {
        use mbv_ui_msg::{ModalId, OverlayId, PopupId};

        // Rung 1: a mounted blocking overlay/modal is eligible alone.
        const BLOCKING: &[ComponentId] = &[
            ComponentId::Overlay(OverlayId::ContextMenu),
            ComponentId::Modal(ModalId::Confirm),
            ComponentId::Modal(ModalId::DaemonLost),
            ComponentId::Modal(ModalId::SavePlaylist),
            ComponentId::Modal(ModalId::TwitchUrl),
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
        if let Some(id) = mbv_components::UiRootComponent::overlay_ids()
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
        let child = Self::active_surface_id();
        if self.library_panel_visible() && self.application.mounted(&child) {
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
    pub(in crate::app) fn queue_boundary_mouse_eligible(&self) -> bool {
        self.app.effective_panel_mode() == PanelMode::Both && self.panel_mouse_eligible()
    }

    pub(in crate::app) fn sync_queue_boundary(&mut self) {
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
                    Box::new(mbv_components::QueueBoundaryComponent::new()),
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
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(boundary) = comp
                .as_any_mut()
                .downcast_mut::<mbv_components::QueueBoundaryComponent>()
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
    pub(in crate::app) fn sync_mouse_subscriptions(&mut self) {
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
                .unsubscribe(&anchor, mbv_components::mouse_event_clause());
        }
        self.mouse_subscribed.clear();
        for id in eligible {
            if self.application.mounted(&id)
                && self
                    .application
                    .subscribe(&id, mbv_components::mouse_sub())
                    .is_ok()
            {
                self.mouse_subscribed.insert(id);
            }
        }
    }

    /// Whether any sidebar, modal, or popup overlay is mounted. The
    /// focus-relevant guards consult this so a mounted overlay that just took
    /// focus is never treated as absent; the overlay identities come from the
    /// focus seam's one list (`focus::overlay_surface_ids`).
    pub(in crate::app) fn overlay_holds_focus(&self) -> bool {
        super::focus::overlay_surface_ids()
            .iter()
            .any(|id| self.application.mounted(id))
    }
}

#[cfg(test)]
mod tests;
