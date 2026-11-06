//! The Client core focus stack (design D3, spec `client-interaction-core`).
//!
//! One focus authority: which surfaces are mounted, which panel side holds
//! focus, and which overlay sits on top. Focus is derived, never recorded per
//! overlay: the topmost overlay holds focus while one is open, and with an
//! empty overlay stack the panel-focus surface holds it. Dismissing any
//! overlay therefore restores the previous focus deterministically,
//! independent of the order surfaces were mounted in.

use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::ComponentId;

/// The mounted surfaces, the Panel focus, and the overlay stack, keyed by
/// `ComponentId` (design D3).
///
/// `overlays` is bottom-to-top; `mounted` preserves mount order for
/// deterministic iteration. A Client applies [`FocusStack::focused`] after
/// its own synchronisation pass and never decides focus by itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusStack {
    mounted: Vec<ComponentId>,
    panel_focus: PanelFocus,
    overlays: Vec<ComponentId>,
    focus_hold: bool,
}

impl FocusStack {
    /// An empty stack whose Panel focus is the default (`PanelFocus::Library`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `id` mounted. Mounting an already-mounted surface is a no-op,
    /// and its mount-order position is kept.
    pub fn mount(&mut self, id: ComponentId) {
        if !self.is_mounted(&id) {
            self.mounted.push(id);
        }
    }

    /// Marks `id` unmounted and drops it from the overlay stack, so an
    /// unmounted overlay can never resolve as focused. Returns whether `id`
    /// was mounted.
    pub fn umount(&mut self, id: &ComponentId) -> bool {
        let was_mounted = self.remove_mounted(id);
        self.overlays.retain(|overlay| overlay != id);
        was_mounted
    }

    /// Whether `id` is mounted.
    #[must_use]
    pub fn is_mounted(&self, id: &ComponentId) -> bool {
        self.mounted.contains(id)
    }

    /// The mounted surfaces in mount order.
    #[must_use]
    pub fn mounted(&self) -> &[ComponentId] {
        &self.mounted
    }

    /// Mounts `id` (if needed) and pushes it onto the overlay stack unless it
    /// is already stacked, where its position is kept.
    pub fn push_overlay(&mut self, id: ComponentId) {
        self.mount(id.clone());
        if !self.overlays.contains(&id) {
            self.overlays.push(id);
        }
    }

    /// Dismisses the overlay `id`: removes it from the overlay stack and
    /// unmounts it. Returns whether `id` was on the overlay stack.
    pub fn dismiss(&mut self, id: &ComponentId) -> bool {
        let Some(position) = self.overlays.iter().position(|overlay| overlay == id) else {
            return false;
        };
        self.overlays.remove(position);
        self.remove_mounted(id);
        true
    }

    /// The overlay stack, bottom-to-top.
    #[must_use]
    pub fn overlay_stack(&self) -> &[ComponentId] {
        &self.overlays
    }

    /// Records the Panel focus. It decides the focused surface only while no
    /// overlay is on the stack.
    pub fn set_panel_focus(&mut self, focus: PanelFocus) {
        self.panel_focus = focus;
    }

    /// The recorded Panel focus.
    #[must_use]
    pub fn panel_focus(&self) -> PanelFocus {
        self.panel_focus
    }

    /// Holds focus off every surface: `focused` resolves to `None` until
    /// [`FocusStack::release_focus`] clears the hold. This is the state the
    /// prefix-armed capture cannot otherwise represent (design D6 of change
    /// `extract-client-core`): armed capture withholds the keyboard from every
    /// surface with no overlay open, so the hold is not an overlay and no
    /// dismissal can lift it.
    pub fn hold_focus(&mut self) {
        self.focus_hold = true;
    }

    /// Clears the focus hold; `focused` derives from the stack again.
    pub fn release_focus(&mut self) {
        self.focus_hold = false;
    }

    /// Whether the focus hold is set.
    #[must_use]
    pub fn focus_held(&self) -> bool {
        self.focus_hold
    }

    /// The surface that holds focus now: the topmost overlay while one is
    /// open, else the Panel-focus surface (`ComponentId::Queue` or
    /// `ComponentId::Library`). An unmounted Panel-focus surface resolves to
    /// `None`, except that Panel focus on the Library side falls back to
    /// `ComponentId::UiRoot` while the Library panel is not mounted, matching
    /// the queue-only presentation. A set hold (`hold_focus`) takes focus off
    /// every surface and resolves to `None` whatever the stack holds.
    #[must_use]
    pub fn focused(&self) -> Option<ComponentId> {
        if self.focus_hold {
            return None;
        }
        if let Some(top) = self.overlays.last() {
            return Some(top.clone());
        }
        match self.panel_focus {
            PanelFocus::Queue => self.mounted_surface(&ComponentId::Queue),
            PanelFocus::Library => self
                .mounted_surface(&ComponentId::Library)
                .or_else(|| self.mounted_surface(&ComponentId::UiRoot)),
        }
    }

    fn mounted_surface(&self, id: &ComponentId) -> Option<ComponentId> {
        self.is_mounted(id).then(|| id.clone())
    }

    fn remove_mounted(&mut self, id: &ComponentId) -> bool {
        let Some(position) = self.mounted.iter().position(|mounted| mounted == id) else {
            return false;
        };
        self.mounted.remove(position);
        true
    }
}
