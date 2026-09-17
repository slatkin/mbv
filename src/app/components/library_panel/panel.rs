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
use tuirealm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::media_list::{
    LibrarySelectionOrigin, MediaListSurfaceInput, SelectionOrigin, SelectionSummary,
};
use crate::app::components::mouse::gesture::{ClickModifier, MouseGesture, MouseGestureState};
use crate::app::components::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::list_pane_width::normalize_list_pane_width;
use crate::app::render::wide_hero_fits;

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
    #[cfg_attr(not(test), allow(dead_code))]
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
pub struct LibraryPanel {
    owners: LibraryOwners,
    focused: bool,
    /// The main Selector row's locally resolved hovered pill, independent of
    /// selection and the other pill surfaces owned by this panel.
    hovered_selector: Option<usize>,
    /// The Wide hero provider-link label under the pointer, if any.
    hovered_link: Option<usize>,
    painted_link_urls: Vec<String>,
    /// Session-only Wide hero list-pane width override (shell-direct push;
    /// `None` = default ratio). Forwarded into the wide skeleton's shared
    /// split; never stored clamped.
    list_pane_width: Option<u16>,
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
            hovered_link: None,
            painted_link_urls: Vec::new(),
            list_pane_width: None,
            hits: SkeletonHits::default(),
            pill_windows: SkeletonPillWindows::default(),
            wide_geometry: None,
            narrow_geometry: None,
            painted_area: None,
            split: None,
            split_gestures: MouseGestureState::new(),
            gestures: MouseGestureState::new(),
            image_paint: None,
            deferred_msg: None,
            focused_summary: None,
            hero_overlay_open: false,
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

    /// Losing mouse eligibility mid-drag (overlay mount, mode change) clears
    /// the split gesture state before the next delivery — the same reset
    /// the former boundary applied while it owned the gesture —
    /// so no stale width can be emitted after eligibility ends, and a drag
    /// without a fresh press stays inert once eligibility returns.
    pub(in crate::app) fn sync_mouse_eligibility(&mut self, eligible: bool) {
        if !eligible {
            self.split_gestures = MouseGestureState::new();
        }
    }

    /// The painted split's gap rect, for the width-resolution test path.
    #[cfg(test)]
    pub(in crate::app) fn test_split_gap(&self) -> Option<ratatui::layout::Rect> {
        self.split.as_ref().map(|split| split.gap)
    }

    /// The split geometry the last painted Wide frame retained: the gap, the
    /// content-area origin, the content width and the current list-pane
    /// width (the facts the former shell-side gesture carried).
    #[cfg(test)]
    pub(in crate::app) fn test_split(&self) -> Option<(ratatui::layout::Rect, u16, u16, u16)> {
        self.split.as_ref().map(|split| {
            (
                split.gap,
                split.pane_origin_x,
                split.content_width,
                split.width,
            )
        })
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

    /// The Workspace selector row's retained hit regions (task 8.4: TV's
    /// season pills), for the pill-click test path.
    #[cfg(test)]
    pub(in crate::app) fn test_workspace_selector_hits(
        &self,
    ) -> &crate::app::components::mouse::hit::HitRegions<usize> {
        &self.hits.workspace_selector
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
                left_area: wide.list_area,
                selected_item_rect: wide.selected,
                selector_tabs: Vec::new(),
            };
        }
        if let Some(narrow) = self.narrow_geometry.as_ref() {
            return crate::app::layout::PaintedRowGeometry {
                left_area: narrow.list_area,
                selected_item_rect: narrow.selected,
                selector_tabs: Vec::new(),
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

    /// Test seam: forget the last wheel gesture so the next tick's wheel is
    /// recognized despite the 30 ms burst throttle (tests drive one wheel
    /// step per tick without sleeping through the throttle window).
    #[cfg(test)]
    pub(in crate::app) fn test_reset_wheel_throttle(&mut self) {
        self.gestures.reset_for_test();
    }

    #[cfg(test)]
    pub(in crate::app) fn test_hero_scroll_offset(&self) -> usize {
        self.owners
            .active_key()
            .and_then(|key| self.owners.get(key))
            .map(|owner| owner.hero_scroll_offset())
            .unwrap_or_default()
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

    // ── Event interpretation ─────────────────────────────────────────────

    /// The list slot's row-flow rect from the last painted frame, when one
    /// painted.
    fn list_rect(&self) -> Option<ratatui::layout::Rect> {
        self.wide_geometry
            .as_ref()
            .map(|geometry| geometry.list_area)
            .or(self
                .narrow_geometry
                .as_ref()
                .map(|geometry| geometry.list_area))
    }

    /// The hero pane's rect from the last painted Wide frame, when one
    /// painted.
    fn hero_pane_rect(&self) -> Option<ratatui::layout::Rect> {
        self.wide_geometry.as_ref().map(|geometry| geometry.hero)
    }

    fn can_open_hero_overlay(&mut self) -> bool {
        self.owners.active_mut().is_some_and(|owner| {
            !owner.inline_search_active()
                && (owner.hero_overlay_available() || owner.hero_overlay_target_available())
        })
    }

    fn open_hero_from_browser(&mut self, at: Option<Position>) -> Option<Msg> {
        let click_message = at.and_then(|at| {
            self.slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(at)))
        });
        // Resolve the gate after the pointer click has moved the canonical
        // browser selection to its post-click target. If the gate loses, the
        // click message must survive: the component already mutated before
        // the parent made this decision.
        if !self.open_hero_overlay_for_active() {
            return click_message;
        }
        let claim = if at.is_some() {
            TerminalObserverEvent::MouseClaimed
        } else {
            TerminalObserverEvent::KeyClaimed
        };
        Some(Msg::TerminalEvent(claim))
    }

    fn slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        if let LibrarySlotEvent::HeroPane(MediaListSurfaceInput::Wheel { at, delta }) = event {
            if let Some(geometry) = self.wide_geometry.as_ref() {
                if let Some(rect) = geometry.overview_box {
                    if rect.contains(at)
                        && geometry.overview_content_length > geometry.overview_viewport
                    {
                        let max = geometry.overview_content_length - geometry.overview_viewport;
                        return self.owners.active_mut().map(|owner| {
                            owner.hero_scroll(delta as i16, max);
                            Msg::TerminalEvent(
                                crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
                            )
                        });
                    }
                }
            }
            // Otherwise preserve the owner's existing HeroPane behavior.
        }
        let is_wheel = matches!(
            event,
            LibrarySlotEvent::List(MediaListSurfaceInput::Wheel { .. })
        );
        let result = self
            .owners
            .active_mut()
            .and_then(|owner| owner.on_slot_event(event));
        if is_wheel {
            self.defer_position_report();
        }
        result
    }

    /// The panel's deferred resting-scroll position report (design D8): the
    /// reached position also carries the owner's pagination reach, so a
    /// window-only viewport step — no cursor echo — still persists and feeds
    /// `maybe_fetch_next_page` through here.
    fn defer_position_report(&mut self) {
        if let (Some(key @ LibraryKey::Service { .. }), Some((index, scroll))) = (
            self.owners.active_key().cloned(),
            self.owners
                .active_mut()
                .and_then(|owner| owner.scroll_position()),
        ) {
            let pagination_index = self
                .owners
                .active_mut()
                .and_then(|owner| owner.viewport_pagination_index());
            self.deferred_msg = Some(Msg::Shell(ShellRequest::LibraryScroll {
                key,
                index,
                scroll,
                pagination_index,
            }));
        }
    }

    /// Whether this chord is one of the list viewport chords (design D6/D7):
    /// the page step (`PgUp`/`PgDn`) and the one-row step (`Ctrl+e`/
    /// `Ctrl+y`). Delivery bookkeeping only — the owner interprets the chord
    /// and the router keeps precedence; the panel defers the position report
    /// after a consumed step exactly as it does for a wheel step.
    fn is_viewport_chord(key: &KeyEvent) -> bool {
        match key.code {
            tuirealm::event::Key::PageUp | tuirealm::event::Key::PageDown => {
                key.modifiers.is_empty()
            }
            tuirealm::event::Key::Char('e' | 'y') => key.modifiers == KeyModifiers::CONTROL,
            _ => false,
        }
    }

    /// The split drag's resolved message: a live-only `ResizeListPaneLive`
    /// with the pointer-resolved width, identical to the boundary gesture
    /// this moved in from.
    fn split_gesture_msg(
        &mut self,
        gesture: MouseGesture,
        at: ratatui::layout::Position,
    ) -> Option<Msg> {
        let split = self.split.as_ref()?;
        match gesture {
            // Press-and-release without motion changes nothing.
            MouseGesture::Click { .. } if split.gap.contains(at) => None,
            // A recognized `Drag` implies an armed press inside the gap, so
            // every drag resolves -- tracking necessarily continues outside
            // the gap once the pointer leaves it.
            MouseGesture::Drag { to, .. } => {
                let width = normalize_list_pane_width(
                    Some(split.pane_origin_x.saturating_sub(to.x)),
                    split.content_width,
                )
                .unwrap_or(split.width);
                if width == split.width {
                    return None;
                }
                self.split.as_mut()?.width = width;
                Some(Msg::Shell(ShellRequest::ResizeListPaneLive(width)))
            }
            // Live-only: there is nothing to persist, so `DragEnd` is a no-op.
            _ => None,
        }
    }

    /// Interpret one non-gap mouse event against the panel's own painted
    /// slot geometry: pill rows resolve their slot events first, then the
    /// list slot delegates the normalized row-local input.
    fn surface_gesture(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let gesture = self.gestures.recognize(mouse)?;
        let at = match gesture {
            MouseGesture::Click { at, .. }
            | MouseGesture::DoubleClick(at)
            | MouseGesture::RightClick(at)
            | MouseGesture::Scroll { at, .. } => at,
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => return None,
        };
        // Painted pill rows first: the pill painted under the pointer in the
        // latest frame is the one that resolves (ADR 0024).
        if let Some(&index) = self.hits.selector.resolve(at) {
            return self.slot_event(LibrarySlotEvent::SelectorPicked(index));
        }
        if let Some(&index) = self.hits.controls.resolve(at) {
            return self.slot_event(LibrarySlotEvent::ControlPicked(index));
        }
        if let Some(&index) = self.hits.workspace_selector.resolve(at) {
            return self.slot_event(LibrarySlotEvent::WorkspaceSelectorPicked(index));
        }
        // Link labels are ordinary painted text, but the panel retains their
        // valid URL geometry and owns the click effect request.
        if let MouseGesture::Click { .. } = gesture {
            if let Some(&index) = self.hits.links.resolve(at) {
                if let Some(url) = self
                    .painted_link_urls
                    .get(index)
                    .cloned()
                    .and_then(|url| super::overview_box::sanitize_url(&url).map(str::to_owned))
                {
                    return Some(Msg::Shell(ShellRequest::OpenUrl(url)));
                }
            }
        }
        // The hero pane's own input (e.g. the Workspace box's episode rows)
        // resolves next: it is painted separately from the Browser pane's
        // list slot, and the owner decides what inside it it claims.
        let inside_hero = self.hero_pane_rect().is_some_and(|rect| rect.contains(at));
        if inside_hero {
            return match gesture {
                MouseGesture::Click { at, modifier } => {
                    let input = match modifier {
                        ClickModifier::Ctrl => MediaListSurfaceInput::ToggleClick(at),
                        ClickModifier::Shift => MediaListSurfaceInput::RangeClick(at),
                        ClickModifier::None => MediaListSurfaceInput::Click(at),
                    };
                    self.slot_event(LibrarySlotEvent::HeroPane(input))
                }
                MouseGesture::DoubleClick(at) => self.slot_event(LibrarySlotEvent::HeroPane(
                    MediaListSurfaceInput::DoubleClick(at),
                )),
                MouseGesture::RightClick(at) => self.slot_event(LibrarySlotEvent::HeroPane(
                    MediaListSurfaceInput::ContextClick(at),
                )),
                MouseGesture::Scroll { at, delta } => {
                    self.slot_event(LibrarySlotEvent::HeroPane(MediaListSurfaceInput::Wheel {
                        at,
                        delta,
                    }))
                }
                _ => None,
            };
        }
        let inside_list = self.list_rect().is_some_and(|rect| rect.contains(at));
        match gesture {
            MouseGesture::Click { at, modifier } if inside_list => {
                let input = match modifier {
                    ClickModifier::Ctrl => MediaListSurfaceInput::ToggleClick(at),
                    ClickModifier::Shift => MediaListSurfaceInput::RangeClick(at),
                    ClickModifier::None => MediaListSurfaceInput::Click(at),
                };
                self.slot_event(LibrarySlotEvent::List(input))
            }
            MouseGesture::DoubleClick(at) if inside_list => {
                if self.narrow_geometry.is_some() {
                    if let Some(message) = self.open_hero_from_browser(Some(at)) {
                        return Some(message);
                    }
                }
                self.slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
                    at,
                )))
            }
            MouseGesture::RightClick(at) if inside_list => self.slot_event(LibrarySlotEvent::List(
                MediaListSurfaceInput::ContextClick(at),
            )),
            MouseGesture::Scroll { at, delta } if inside_list => {
                self.slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
                    at,
                    delta,
                }))
            }
            _ => None,
        }
    }

    fn overlay_gesture(&mut self, mouse: &MouseEvent, at: Position) -> Option<Msg> {
        let geometry = self.overlay_geometry.as_ref()?;
        let gesture = self.gestures.recognize(mouse)?;
        if let Some(&index) = self.hits.workspace_selector.resolve(at) {
            return self.slot_event(LibrarySlotEvent::WorkspaceSelectorPicked(index));
        }
        if let MouseGesture::Click { .. } = gesture {
            if let Some(&index) = self.hits.links.resolve(at) {
                if let Some(url) = self
                    .painted_link_urls
                    .get(index)
                    .cloned()
                    .and_then(|url| super::overview_box::sanitize_url(&url).map(str::to_owned))
                {
                    return Some(Msg::Shell(ShellRequest::OpenUrl(url)));
                }
            }
        }
        if geometry
            .hero
            .workspace
            .is_some_and(|(panel, _)| panel.contains(at))
        {
            return match gesture {
                MouseGesture::Click { at, modifier } => {
                    let input = match modifier {
                        ClickModifier::Ctrl => MediaListSurfaceInput::ToggleClick(at),
                        ClickModifier::Shift => MediaListSurfaceInput::RangeClick(at),
                        ClickModifier::None => MediaListSurfaceInput::Click(at),
                    };
                    self.slot_event(LibrarySlotEvent::HeroPane(input))
                }
                MouseGesture::DoubleClick(at) => self.slot_event(LibrarySlotEvent::HeroPane(
                    MediaListSurfaceInput::DoubleClick(at),
                )),
                MouseGesture::RightClick(at) => self.slot_event(LibrarySlotEvent::HeroPane(
                    MediaListSurfaceInput::ContextClick(at),
                )),
                MouseGesture::Scroll { at, delta } => {
                    self.slot_event(LibrarySlotEvent::HeroPane(MediaListSurfaceInput::Wheel {
                        at,
                        delta,
                    }))
                }
                _ => None,
            };
        }
        if matches!(gesture, MouseGesture::DoubleClick(_)) {
            let result = self
                .owners
                .active_mut()
                .map(|owner| owner.activate_hero_selection())
                .unwrap_or(LeafKeyResult::Unhandled);
            return match result {
                // A pointer gesture always claims as mouse. Preserve a
                // destination request, but never leak a keyboard claim from
                // an owner's legacy leaf disposition.
                LeafKeyResult::Consumed(Some(Msg::TerminalEvent(_))) => {
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
                LeafKeyResult::Consumed(message) => message.or(Some(Msg::TerminalEvent(
                    TerminalObserverEvent::MouseClaimed,
                ))),
                LeafKeyResult::Unhandled => {
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
            };
        }
        Some(Msg::TerminalEvent(
            crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
        ))
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            let at = Position::new(mouse.column, mouse.row);
            self.hovered_selector = self.hits.selector.resolve(at).copied();
            self.hovered_link = self.hits.links.resolve(at).copied();
            return None;
        }
        // The panel resolves only geometry it painted; an unpainted frame
        // (no migrated owner) claims nothing.
        let painted_area = self.painted_area?;
        let at = Position::new(mouse.column, mouse.row);
        if !painted_area.contains(at) {
            return None;
        }
        // The overlay owns the Library pane's current-frame gesture. A
        // backdrop click dismisses and is consumed; covered browser geometry
        // is never replayed into the list.
        if self.hero_overlay_open {
            if !matches!(mouse.kind, MouseEventKind::Moved)
                && !self
                    .overlay_geometry
                    .as_ref()
                    .is_some_and(|geometry| geometry.frame.contains(at))
            {
                self.dismiss_hero_overlay();
                return Some(Msg::TerminalEvent(
                    crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
                ));
            }
            if self
                .overlay_geometry
                .as_ref()
                .is_some_and(|geometry| geometry.frame.contains(at))
            {
                return self.overlay_gesture(mouse, at);
            }
        }
        match mouse.kind {
            // A left press inside the painted gap arms only the split drag;
            // a press outside never arms it, so pane gestures are untouched.
            MouseEventKind::Down(MouseButton::Left) if self.split_hit(at) => self
                .split_gestures
                .recognize(mouse)
                .and_then(|gesture| self.split_gesture_msg(gesture, at)),
            // A recognized `Drag` implies an armed press: whichever
            // recognizer armed it consumes the continuation, so the split
            // drag tracks outside the gap and pane gestures never see a
            // gap-armed drag.
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(msg) = self
                    .split_gestures
                    .recognize(mouse)
                    .and_then(|gesture| self.split_gesture_msg(gesture, at))
                {
                    return Some(msg);
                }
                self.surface_gesture(mouse)
            }
            // Release closes whichever gesture armed; a gap-armed release is
            // a live-only no-op and a surface-armed release claims nothing.
            MouseEventKind::Up(MouseButton::Left) => {
                let _ = self.split_gestures.recognize(mouse);
                self.surface_gesture(mouse)
            }
            _ => self.surface_gesture(mouse),
        }
    }

    /// Whether the painted split's gap claims `at`.
    fn split_hit(&self, at: Position) -> bool {
        self.split
            .as_ref()
            .is_some_and(|split| split.gap.contains(at))
    }

    /// Forget the last painted frame's geometry: the panel resolves only
    /// geometry it painted. `view` calls this before painting, so an unpainted
    /// frame (no migrated owner) and a breakpoint transition both leave only
    /// the current frame's geometry behind.
    fn reset_frame(&mut self) {
        self.hits = SkeletonHits::default();
        self.wide_geometry = None;
        self.narrow_geometry = None;
        self.overlay_geometry = None;
        self.painted_area = None;
        self.split = None;
        self.image_paint = None;
        self.painted_link_urls.clear();
    }
}

#[path = "panel_view.rs"]
mod panel_view;

#[cfg(test)]
#[path = "panel_tests.rs"]
mod panel_tests;
