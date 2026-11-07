//! The GUI Client's [`GuiSurfaceLeaf`] adapters (design D4 of
//! `add-tauri-gui-client`): the first slice's Surface state seen from the
//! shared router's leaf side. Each adapter resolves one recognized gesture
//! against the combined hit map of the last `LayoutReport`, claims only the
//! regions its surface painted, and emits the same semantic `Msg`s the
//! matching TUI component's pointer path emits — the interaction rules stay
//! in the Client core and the Surface state, never here.
//!
//! The adapters are constructed per routing call over the core thread's own
//! fields; they hold no state of their own. The seekbar's scrub throttle
//! lives in the [`QueuePlaybackSurface`] (shared Surface state), and the
//! queue's drag anchor lives in the gesture recognizer, so a drag re-derives
//! its grabbed row from the anchor position each motion event, exactly as the
//! TUI's retained grab resolves to.

use mbv::{GuiSurfaceLeaf, PlaybackTargetChoice};
use mbv_queue::QueueSlotId;
use mbv_surfaces::gesture::{ClickSelection, PointerGesture};
use mbv_surfaces::media_list::{MediaListOperation, RowIntent};
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::{HitMap, HitRegion};
use mbv_ui_model::context_menu::ContextMenuTargets;
use mbv_ui_model::playback::VOLUME_STEP;
use mbv_ui_msg::{
    ComponentId, KeyEvent, LeafKeyResult, Msg, PlaybackRequest, Position, QueueRequest,
    ShellRequest,
};

use crate::bridge::layout::{
    QueuePlaybackTargetId, QueueTargetId, RegionTarget, StatusBarTargetId,
};

/// The point a gesture resolves against: its own position, or the leading
/// edge of a drag (the row the pointer reached). A `DragEnd` carries no
/// point — it only closes the drag — and resolves to nothing.
pub(crate) fn gesture_point(gesture: &PointerGesture<Position>) -> Option<Position> {
    match *gesture {
        PointerGesture::Click { at, .. }
        | PointerGesture::DoubleClick(at)
        | PointerGesture::RightClick(at)
        | PointerGesture::Scroll { at, .. }
        | PointerGesture::Drag { to: at, .. } => Some(at),
        PointerGesture::DragEnd => None,
    }
}

/// The Queue panel's leaf: the Queue Surface state and the reported regions.
///
/// The click, double-click, right-click, wheel, and drag arms mirror the
/// TUI Queue component's pointer path (`mbv-components` `queue/pointer.rs`)
/// with the reported regions in place of its retained paint geometry. The
/// playlist pill's click opens the TUI's playlists overlay, which the first
/// slice does not present, so it resolves to nothing here — the same
/// documented absence the GUI dispatch records for the surfaces it has not
/// ported.
pub(crate) struct QueueLeaf<'a> {
    surface: &'a mut QueueSurface,
    regions: &'a HitMap<RegionTarget>,
}

impl<'a> QueueLeaf<'a> {
    pub(crate) fn new(surface: &'a mut QueueSurface, regions: &'a HitMap<RegionTarget>) -> Self {
        Self { surface, regions }
    }

    /// The row a queue region resolves to, if one does.
    fn row_at(&self, at: Position) -> Option<QueueSlotId> {
        match self.regions.resolve(at) {
            Some(RegionTarget::Queue(QueueTargetId::Slot(raw))) => {
                Some(QueueSlotId::from_raw(*raw))
            }
            _ => None,
        }
    }

    /// The viewport length the row-local delegation clamps its wheel with:
    /// the capacity the front end reported, at least one row.
    fn viewport_len(&self) -> usize {
        self.surface.viewport_height().max(1)
    }

    fn click(&mut self, at: Position, selection: ClickSelection) -> Option<Msg> {
        // The playlist pill's click opens the TUI's playlists overlay, which
        // the first slice does not present, so it resolves to nothing here —
        // the same documented absence the GUI dispatch records for the
        // surfaces it has not ported. A point in no queue region resolves to
        // nothing too.
        let Some(RegionTarget::Queue(QueueTargetId::Slot(raw))) = self.regions.resolve(at) else {
            return None;
        };
        let target = QueueSlotId::from_raw(*raw);
        let operation = match selection {
            ClickSelection::Plain => MediaListOperation::Select(target),
            ClickSelection::Toggle => MediaListOperation::Toggle(target),
            ClickSelection::Range => MediaListOperation::Range(target),
        };
        self.surface
            .delegate_operation_with_viewport_len(operation, self.viewport_len());
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowClick {
            slot_id: self.surface.selected_target().copied(),
        })))
    }

    fn double_click(&mut self, at: Position) -> Option<Msg> {
        // The TUI's double click re-selects the row (the first press already
        // did) and lets the dispatched activation play it.
        let target = self.row_at(at)?;
        self.surface.delegate_operation_with_viewport_len(
            MediaListOperation::Select(target),
            self.viewport_len(),
        );
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
            slot_id: self.surface.selected_target().copied(),
        })))
    }

    fn right_click(&mut self, at: Position) -> Option<Msg> {
        // Legacy parity with the TUI: a right-click on blank queue space
        // opens no menu, so only a resolved row produces one. The context
        // menu is a surface the first slice does not present, so the
        // dispatched request stays undelivered (the GUI dispatch's
        // documented no-op).
        let target = self.row_at(at)?;
        let outcome = self.surface.delegate_operation_with_viewport_len(
            MediaListOperation::Context(target),
            self.viewport_len(),
        );
        let targets = match outcome.external_intent {
            Some(RowIntent::Context(target)) => vec![target],
            Some(RowIntent::ContextSelection(targets)) => targets,
            _ => vec![target],
        };
        Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            ContextMenuTargets::Queue(targets),
            // The terminal cell anchor has no GUI counterpart.
            None,
        ))))
    }

    fn scroll(&mut self, at: Position, delta: i64) -> Option<Msg> {
        // The uniform wheel step moves the viewport only over a painted row:
        // the TUI's claim covers its whole painted list rect, and the GUI's
        // reported regions stand in for it row by row.
        self.row_at(at)?;
        self.surface.delegate_operation_with_viewport_len(
            MediaListOperation::Scroll(delta),
            self.viewport_len(),
        );
        // The mutation stands on the Surface state; the next frame carries it.
        None
    }

    fn drag(&mut self, from: Position, to: Position) -> Option<Msg> {
        // The grabbed row is the one the press anchored on; the target is the
        // row the pointer reached. The TUI pins the grab at press time and
        // the anchor is that press's position, so resolving it here resolves
        // to the same row.
        let grabbed = self.row_at(from)?;
        let onto = self.row_at(to)?;
        if grabbed == onto {
            return None;
        }
        self.surface.delegate_operation_with_viewport_len(
            MediaListOperation::Select(grabbed),
            self.viewport_len(),
        );
        Some(Msg::Queue(QueueRequest::MoveTo {
            link: self.surface.link(),
            slot_id: grabbed,
            onto,
        }))
    }
}

impl GuiSurfaceLeaf for QueueLeaf<'_> {
    fn id(&self) -> ComponentId {
        ComponentId::Queue
    }

    fn visual_selection_active(&self) -> bool {
        // The TUI's Visual-selection fact is the focused list's non-empty
        // mark set (`SelectionSummary::count`).
        !self.surface.multi_selection().is_empty()
    }

    fn key(&mut self, key: KeyEvent) -> LeafKeyResult {
        self.surface.key(key)
    }

    fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg> {
        match gesture {
            PointerGesture::Click { at, selection } => self.click(at, selection),
            PointerGesture::DoubleClick(at) => self.double_click(at),
            PointerGesture::RightClick(at) => self.right_click(at),
            PointerGesture::Scroll { at, delta } => self.scroll(at, delta),
            PointerGesture::Drag { from, to } => self.drag(from, to),
            // The drag's grab re-derives from the anchor every motion, so the
            // end carries no state to clear.
            PointerGesture::DragEnd => None,
        }
    }
}

/// Which transport controls are actionable: the Queue playback projection's
/// availability, read at routing time. The front end paints an unavailable
/// control dimmed and still reports its region, so the leaf carries the gate
/// the TUI component resolves against its transport projection.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TransportAvailability {
    pub(crate) stop: bool,
    pub(crate) previous: bool,
    pub(crate) next: bool,
}

/// The Queue playback panel's leaf: the Queue playback Surface state, the
/// reported regions, the transport availability, and the playback-target
/// chooser's live choice list.
///
/// The arms mirror the TUI Queue playback panel's pointer path: the transport
/// glyphs act on a left press — the second press of a double-click pair
/// included, so a double-click on a glyph toggles twice and nets out — and
/// the seekbar seeks on a press (the `gui-client` spec's single-press seek)
/// and scrubs on a left-drag anchored inside its bar. The header's target
/// indicator opens the chooser the Sessions sidebar is in the TUI, a
/// chooser row selects its output through the same `SelectSession` request,
/// and the header's collapse and queue-fold controls run the `z`/`q`
/// toggles' dispatches. The visual slot's chrome menu is a surface the
/// first slice does not present, so a right-click resolves to nothing here.
pub(crate) struct PlaybackLeaf<'a> {
    surface: &'a mut QueuePlaybackSurface,
    regions: &'a HitMap<RegionTarget>,
    availability: TransportAvailability,
    /// The playback-target chooser's live choices: a `TargetChoice` region's
    /// index resolves against this list, the same list the frame carries.
    choices: &'a [PlaybackTargetChoice],
}

impl<'a> PlaybackLeaf<'a> {
    pub(crate) fn new(
        surface: &'a mut QueuePlaybackSurface,
        regions: &'a HitMap<RegionTarget>,
        availability: TransportAvailability,
        choices: &'a [PlaybackTargetChoice],
    ) -> Self {
        Self {
            surface,
            regions,
            availability,
            choices,
        }
    }

    /// The seekbar's painted region, when one contains `at`: the last
    /// painted seekbar region holding the point, the resolution precedence
    /// the hit map gives the last push.
    fn seekbar_at(&self, at: Position) -> Option<HitRegion> {
        self.regions
            .iter()
            .filter(|(region, target)| {
                matches!(
                    target,
                    RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar)
                ) && region.contains(at)
            })
            .last()
            .map(|(region, _)| *region)
    }

    /// The action a press over the transport band takes. The glyph arms run
    /// on every left press, exactly as the TUI's `Down` arms do, and the
    /// seekbar seeks to the pressed position (the `gui-client` spec's "a
    /// press on the bar SHALL seek"), through the same fraction rule the
    /// double-click seek used.
    fn press(&self, at: Position) -> Option<Msg> {
        match self.regions.resolve(at) {
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause)) => {
                Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
            }
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Stop))
                if self.availability.stop =>
            {
                Some(Msg::Playback(PlaybackRequest::Stop))
            }
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Previous))
                if self.availability.previous =>
            {
                Some(Msg::Playback(PlaybackRequest::Previous))
            }
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next))
                if self.availability.next =>
            {
                Some(Msg::Playback(PlaybackRequest::Next))
            }
            // The back and forward controls send the deltas the `seek_back`
            // and `seek_forward` chords carry, so the buttons and the
            // chords dispatch the same command (the `gui-client` spec).
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekBack)) => {
                Some(Msg::Playback(PlaybackRequest::SeekRelative(-5.0)))
            }
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekForward)) => {
                Some(Msg::Playback(PlaybackRequest::SeekRelative(5.0)))
            }
            // The header's target press emits the same opener the TUI's
            // indicator sends the Sessions sidebar; the GUI's counterpart
            // is the Client-core chooser overlay.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget)) => {
                Some(Msg::Shell(Box::new(ShellRequest::OpenSessions)))
            }
            // A chooser row resolves its choice by index against the live
            // list and selects the output through the request the TUI's
            // Sessions sidebar sends for its selected row.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(index))) => {
                self.choices.get(*index).map(|choice| {
                    Msg::Shell(Box::new(ShellRequest::SelectSession(choice.key.clone())))
                })
            }
            // The header's collapse and queue-fold controls run the `z`/`q`
            // toggles' own dispatch.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse)) => {
                Some(Msg::Playback(PlaybackRequest::TogglePanelCollapse))
            }
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::QueueFold)) => {
                Some(Msg::Playback(PlaybackRequest::ToggleQueueFold))
            }
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar)) => self.seek(at),
            _ => None,
        }
    }

    /// The seek a press on the bar seeks to: the press column's fraction
    /// of the painted bar span.
    fn seek(&self, at: Position) -> Option<Msg> {
        let bar = self.seekbar_at(at)?;
        self.surface
            .seek_target(bar, at.x)
            .map(|fraction| Msg::Playback(PlaybackRequest::SeekTo(fraction)))
    }
}

impl GuiSurfaceLeaf for PlaybackLeaf<'_> {
    fn id(&self) -> ComponentId {
        ComponentId::QueuePlaybackPanel
    }

    fn visual_selection_active(&self) -> bool {
        // The transport band holds no list selection.
        false
    }

    fn key(&mut self, _key: KeyEvent) -> LeafKeyResult {
        // The transport band has no key rules of its own: every transport
        // chord routes through the central router as an App command.
        LeafKeyResult::Unhandled
    }

    fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg> {
        match gesture {
            // A press seeks or activates what it lands on: the seekbar
            // seeks to the pressed position (the `gui-client` spec), the
            // transport and header targets act. The second press of a pair
            // resolves through the same arms, so a double-click on the bar
            // seeks twice to the same position — the kept double-click
            // behaviour.
            PointerGesture::Click { at, .. } | PointerGesture::DoubleClick(at) => self.press(at),
            // A right-click on the visual slot opens the TUI's chrome menu,
            // which the first slice does not present; the wheel scrolls
            // nothing on the transport band; and the scrub's grab re-derives
            // from the anchor every motion, so the drag's end carries no
            // state to clear.
            PointerGesture::RightClick(_)
            | PointerGesture::Scroll { .. }
            | PointerGesture::DragEnd => None,
            PointerGesture::Drag { from, to } => {
                // The scrub's bar must hold both the anchor and the pointer:
                // a drag that merely crossed the bar is not a scrub of it.
                let bar = self.seekbar_at(to)?;
                self.surface
                    .admit_scrub(bar, from, to, std::time::Instant::now())
                    .map(|fraction| Msg::Playback(PlaybackRequest::SeekTo(fraction)))
            }
        }
    }
}

/// The Status bar's leaf over the reported pill regions.
///
/// The volume pill takes the uniform wheel step as one shared
/// [`VOLUME_STEP`] per notch (the TUI's wheel mapping), and the mute pill
/// toggles mute on a press. The visual-mode indicator's clear action is a
/// shell-level projection the GUI frame does not carry — the status
/// projection names the armed pill and the toast only — so no region can
/// report it until a later slice carries the fact.
pub(crate) struct StatusBarLeaf<'a> {
    regions: &'a HitMap<RegionTarget>,
}

impl<'a> StatusBarLeaf<'a> {
    pub(crate) fn new(regions: &'a HitMap<RegionTarget>) -> Self {
        Self { regions }
    }
}

impl GuiSurfaceLeaf for StatusBarLeaf<'_> {
    fn id(&self) -> ComponentId {
        ComponentId::StatusBarPanel
    }

    fn visual_selection_active(&self) -> bool {
        false
    }

    fn key(&mut self, _key: KeyEvent) -> LeafKeyResult {
        LeafKeyResult::Unhandled
    }

    fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg> {
        match gesture {
            PointerGesture::Scroll { at, delta } => {
                if !matches!(
                    self.regions.resolve(at),
                    Some(RegionTarget::StatusBar(StatusBarTargetId::VolumePill))
                ) {
                    return None;
                }
                // Scroll up raises the volume, scroll down lowers it, one
                // shared step per notch.
                let step = if delta < 0 { VOLUME_STEP } else { -VOLUME_STEP };
                Some(Msg::Playback(PlaybackRequest::VolumeDelta(step)))
            }
            // Same `ToggleMute` the `m` key dispatches.
            PointerGesture::Click { at, .. }
                if matches!(
                    self.regions.resolve(at),
                    Some(RegionTarget::StatusBar(StatusBarTargetId::MutePill))
                ) =>
            {
                Some(Msg::Playback(PlaybackRequest::ToggleMute))
            }
            _ => None,
        }
    }
}
