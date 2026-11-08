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
//! queue's row drag lives in the [`QueueSurface`]: a grip's press starts it,
//! motion previews the drop, and the release emits the one move request
//! (add-tauri-gui-client row 6.5). The chooser and popover presses resolve
//! their painted row indexes against [`PaintedChoices`], the lists the last
//! emitted frame painted, so a change between paint and press selects the
//! painted choice or nothing.

use mbv::GuiSurfaceLeaf;
use mbv_queue::QueueSlotId;
use mbv_surfaces::gesture::{ClickSelection, PointerGesture};
use mbv_surfaces::media_list::{MediaListOperation, RowIntent};
use mbv_surfaces::queue::{QueueRowInsertionSide, QueueSurface};
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::{HitMap, HitRegion};
use mbv_ui_model::context_menu::ContextMenuTargets;
use mbv_ui_model::playback::VOLUME_STEP;
use mbv_ui_msg::{
    ComponentId, KeyEvent, LeafKeyResult, Msg, PlaybackRequest, Position, QueueMove, QueueRequest,
    ShellRequest, TrackChipKind,
};

use crate::bridge::layout::{
    QueuePlaybackTargetId, QueueTargetId, RegionTarget, StatusBarTargetId,
};
use crate::painted_choices::PaintedChoices;

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

/// The queue row a region names, if it names a row: the row's own region or
/// the chrome painted over it (its grip and actions button), so moving onto
/// the chrome keeps the row hovered.
pub(crate) fn row_region_target(target: &RegionTarget) -> Option<QueueSlotId> {
    match target {
        RegionTarget::Queue(
            QueueTargetId::Slot(raw) | QueueTargetId::RowGrip(raw) | QueueTargetId::RowActions(raw),
        ) => Some(QueueSlotId::from_raw(*raw)),
        RegionTarget::Queue(_)
        | RegionTarget::QueuePlayback(_)
        | RegionTarget::StatusBar(_)
        | RegionTarget::Confirm(_)
        | RegionTarget::DaemonLost(_)
        | RegionTarget::Attach(_) => None,
    }
}

/// The queue row the point lands on, resolved against the reported regions:
/// the row's own region or the chrome painted over it.
pub(crate) fn row_slot_at(regions: &HitMap<RegionTarget>, at: Position) -> Option<QueueSlotId> {
    regions.resolve(at).and_then(row_region_target)
}

/// The last painted seekbar region holding `at`, the resolution precedence
/// the hit map gives the last push; `None` when the pointer is away from the
/// bar.
pub(crate) fn seekbar_region(regions: &HitMap<RegionTarget>, at: Position) -> Option<HitRegion> {
    regions
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

/// The Queue panel's leaf: the Queue Surface state and the reported regions.
///
/// The click, double-click, right-click, wheel, and drag arms mirror the
/// TUI Queue component's pointer path (`mbv-components` `queue/pointer.rs`)
/// with the reported regions in place of its retained paint geometry, plus
/// the canvas queue's row chrome (add-tauri-gui-client row 6.5): the hovered
/// row's grip starts the drag, its actions button opens the row menu, and an
/// open menu's entry executes. The playlist pill's click opens the TUI's
/// playlists overlay, which the first slice does not present, so it resolves
/// to nothing here — the same documented absence the GUI dispatch records for
/// the surfaces it has not ported.
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
        row_slot_at(self.regions, at)
    }

    /// The row region under `at`, with its slot: the last painted row region
    /// holding the point. A row drag reads the pointer's half of the region
    /// it names, so the insertion side follows the painted row's height.
    fn row_region_at(&self, at: Position) -> Option<(HitRegion, QueueSlotId)> {
        self.regions
            .iter()
            .filter_map(|(region, target)| row_region_target(target).map(|slot| (*region, slot)))
            .filter(|(region, _)| region.contains(at))
            .last()
    }

    /// The viewport length the row-local delegation clamps its wheel with:
    /// the capacity the front end reported, at least one row.
    fn viewport_len(&self) -> usize {
        self.surface.viewport_height().max(1)
    }

    /// Select a row and emit the TUI single click's App effects.
    fn select_row(&mut self, target: QueueSlotId, selection: ClickSelection) -> Msg {
        let operation = match selection {
            ClickSelection::Plain => MediaListOperation::Select(target),
            ClickSelection::Toggle => MediaListOperation::Toggle(target),
            ClickSelection::Range => MediaListOperation::Range(target),
        };
        self.surface
            .delegate_operation_with_viewport_len(operation, self.viewport_len());
        Msg::Shell(Box::new(ShellRequest::QueueRowClick {
            slot_id: self.surface.selected_target().copied(),
        }))
    }

    fn click(&mut self, at: Position, selection: ClickSelection) -> Option<Msg> {
        // The playlist pill's click opens the TUI's playlists overlay, which
        // the first slice does not present, so it resolves to nothing here —
        // the same documented absence the GUI dispatch records for the
        // surfaces it has not ported. A point in no queue region resolves to
        // nothing too.
        let region_target = self.regions.resolve(at)?;
        match region_target {
            RegionTarget::Queue(QueueTargetId::Slot(raw)) => {
                Some(self.select_row(QueueSlotId::from_raw(*raw), selection))
            }
            // The open row menu's entry: the Client core executes the entry
            // the index names in the menu it opened. The menu carries its
            // entries' resolved queue targets, so the press addresses the
            // row painted for them.
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(index)) => Some(Msg::Shell(Box::new(
                ShellRequest::ContextMenuSelect(*index),
            ))),
            // The hovered row's actions button opens the row menu over that
            // row (the canvas row menu).
            RegionTarget::Queue(QueueTargetId::RowActions(raw)) => {
                Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
                    ContextMenuTargets::Queue(vec![QueueSlotId::from_raw(*raw)]),
                    None,
                ))))
            }
            // The hovered row's grip starts the drag and selects the row, as
            // a press on the row does; motion previews the drop and the
            // release sends the one move request.
            RegionTarget::Queue(QueueTargetId::RowGrip(raw)) => {
                let target = QueueSlotId::from_raw(*raw);
                self.surface.begin_row_drag(target);
                Some(self.select_row(target, selection))
            }
            // The queue header's switch and filter controls (row 6.5): each
            // press emits the request whose App-side effect the `t` and `/`
            // chords dispatch, so the control and the chord run one path.
            RegionTarget::Queue(QueueTargetId::RightColumn) => {
                Some(Msg::Playback(PlaybackRequest::ToggleQueueRightColumn))
            }
            RegionTarget::Queue(QueueTargetId::Filter) => {
                Some(Msg::Playback(PlaybackRequest::ActivateQueueFilter))
            }
            // The footer's playlist pill is informational in the GUI (its
            // layout report carries no `playlistPill` region — the GUI
            // presents no playlists surface, so no press resolves here).
            // The TUI's pill opens the Playlists overlay.
            RegionTarget::Queue(QueueTargetId::PlaylistPill)
            | RegionTarget::QueuePlayback(_)
            | RegionTarget::StatusBar(_)
            // A confirm button and a daemon-lost dialog's button resolve in
            // the core's own route_pointer arm, never through a surface
            // leaf.
            | RegionTarget::Confirm(_)
            | RegionTarget::DaemonLost(_)
            | RegionTarget::Attach(_) => None,
        }
    }

    fn double_click(&mut self, at: Position) -> Option<Msg> {
        // The TUI's double click re-selects the row (the first press already
        // did) and lets the dispatched activation play it. Only the row's own
        // region plays: the hover chrome's grip and actions button swallow a
        // double click (review of row 6.5, finding P2-2), so a fast second
        // press on either control never opens playback.
        let Some(RegionTarget::Queue(QueueTargetId::Slot(raw))) = self.regions.resolve(at) else {
            return None;
        };
        let target = QueueSlotId::from_raw(*raw);
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

    fn drag(&mut self, to: Position) -> Option<Msg> {
        // Motion only previews the drop: the row drag's request is the
        // release's, and a motion the drag does not own (a press that did not
        // start on a grip) changes nothing.
        self.surface.row_drag()?;
        let Some((region, anchor)) = self.row_region_at(to) else {
            // Off the rows: the drag keeps its last preview.
            return None;
        };
        let side = if to.y < region.y + region.height / 2.0 {
            QueueRowInsertionSide::Before
        } else {
            QueueRowInsertionSide::After
        };
        self.surface.preview_row_drag_drop(anchor, side);
        None
    }

    /// End the drag: one move request when the drop changes the slot's place,
    /// and none when the release lands it where it started or the rows no
    /// longer hold the dragged slot (a content refresh can drop it).
    fn finish_drag(&mut self) -> Option<Msg> {
        let drag = self.surface.take_row_drag()?;
        if drag.drop_index == drag.from_index {
            return None;
        }
        self.surface.item_index_of(drag.slot_id)?;
        Some(Msg::Queue(QueueRequest::MoveToIndex {
            link: self.surface.link(),
            slot_id: drag.slot_id,
            to_index: drag.drop_index,
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
            PointerGesture::Drag { to, .. } => self.drag(to),
            // The release finalizes the row drag: one move request, or none.
            PointerGesture::DragEnd => self.finish_drag(),
        }
    }

    fn move_selected_row(&mut self, direction: QueueMove) -> Option<Msg> {
        self.surface.selected_move_request(direction)
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
/// reported regions, the transport availability, and the choice lists the
/// last emitted frame painted.
///
/// The arms mirror the TUI Queue playback panel's pointer path: the transport
/// glyphs act on a left press — the second press of a double-click pair
/// included, so a double-click on a glyph toggles twice and nets out — and
/// the seekbar seeks on a press (the `gui-client` spec's single-press seek)
/// and scrubs on a left-drag anchored inside its bar. The header's target
/// indicator opens the chooser the Sessions sidebar is in the TUI, a
/// chooser row selects its painted output through the same `SelectSession`
/// request, and the header's collapse and queue-fold controls run the `z`/`q`
/// toggles' dispatches. The audio and subtitle chips toggle their track
/// popover through the `OpenTrackPopover` request's open path, and a
/// popover row selects the track its painted row named (row 6.5's review of
/// row 6.4: the index addresses the list the window painted).
/// The visual slot's chrome menu is a surface the
/// first slice does not present, so a right-click resolves to nothing here.
pub(crate) struct PlaybackLeaf<'a> {
    surface: &'a mut QueuePlaybackSurface,
    regions: &'a HitMap<RegionTarget>,
    availability: TransportAvailability,
    /// The choice lists the last emitted frame painted: a chooser or popover
    /// row's index resolves against what the window shows, not a fresh
    /// projection that may have shifted under it.
    painted: &'a PaintedChoices,
}

impl<'a> PlaybackLeaf<'a> {
    pub(crate) fn new(
        surface: &'a mut QueuePlaybackSurface,
        regions: &'a HitMap<RegionTarget>,
        availability: TransportAvailability,
        painted: &'a PaintedChoices,
    ) -> Self {
        Self {
            surface,
            regions,
            availability,
            painted,
        }
    }

    /// The seekbar's painted region holding `at`, the core thread's hover
    /// resolution's shared rule.
    fn seekbar_at(&self, at: Position) -> Option<HitRegion> {
        seekbar_region(self.regions, at)
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
            // The back and forward controls press the canvas transport's
            // deeper steps (add-tauri-gui-client row 6.4, the "back 10 s,
            // forward 30 s" spec): the same commands the `seek_back_10` and
            // `seek_forward_30` chords dispatch, never the TUI's ±5 s
            // `seek_back`/`seek_forward`.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekBack)) => {
                Some(Msg::Playback(PlaybackRequest::SeekRelative(-10.0)))
            }
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekForward)) => {
                Some(Msg::Playback(PlaybackRequest::SeekRelative(30.0)))
            }
            // The header's target press emits the same opener the TUI's
            // indicator sends the Sessions sidebar; the GUI's counterpart
            // is the Client-core chooser overlay.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget)) => {
                Some(Msg::Shell(Box::new(ShellRequest::OpenSessions)))
            }
            // A chooser row resolves its painted index to the stable target
            // key and selects the output through the request the TUI's
            // Sessions sidebar sends for its selected row.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(index))) => self
                .painted
                .target_key(*index)
                .map(|key| Msg::Shell(Box::new(ShellRequest::SelectSession(key.clone())))),
            // The audio and subtitle chips emit the popover's opener: the
            // press toggles the open popover as the target press toggles
            // the chooser. The video chip is label-only (design D4a's
            // ruling), so it reports no region.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::AudioChip)) => Some(
                Msg::Playback(PlaybackRequest::OpenTrackPopover(TrackChipKind::Audio)),
            ),
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::SubtitleChip)) => Some(
                Msg::Playback(PlaybackRequest::OpenTrackPopover(TrackChipKind::Subtitles)),
            ),
            // A track popover row resolves its painted index to the track id
            // the frame painted, so a list change between paint and press
            // switches that track or none, never the neighbour; the App runs
            // the same execution the popover's Enter applies.
            Some(RegionTarget::QueuePlayback(QueuePlaybackTargetId::TrackPopoverChoice(index))) => {
                self.painted
                    .popover_track_id(*index)
                    .map(|track_id| Msg::Playback(PlaybackRequest::TrackPopoverSelect(track_id)))
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

    fn move_selected_row(&mut self, _direction: QueueMove) -> Option<Msg> {
        // The transport band paints no queue rows, so it holds no selection
        // for the GUI-only row move to address.
        None
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

    fn move_selected_row(&mut self, _direction: QueueMove) -> Option<Msg> {
        // The Status bar paints no queue rows, so it holds no selection for
        // the GUI-only row move to address.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{QueueLeaf, QueueSlotId, QueueSurface, QueueTargetId, RegionTarget};
    use mbv::GuiSurfaceLeaf;
    use mbv_surfaces::gesture::PointerGesture;
    use mbv_surfaces::{HitMap, HitRegion};
    use mbv_ui_model::media_list::{MediaKind, MediaListRow, MediaSemanticState};
    use mbv_ui_msg::Position;
    use rstest::rstest;

    /// A queue holding one item row for slot 1.
    fn one_row_queue() -> QueueSurface {
        let mut queue = QueueSurface::new();
        queue.list_mut().set_content(vec![MediaListRow::Item {
            target: QueueSlotId::from_raw(1),
            primary: "Item 1".to_string(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        }]);
        queue
    }

    /// A double click on the row chrome — the grip or the actions button —
    /// plays nothing (review of add-tauri-gui-client row 6.5, finding P2-2):
    /// the chrome regions swallow the gesture, so a fast second press on the
    /// control never activates the row beneath it.
    #[rstest]
    #[case(QueueTargetId::RowGrip(1))]
    #[case(QueueTargetId::RowActions(1))]
    fn a_double_click_on_the_row_chrome_plays_nothing_review_u65_p2_2(
        #[case] target: QueueTargetId,
    ) {
        let mut queue = one_row_queue();
        let mut regions = HitMap::new();
        regions.push(
            HitRegion::new(0.0, 0.0, 100.0, 30.0),
            RegionTarget::Queue(target),
        );
        let mut leaf = QueueLeaf::new(&mut queue, &regions);
        assert_eq!(
            leaf.pointer(PointerGesture::DoubleClick(Position { x: 10.0, y: 10.0 })),
            None
        );
    }
}
