//! The mounted Library panel (task 5.9, design D2): the library area's one
//! Interactive Component and event boundary. It owns the map of embedded
//! content owners keyed by [`LibraryKey`](super::owner::LibraryKey) (retained
//! while their library is in the catalog), the library area's framework
//! focus, the mouse subscription's event interpretation, the skeleton's
//! retained slot hit geometry, and the Wide split-boundary drag gesture
//! (moved in from `WideHeroBoundaryComponent`). Slot events are typed
//! `Msg`s routed to the active owner, which translates them into its
//! existing `Msg`s — no raw event or coordinate crosses to the shell for
//! re-resolution.
//!
//! Transitional branch (task 5.9): the panel paints only when the active
//! library's owner has migrated; with zero migrated owners the old mounted
//! destination stays the surface and the panel claims nothing.

use ratatui::layout::Position;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use crate::app::components::media_list::RowLocalInput;
use crate::app::components::mouse::gesture::{MouseGesture, MouseGestureState};
use crate::app::components::msg::{Msg, ShellRequest};
use crate::app::components::UserEvent;
use crate::app::list_pane_width::normalize_list_pane_width;
use crate::app::render::wide_hero_fits;

use super::narrow::{render_narrow_skeleton, NarrowSkeletonGeometry};
use super::owner::{LibraryContentOwner, LibraryKey, LibraryOwners, LibrarySlotEvent};
use super::wide::{render_wide_skeleton, SkeletonHits, WideSkeletonGeometry};

/// The painted split's pointer→width resolution inputs, shared by the drag
/// gesture's arming and resolution (the same facts the old
/// `WideHeroBoundaryComponent::sync` carried, now derived from the panel's
/// own painted skeleton geometry).
#[derive(Clone, Copy, Debug)]
struct SplitGeometry {
    /// The shared `WIDE_HERO_PANE_GAP` gutter between the panes.
    gap: ratatui::layout::Rect,
    /// Left edge of the panel's content area; the pointer column minus this
    /// is the resolved list-pane width.
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
    /// Session-only Wide hero list-pane width override (shell-direct push;
    /// `None` = default ratio). Forwarded into the wide skeleton's shared
    /// split; never stored clamped.
    list_pane_width: Option<u16>,
    // The last painted frame's retained geometry (ADR 0024: the mounted
    // parent resolves only geometry it painted).
    hits: SkeletonHits,
    wide_geometry: Option<WideSkeletonGeometry>,
    narrow_geometry: Option<NarrowSkeletonGeometry>,
    painted_area: Option<ratatui::layout::Rect>,
    split: Option<SplitGeometry>,
    /// The split boundary's private gesture state: a left press inside the
    /// painted gap arms a drag; pane presses never arm it (the
    /// `WideHeroBoundaryComponent` rule this gesture moved in from).
    split_gestures: MouseGestureState,
    /// The library surface's own gesture recognizer for slot events.
    gestures: MouseGestureState,
}

impl LibraryPanel {
    pub fn new() -> Self {
        Self {
            owners: LibraryOwners::new(),
            focused: false,
            list_pane_width: None,
            hits: SkeletonHits::default(),
            wide_geometry: None,
            narrow_geometry: None,
            painted_area: None,
            split: None,
            split_gestures: MouseGestureState::new(),
            gestures: MouseGestureState::new(),
        }
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
        self.owners.set_active(key);
    }

    /// Record the session-only Wide split width override for the next `view`.
    /// Pushed each sync pass by the shell beside the other per-frame facts.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.list_pane_width = list_pane_width;
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

    // ── Event interpretation ─────────────────────────────────────────────

    /// The list slot's row-flow rect from the last painted frame, when one
    /// painted.
    fn list_rect(&self) -> Option<ratatui::layout::Rect> {
        self.wide_geometry
            .map(|geometry| geometry.list_area)
            .or(self
                .narrow_geometry
                .as_ref()
                .map(|geometry| geometry.list_area))
    }

    /// Route one already-normalized pointer input to the active owner's
    /// list. The owner performs the typed point resolution through its own
    /// carrier and translates the outcome into its `Msg`s.
    fn delegate_list_input(&mut self, input: RowLocalInput) -> Option<Msg> {
        self.slot_event(LibrarySlotEvent::List(input))
    }

    fn slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.owners
            .active_mut()
            .and_then(|owner| owner.on_slot_event(event))
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
            MouseGesture::Click(_) if split.gap.contains(at) => None,
            // A recognized `Drag` implies an armed press inside the gap, so
            // every drag resolves -- tracking necessarily continues outside
            // the gap once the pointer leaves it.
            MouseGesture::Drag { to, .. } => {
                let width = normalize_list_pane_width(
                    Some(to.x.saturating_sub(split.pane_origin_x)),
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
            MouseGesture::Click(at)
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
        let inside_list = self.list_rect().is_some_and(|rect| rect.contains(at));
        match gesture {
            MouseGesture::Click(at) if inside_list => {
                self.slot_event(LibrarySlotEvent::List(RowLocalInput::Click(at)))
            }
            MouseGesture::DoubleClick(at) if inside_list => {
                self.slot_event(LibrarySlotEvent::List(RowLocalInput::DoubleClick(at)))
            }
            MouseGesture::RightClick(at) if inside_list => {
                self.slot_event(LibrarySlotEvent::List(RowLocalInput::ContextClick(at)))
            }
            MouseGesture::Scroll { at, delta } if inside_list => {
                self.slot_event(LibrarySlotEvent::List(RowLocalInput::Wheel { at, delta }))
            }
            _ => None,
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Browse does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        // The panel resolves only geometry it painted; an unpainted frame
        // (no migrated owner) claims nothing.
        let painted_area = self.painted_area?;
        let at = Position::new(mouse.column, mouse.row);
        if !painted_area.contains(at) {
            return None;
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
        self.painted_area = None;
        self.split = None;
    }
}

impl Default for LibraryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LibraryPanel {
    fn view(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // Each frame's retained geometry is exactly what that frame painted
        // (ADR 0024): the reset also drops the other breakpoint's geometry, so
        // a Wide→Narrow resize leaves neither the vanished split's gap armed
        // nor the stale Wide list rect claiming clicks.
        self.reset_frame();
        let Some(owner) = self.owners.active_mut() else {
            return;
        };
        let mut content = owner.content();
        let mut hits = std::mem::take(&mut self.hits);
        // One breakpoint predicate (design D4): `wide_hero_fits` stays the
        // single Wide/Narrow choice; the panel drives the presentation
        // transition through the list's `set_presentation` inside each
        // skeleton.
        if wide_hero_fits(area) {
            if let Some(geometry) = render_wide_skeleton(
                frame,
                area,
                &mut content,
                self.focused,
                self.list_pane_width,
                &mut hits,
            ) {
                // The split gesture owns the gap columns it painted: the
                // gutter between the browser and hero panes, resolved
                // against the panel's own content area.
                let gap = ratatui::layout::Rect {
                    x: geometry.browser.right(),
                    y: area.y,
                    width: geometry.hero.x.saturating_sub(geometry.browser.right()),
                    height: area.height,
                };
                self.split = (gap.width > 0 && gap.height > 0).then_some(SplitGeometry {
                    gap,
                    pane_origin_x: area.x,
                    content_width: area.width,
                    width: geometry.browser.width,
                });
                self.wide_geometry = Some(geometry);
            }
        } else {
            let geometry =
                render_narrow_skeleton(frame, area, &mut content, self.focused, &mut hits);
            self.narrow_geometry = Some(geometry);
        }
        self.hits = hits;
        self.painted_area = Some(area);
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for LibraryPanel {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            // Keyboard interpretation for migrated owners lands with each
            // destination's conversion (tasks 5.11+): the router keeps
            // precedence, and the panel will translate resolved chords into
            // slot events. Until then the old destination components stay
            // the keyboard endpoints for their libraries.
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod panel_tests {
    use super::*;
    use crate::app::components::library_panel::content::{
        HeroContent, LibraryPanelContent, ListSlot, SelectorRow,
    };
    use crate::app::components::media_list::{
        MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation,
    };
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;
    use std::cell::RefCell;
    use std::rc::Rc;
    use tuirealm::event::{KeyModifiers, MouseButton};

    fn item(target: &str) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: target.into(),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    /// What a fixture owner did: every slot event and every post-event list
    /// selection, shared with the test through an `Rc` (the panel owns the
    /// owner, so the test reads the log, not the component).
    #[derive(Default)]
    struct FixtureLog {
        events: Vec<LibrarySlotEvent>,
        selections: Vec<Option<String>>,
    }

    /// A test-owned content owner: the minimal migrated owner the panel can
    /// host — a carrier-backed list slot plus a policy-shaped hero. It
    /// translates slot events exactly as a destination will ("as today":
    /// typed point resolution through its own carrier), proving the
    /// infrastructure without converting any destination.
    struct FixtureOwner {
        carrier: MediaListCarrier<String>,
        log: Rc<RefCell<FixtureLog>>,
    }

    impl FixtureOwner {
        fn new(log: Rc<RefCell<FixtureLog>>) -> Self {
            let mut carrier = MediaListCarrier::new(Presentation::Wide);
            carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
            Self { carrier, log }
        }
    }

    impl LibraryContentOwner for FixtureOwner {
        fn content(&mut self) -> LibraryPanelContent<'_> {
            LibraryPanelContent {
                selector: Some(SelectorRow {
                    pills: vec!["All".into(), "New".into()],
                    active: Some(0),
                }),
                controls: None,
                list: ListSlot::Media(&mut self.carrier),
                hero: Some(HeroContent {
                    facts: crate::app::components::library_panel::HeroFacts {
                        title: "Dune".into(),
                        meta_rows: vec!["2021".into()],
                        artwork: crate::app::components::library_panel::HeroArtwork {
                            shape: crate::app::components::library_panel::ArtworkShape::Landscape,
                            source: None,
                        },
                    },
                    overview: None,
                    workspace: None,
                }),
            }
        }

        fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
            let selection = match event {
                LibrarySlotEvent::List(input) => {
                    // Typed translation "as today": the owner resolves the
                    // pointer target through its own carrier, then delegates.
                    let target = match input {
                        RowLocalInput::Click(at)
                        | RowLocalInput::DoubleClick(at)
                        | RowLocalInput::ContextClick(at) => {
                            self.carrier.resolve_current_point(at).cloned()
                        }
                        _ => None,
                    };
                    if let Some(target) = &target {
                        self.carrier.select_target(target);
                    }
                    self.carrier.delegate(input, target);
                    self.carrier.selected_target().cloned()
                }
                _ => self.carrier.selected_target().cloned(),
            };
            let mut log = self.log.borrow_mut();
            log.events.push(event);
            log.selections.push(selection);
            // A consumed pointer gesture after a list mutation reports the
            // framework's claim marker (ADR 0024).
            Some(Msg::TerminalEvent(
                crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
            ))
        }
    }

    fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn draw_panel(panel: &mut LibraryPanel) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let area = Rect::new(0, 0, 120, 30);
        terminal.draw(|f| Component::view(panel, f, area)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn line_text(buf: &ratatui::buffer::Buffer, row: u16) -> String {
        (0..120).map(|x| buf[(x, row)].symbol()).collect::<String>()
    }

    /// The panel without a migrated owner claims nothing: no paint, no
    /// gesture, no slot event (the transitional branch).
    #[test]
    fn unpainted_panel_claims_nothing() {
        let mut panel = LibraryPanel::new();
        let buf = draw_panel(&mut panel);
        // Untouched: every cell still carries the empty terminal's style,
        // compared relatively so no raw colour primitive is named here.
        let untouched = buf[(0, 0)].bg;
        for y in 0..30 {
            for x in 0..120 {
                assert_eq!(buf[(x, y)].bg, untouched);
            }
        }
        assert_eq!(
            panel.on(&mouse_event(MouseEventKind::Down(MouseButton::Left), 4, 4)),
            None,
            "an unpainted panel claims no pointer input"
        );
        assert!(panel.test_split_gap().is_none());
    }

    /// The panel paints the active owner's content and retains its slot hit
    /// geometry: pills and list rows are on screen, and a pill click routes
    /// `SelectorPicked` to the active owner.
    #[test]
    fn panel_paints_the_active_owner_and_resolves_slot_events() {
        let log = Rc::new(RefCell::new(FixtureLog::default()));
        let mut panel = LibraryPanel::new();
        panel.set_active(Some(LibraryKey::Home));
        panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
        let buf = draw_panel(&mut panel);

        // Content painted: the selector pill and a list row.
        let pill = (0..30)
            .map(|row| (row, line_text(&buf, row)))
            .find(|(_, text)| text.contains("All"))
            .expect("the migrated owner's selector pill paints");
        assert!(
            (0..30).any(|row| line_text(&buf, row).contains("alpha")),
            "the migrated owner's list rows paint"
        );

        // A click on the painted pill routes SelectorPicked to the owner.
        let pill_x = (0..120)
            .find(|&x| buf[(x, pill.0)].symbol() == "A")
            .expect("pill text painted");
        let msg = panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            pill_x,
            pill.0,
        ));
        assert!(msg.is_some(), "a pill click claims the event");
        assert_eq!(
            log.borrow().events.last(),
            Some(&LibrarySlotEvent::SelectorPicked(0)),
            "the painted pill's index is the slot event"
        );
    }

    /// A list click resolves the row under the pointer through the owner's
    /// typed carrier and delegates it as today.
    #[test]
    fn list_click_delegates_to_the_active_owner() {
        let log = Rc::new(RefCell::new(FixtureLog::default()));
        let mut panel = LibraryPanel::new();
        panel.set_active(Some(LibraryKey::Home));
        panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
        let buf = draw_panel(&mut panel);

        // Find the second row's text and click it.
        let row = (0..30)
            .find(|&row| line_text(&buf, row).contains("beta"))
            .expect("list rows paint");
        let x = (0..120)
            .find(|&x| buf[(x, row)].symbol() == "b")
            .expect("row text painted");
        let _ = panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            x,
            row,
        ));
        let log = log.borrow();
        assert!(
            log.events.last().is_some_and(|event| matches!(
                event,
                LibrarySlotEvent::List(crate::app::components::media_list::RowLocalInput::Click(_))
            )),
            "a row click routes List delegation to the owner"
        );
        assert_eq!(
            log.selections.last(),
            Some(&Some("beta".into())),
            "the owner resolved the clicked row's typed target"
        );
    }

    /// The Wide split-boundary drag moved in from
    /// `WideHeroBoundaryComponent`: a press inside the painted gap arms only
    /// the split gesture, and the drag resolves the live width from the
    /// panel's own painted geometry.
    #[test]
    fn split_drag_resolves_the_live_width_from_the_painted_gap() {
        let log = Rc::new(RefCell::new(FixtureLog::default()));
        let mut panel = LibraryPanel::new();
        panel.set_active(Some(LibraryKey::Home));
        panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
        let _ = draw_panel(&mut panel);
        let gap = panel.test_split_gap().expect("a wide split painted");
        assert!(gap.width > 0 && gap.height > 0);

        // Press inside the gap, drag right, release.
        assert_eq!(
            panel.on(&mouse_event(
                MouseEventKind::Down(MouseButton::Left),
                gap.x,
                gap.y
            )),
            None,
            "press-and-release without motion changes nothing"
        );
        let msg = panel.on(&mouse_event(
            MouseEventKind::Drag(MouseButton::Left),
            gap.x + 6,
            gap.y,
        ));
        match msg {
            Some(Msg::Shell(ShellRequest::ResizeListPaneLive(width))) => {
                assert!(width > 0, "the drag resolves a live width: {width}");
            }
            other => panic!("the drag must resolve the live width: {other:?}"),
        }
        // DragEnd is a live-only no-op.
        assert_eq!(
            panel.on(&mouse_event(
                MouseEventKind::Up(MouseButton::Left),
                gap.x + 6,
                gap.y
            )),
            None
        );
    }

    /// Owner retention: dropping a key from the live set removes its owner;
    /// retained owners keep their state.
    #[test]
    fn owner_retention_follows_the_catalog() {
        let mut panel = LibraryPanel::new();
        panel.set_active(Some(LibraryKey::Home));
        panel.insert_owner(
            LibraryKey::Home,
            Box::new(FixtureOwner::new(Rc::new(RefCell::new(
                FixtureLog::default(),
            )))),
        );
        panel.insert_owner(
            LibraryKey::Feeds,
            Box::new(FixtureOwner::new(Rc::new(RefCell::new(
                FixtureLog::default(),
            )))),
        );
        panel.retain_owners(&[LibraryKey::Home]);
        assert!(panel.has_owner(&LibraryKey::Home));
        assert!(!panel.has_owner(&LibraryKey::Feeds));
    }

    /// An unpainted frame (hidden library column, no owner) arms nothing:
    /// the panel resolves only geometry it painted.
    #[test]
    fn unpainted_frame_after_a_painted_one_arms_nothing() {
        let mut panel = LibraryPanel::new();
        panel.set_active(Some(LibraryKey::Home));
        panel.insert_owner(
            LibraryKey::Home,
            Box::new(FixtureOwner::new(Rc::new(RefCell::new(
                FixtureLog::default(),
            )))),
        );
        let _ = draw_panel(&mut panel);
        assert!(panel.test_split_gap().is_some());
        // The next view without an owner clears the geometry.
        panel.set_active(None);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal
            .draw(|f| Component::view(&mut panel, f, Rect::new(0, 0, 120, 30)))
            .unwrap();
        assert_eq!(
            panel.on(&mouse_event(MouseEventKind::Down(MouseButton::Left), 4, 4)),
            None,
            "an unpainted frame claims no pointer input"
        );
        assert!(panel.test_split_gap().is_none());
    }
}
