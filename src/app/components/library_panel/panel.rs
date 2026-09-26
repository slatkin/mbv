//! The mounted Library panel (task 5.9, design D2): the library area's one
//! Interactive Component and event boundary. It owns the map of embedded
//! content owners keyed by [`LibraryKey`](super::owner::LibraryKey) (retained
//! while their library is in the catalog), the library area's framework
//! focus, the mouse subscription's event interpretation, the skeleton's
//! retained slot hit geometry, and the Wide split-boundary drag gesture
//! (moved into this parent). Slot events are typed
//! `Msg`s routed to the active owner, which translates them into its
//! existing `Msg`s — no raw event or coordinate crosses to the shell for
//! re-resolution.
//!

use ratatui::layout::Position;
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::media_list::{
    LibrarySelectionOrigin, MediaListSurfaceInput, SelectionOrigin, SelectionSummary,
};
use crate::app::components::mouse::gesture::{ClickModifier, MouseGesture, MouseGestureState};
use crate::app::components::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::render::wide_hero_fits;
use crate::app::state::list_pane_width::normalize_list_pane_width;

use super::content::{HeroImageState, PanelHeroImagePaint};
use super::hero::HeroContentData;
use super::hero_composition::HeroCompositionGeometry;
use super::narrow::render_narrow_skeleton;
use super::owner::{LibraryContentOwner, LibraryKey, LibraryOwners, LibrarySlotEvent};
use super::wide::{render_wide_skeleton, SkeletonHits, SkeletonPillWindows, WideSkeletonGeometry};
use crate::app::components::inline_search::InlineSearchHost;

/// The painted split's pointer→width resolution inputs, shared by the drag
/// gesture's arming and resolution (the same facts the old
/// the former boundary carried, now derived from the panel's
/// own painted skeleton geometry).
#[derive(Clone, Debug)]
struct OverlayGeometry {
    #[cfg(test)]
    pane: ratatui::layout::Rect,
    frame: ratatui::layout::Rect,
    hero: HeroCompositionGeometry,
}

#[derive(Clone, Copy, Debug)]
struct SplitGeometry {
    /// The shared `WIDE_HERO_PANE_GAP` gutter between the panes.
    gap: ratatui::layout::Rect,
    /// Right edge of the panel's content area; this minus the pointer
    /// column is the resolved list-pane width.
    pane_origin_x: u16,
    /// The content area's width, used to clamp the resolved width.
    content_width: u16,
    /// The list-pane width the current override (or default ratio) paints.
    width: u16,
}

/// The Library panel: owner map, focus, painted-slot geometry, and the Wide
/// split drag. Content owners are plain types beneath it (design D2); they
/// are never mounted, focused, subscribed, or given a `ComponentId`.
#[expect(
    clippy::struct_excessive_bools,
    reason = "focused, split_changed, hero_overlay_open, and mini_view_hero_auto_open are independent panel interaction/lifecycle flags, not one mode (design analysis, issue #804)"
)]
pub struct LibraryPanel {
    owners: LibraryOwners,
    focused: bool,
    /// The main Selector row's locally resolved hovered pill, independent of
    /// selection and the other pill surfaces owned by this panel.
    hovered_selector: Option<usize>,
    #[cfg(test)]
    projected_selector_markers: Vec<bool>,
    /// The Wide hero provider-link label under the pointer, if any.
    hovered_link: Option<usize>,
    painted_link_urls: Vec<String>,
    /// Session-only Wide hero list-pane width override (shell-direct push;
    /// `None` = default ratio). Forwarded into the wide skeleton's shared
    /// split; never stored clamped.
    list_pane_width: Option<u16>,
    /// Terminal row count, pushed each sync pass: the compact hero caps
    /// (artwork 15, overview 5) apply at 50 terminal rows or fewer.
    terminal_height: u16,
    // The last painted frame's retained geometry (ADR 0024: the mounted
    // parent resolves only geometry it painted).
    hits: SkeletonHits,
    /// The pill rows' sticky overflow windows: session state (unlike the
    /// per-frame `hits`), revalidated by the painter against each frame's
    /// pills, so a pointer selection of a painted pill never slides its bar.
    pill_windows: SkeletonPillWindows,
    wide_geometry: Option<WideSkeletonGeometry>,
    narrow_geometry: Option<WideSkeletonGeometry>,
    painted_area: Option<ratatui::layout::Rect>,
    split: Option<SplitGeometry>,
    /// The split boundary's private gesture state: a left press inside the
    /// painted gap arms a drag; pane presses never arm it (the
    /// legacy split-boundary rule this gesture moved in from).
    split_gestures: MouseGestureState,
    /// Whether the current split gesture changed the resolved width. Used to
    /// emit one persistence request at drag end, never for intermediate moves.
    split_changed: bool,
    /// The library surface's own gesture recognizer for slot events.
    gestures: MouseGestureState,
    /// The projected hero image paint the last view retained (task 5.10,
    /// design D9): the shell takes it after `view` returns and paints the
    /// projected protocol into the reserved box (the same defer-the-pixel-
    /// paint seam every destination component uses).
    image_paint: Option<PanelHeroImagePaint>,
    /// A wheel also needs to persist the owner's resolved scroll. Queue that
    /// secondary shell intent while returning the owner's cursor echo.
    deferred_msg: Option<Msg>,
    focused_summary: Option<SelectionSummary>,
    hero_overlay_open: bool,
    /// Whether the current overlay was opened automatically for a compact
    /// flat-episode hero. This remains armed after an explicit dismissal so
    /// the mini-view sync pass does not immediately reopen the overlay.
    /// Explicit series overlays are never closed by that pass.
    mini_view_hero_auto_open: bool,
    overlay_geometry: Option<OverlayGeometry>,
}

impl From<LibraryKey> for LibrarySelectionOrigin {
    fn from(key: LibraryKey) -> Self {
        match key {
            LibraryKey::Home => Self::Home,
            LibraryKey::Feeds => Self::Feeds,
            key @ LibraryKey::Service { .. } => Self::Service(key),
        }
    }
}

impl From<LibrarySelectionOrigin> for LibraryKey {
    fn from(origin: LibrarySelectionOrigin) -> Self {
        match origin {
            LibrarySelectionOrigin::Home => Self::Home,
            LibrarySelectionOrigin::Feeds => Self::Feeds,
            LibrarySelectionOrigin::Service(key) => key,
        }
    }
}

impl LibraryPanel {
    pub fn new() -> Self {
        Self {
            owners: LibraryOwners::new(),
            focused: false,
            hovered_selector: None,
            #[cfg(test)]
            projected_selector_markers: Vec::new(),
            hovered_link: None,
            painted_link_urls: Vec::new(),
            list_pane_width: None,
            terminal_height: 0,
            hits: SkeletonHits::default(),
            pill_windows: SkeletonPillWindows::default(),
            wide_geometry: None,
            narrow_geometry: None,
            painted_area: None,
            split: None,
            split_gestures: MouseGestureState::new(),
            split_changed: false,
            gestures: MouseGestureState::new(),
            image_paint: None,
            deferred_msg: None,
            focused_summary: None,
            hero_overlay_open: false,
            mini_view_hero_auto_open: false,
            overlay_geometry: None,
        }
    }

    pub(in crate::app) fn take_deferred_msg(&mut self) -> Option<Msg> {
        self.deferred_msg.take()
    }

    // ── Shell-directed owner map (design D2) ─────────────────────────────

    /// Install or replace the content owner for `key`. The shell pushes
    /// owners addressed by `LibraryKey` as their destinations migrate.
    pub(in crate::app) fn insert_owner(
        &mut self,
        key: LibraryKey,
        owner: Box<dyn LibraryContentOwner>,
    ) {
        self.owners.insert(key, owner);
    }

    /// Whether an owner exists for `key` — the transitional branch's
    /// migrated-owner check.
    pub(in crate::app) fn has_owner(&self, key: &LibraryKey) -> bool {
        self.owners.has(key)
    }

    /// Drop every owner whose key is not in `live`: the catalog-retention
    /// rule (design D2, moved inside from the shell's
    /// `reconcile_destination_mounts`). An owner is retained while its
    /// library is in the catalog, so an inactive owner keeps cursor/scroll
    /// across tab changes.
    pub(in crate::app) fn retain_owners(&mut self, live: &[LibraryKey]) {
        self.owners.retain(live);
    }

    /// Point the panel at the active library's owner (the shell drives this
    /// from its tab resolution each sync pass).
    pub(in crate::app) fn set_active(&mut self, key: Option<LibraryKey>) {
        let identity_changed = self.owners.active_key() != key.as_ref();
        if identity_changed {
            self.dismiss_hero_overlay();
            self.mini_view_hero_auto_open = false;
            if let Some(previous) = self.owners.active_key().cloned() {
                if let Some(owner) = self.owners.get_mut(&previous) {
                    owner.clear_selection();
                }
            }
        }
        self.owners.set_active(key.clone());
        if let Some(key) = key {
            let origin = LibrarySelectionOrigin::from(key);
            if let Some(owner) = self.owners.active_mut() {
                owner.set_selection_origin(SelectionOrigin::Library(origin));
            }
        }
        self.focused_summary = self
            .owners
            .active_mut()
            .and_then(|owner| owner.selection_summary());
    }

    /// Record the session-only Wide split width override for the next `view`.
    /// Pushed each sync pass by the shell beside the other per-frame facts.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.list_pane_width = list_pane_width;
    }

    /// Record the terminal row count for the compact hero caps. Pushed each
    /// sync pass by the shell beside the other per-frame facts.
    pub(in crate::app) fn set_terminal_height(&mut self, terminal_height: u16) {
        self.terminal_height = terminal_height;
    }

    /// The hero image box the last painted frame reserved — the Library Hero
    /// overlay's while it is open, else the Wide Hero pane's. The shell's
    /// image projection encodes for the box that paints.
    pub(in crate::app) fn active_hero_image_box(&self) -> Option<(u16, u16)> {
        let paint = if self.hero_overlay_open {
            self.overlay_geometry
                .as_ref()
                .and_then(|g| g.hero.hero_image.as_ref())
        } else {
            self.wide_geometry
                .as_ref()
                .and_then(|g| g.hero_image.as_ref())
        };
        paint.map(|paint| (paint.area.width, paint.area.height))
    }

    /// Losing mouse eligibility mid-drag (overlay mount, mode change) clears
    /// the split gesture state before the next delivery — the same reset
    /// the former boundary applied while it owned the gesture —
    /// so no stale width can be emitted after eligibility ends, and a drag
    /// without a fresh press stays inert once eligibility returns.
    pub(in crate::app) fn sync_mouse_eligibility(&mut self, eligible: bool) {
        if !eligible {
            self.reset_split_gesture();
        }
    }

    /// The painted split's gap rect, for the width-resolution test path.
    #[cfg(test)]
    pub(in crate::app) fn test_split_gap(&self) -> Option<ratatui::layout::Rect> {
        self.split.as_ref().map(|split| split.gap)
    }

    /// The painted list slot's rect, for the breakpoint-transition test path.
    #[cfg(test)]
    pub(in crate::app) fn test_list_rect(&self) -> Option<ratatui::layout::Rect> {
        self.list_rect()
    }

    /// The Selector row's retained hit regions, for the pill-row test path.
    #[cfg(test)]
    pub(in crate::app) fn test_selector_hits(
        &self,
    ) -> &crate::app::components::mouse::hit::HitRegions<usize> {
        &self.hits.selector
    }

    #[cfg(test)]
    pub(in crate::app) fn test_hovered_selector(&self) -> Option<usize> {
        self.hovered_selector
    }

    #[cfg(test)]
    pub(in crate::app) fn test_link_hits(
        &self,
    ) -> &crate::app::components::mouse::hit::HitRegions<usize> {
        &self.hits.links
    }

    /// The last painted Wide skeleton geometry, for the panel-output test
    /// path.
    #[cfg(test)]
    pub(in crate::app) fn test_wide_geometry(&self) -> Option<WideSkeletonGeometry> {
        self.wide_geometry.clone()
    }

    /// The last painted frame's role rects, in the shared test-only
    /// `PaintedRowGeometry` shape the characterization helpers read (task
    /// 8.4: the panel owns the rects the deleted destination component used
    /// to publish; task 12.3: this no longer round-trips through the
    /// shell's legacy chrome geometry).
    #[cfg(test)]
    pub(in crate::app) fn test_painted_layout(&self) -> crate::app::layout::PaintedRowGeometry {
        if let Some(wide) = self.wide_geometry.as_ref() {
            return crate::app::layout::PaintedRowGeometry {
                selected_item_rect: wide.selected,
            };
        }
        if let Some(narrow) = self.narrow_geometry.as_ref() {
            return crate::app::layout::PaintedRowGeometry {
                selected_item_rect: narrow.selected,
            };
        }
        crate::app::layout::PaintedRowGeometry::default()
    }

    /// The last painted Narrow skeleton geometry (the shared
    /// [`WideSkeletonGeometry`] shape: the non-Wide panel is the Wide
    /// browser pane without a Hero), for the panel-output test path.
    #[cfg(test)]
    pub(in crate::app) fn test_narrow_geometry(&self) -> Option<WideSkeletonGeometry> {
        self.narrow_geometry.clone()
    }

    // ── Shell-directed owner projection (task 5.10, design D9) ───────────

    /// The active owner's current hero content data, for the shell's image
    /// projection.
    pub(in crate::app) fn active_hero_data(&mut self) -> Option<HeroContentData> {
        self.owners.active_mut().and_then(|owner| owner.hero_data())
    }

    /// Deliver the projection's image state to the active owner.
    pub(in crate::app) fn set_active_hero_image(&mut self, state: HeroImageState) {
        if let Some(owner) = self.owners.active_mut() {
            owner.set_hero_image(state);
        }
    }

    /// Clear the multi-selection of the owner named by `origin`, whether or
    /// not it is the active library (design D6: a clear intent carries the
    /// origin captured at invocation, never the dispatch-time focus).
    /// Returns whether an owner for `origin` is installed.
    pub(in crate::app) fn clear_selection_for_origin(
        &mut self,
        origin: &LibrarySelectionOrigin,
    ) -> bool {
        let key = LibraryKey::from(origin.clone());
        let was_active = self.owners.active_key() == Some(&key);
        let Some(owner) = self.owners.get_mut(&key) else {
            return false;
        };
        owner.clear_selection();
        if was_active {
            self.focused_summary = owner.selection_summary();
        }
        true
    }

    pub(in crate::app) fn focused_summary(&mut self) -> Option<SelectionSummary> {
        self.focused_summary = self
            .owners
            .active_mut()
            .and_then(|owner| owner.selection_summary());
        self.focused_summary.clone()
    }

    /// Mutably borrow the owner installed for `key` (the shell's
    /// destination-specific content pushes reach a typed owner through its
    /// `as_any_mut`).
    pub(in crate::app) fn owner_mut(
        &mut self,
        key: &LibraryKey,
    ) -> Option<&mut dyn LibraryContentOwner> {
        self.owners.get_mut(key)
    }

    /// The active owner's embedded Inline Search session, mutably — the
    /// shell's open/load/push/dismiss path resolves the host through it.
    pub(in crate::app) fn active_inline_search_session(
        &mut self,
    ) -> Option<&mut dyn InlineSearchHost> {
        self.owners.active_mut()?.inline_search_session()
    }

    /// The shared-borrow twin of [`LibraryPanel::active_inline_search_session`]
    /// for the shell's pure reads (is-open, selected result).
    pub(in crate::app) fn active_inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        let key = self.owners.active_key()?;
        self.owners.get(key)?.inline_search_session_ref()
    }

    /// The shared-borrow twin of [`LibraryPanel::owner_mut`], for the
    /// shell's pure reads.
    pub(in crate::app) fn owner(&self, key: &LibraryKey) -> Option<&dyn LibraryContentOwner> {
        self.owners.get(key)
    }

    /// Read the bounded launch identities from exactly one destination owner.
    /// The shell calls this only for the selected key during orderly teardown;
    /// inactive owners are never queried.
    pub(in crate::app) fn launch_snapshot(
        &self,
        key: &LibraryKey,
    ) -> Option<(
        Option<mbv_core::config::SelectorIdentity>,
        Option<mbv_core::config::LibraryItemIdentity>,
    )> {
        self.owners
            .get(key)
            .map(LibraryContentOwner::launch_snapshot)
    }

    /// Apply the one discrete startup launch re-anchor to exactly one active
    /// destination owner. The panel does not mirror the resulting local state.
    pub(in crate::app) fn launch_selector(
        &self,
        key: &LibraryKey,
        state: &mbv_core::config::TuiLaunchState,
    ) -> Option<super::owner::LaunchSelector> {
        self.owners
            .get(key)
            .and_then(|owner| owner.launch_selector(state))
    }

    pub(in crate::app) fn reanchor_launch_state(
        &mut self,
        key: &LibraryKey,
        state: &mbv_core::config::TuiLaunchState,
    ) -> bool {
        self.owners
            .get_mut(key)
            .is_some_and(|owner| owner.reanchor_launch_state(state))
    }

    /// Take the hero image paint the last view retained (the shell paints it
    /// right after `view` returns).
    pub(in crate::app) fn take_image_paint(&mut self) -> Option<PanelHeroImagePaint> {
        self.image_paint.take()
    }

    /// The painted panel's (list pane rect, selected row rect) for the
    /// context-menu anchor path — the panel's own last-painted geometry, the
    /// same painted-truth contract the old destination components'
    /// `menu_placement_geometry` served. Both skeletons save the Browser
    /// pane's panel rect in `list_panel` (the non-Wide panel *is* the Wide
    /// browser pane), so one expression covers every geometry.
    pub(in crate::app) fn menu_geometry(
        &self,
    ) -> Option<(ratatui::layout::Rect, Option<ratatui::layout::Rect>)> {
        self.wide_geometry
            .as_ref()
            .or(self.narrow_geometry.as_ref())
            .map(|geometry| (geometry.list_panel, geometry.selected))
    }
}

mod mouse;
mod overlay;
mod view;

#[cfg(test)]
mod tests;
