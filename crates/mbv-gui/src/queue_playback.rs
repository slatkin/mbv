//! The GUI Client's Queue playback band (row 6.5): the egui `Painter` over the
//! Queue playback Surface state. The band paints a header row (the
//! playback-target indicator and the status word) in every state. Under it, while
//! playback shows, the band paints the now-playing title, the four transport
//! controls, and the seekbar with its elapsed and total times. Every colour
//! resolves through the theme's neutral `Rgb`.
//!
//! The band holds no interaction state of its own. The transport focus and the
//! scrub throttle live in `QueuePlaybackSurface`, and no stateful egui widget
//! exists here (design D8). The painter reports each frame's hit map, and
//! pointer gestures resolve against the last completed frame's report, as the
//! Queue panel's do (row 6.4).

use std::time::Instant;

use eframe::egui;
use mbv::{GuiSurfaceLeaf, QueuePlaybackProjection};
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_surfaces::gesture::PointerGesture;
use mbv_surfaces::queue_playback::{
    QueuePlaybackFrameReport, QueuePlaybackHitTarget, QueuePlaybackSurface,
};
use mbv_surfaces::{HitMap, HitRegion};
use mbv_theme::{Role, Surface};
use mbv_ui_model::playback::PlaybackState;
use mbv_ui_model::playback_target::NowPlayingStatus;
use mbv_ui_model::ui_util::fmt_duration_short;
use mbv_ui_msg::{ComponentId, KeyEvent, LeafKeyResult, Msg, PlaybackRequest, Position};

use crate::queue::{PANEL_PADDING, ROW_HEIGHT, role_colour, surface_fill, to_region};

/// The header row's height: the playback-target indicator and the status word,
/// one fixed row. The header paints in every state (`specs/gui-client`: fixed
/// row heights).
const HEADER_HEIGHT: f32 = ROW_HEIGHT;

/// The transport section's height: the title row, the controls row, and the seek
/// row, each one fixed row. The section collapses while idle.
const TRANSPORT_HEIGHT: f32 = 3.0 * ROW_HEIGHT;

/// The gap between the header's painted elements (the target icon, the host
/// name, and the status word), in logical points.
const HEADER_GAP: f32 = 4.0;

/// The playback-target icon: the TUI's glyph while nerd fonts are off
/// (`target_icon` in `chrome_player/title/header.rs`). This Client has no
/// nerd-font setting, so it paints this glyph in both target states. egui's
/// bundled emoji font covers it.
const TARGET_ICON: &str = "\u{1F5A7}";

/// The width of one transport control's cell, in logical points.
const CONTROL_WIDTH: f32 = 32.0;

/// The thickness of the seekbar's track, in logical points.
const BAR_THICKNESS: f32 = 4.0;

/// The gap between a seek row's time label and the bar, in logical points.
const TIME_GAP: f32 = 8.0;

/// The text size of the band's title, glyphs, and time labels, in logical
/// points.
const TEXT_SIZE: f32 = 13.0;

/// The Queue playback band: the Surface state plus the projection the sync
/// pass adopted, and the painted geometry and report the pointer gestures
/// resolve against.
pub(crate) struct QueuePlaybackPanel {
    surface: QueuePlaybackSurface,
    /// The facts the band paints, adopted each tick.
    projection: QueuePlaybackProjection,
    /// The band's rect in the last completed frame. A gesture anchored inside
    /// it resolves here; an idle frame's zero rect claims nothing.
    band: HitRegion,
    /// The painted seekbar span in the last completed frame. Zero when no bar
    /// painted, so a seek or scrub resolves nothing.
    seek_bar: HitRegion,
    /// The header row's rect in the last completed frame: the paint evidence
    /// the headless tests read (`queue_playback/tests.rs`).
    #[cfg(test)]
    header: HitRegion,
    /// The last completed frame's report: the hit map presses resolve against.
    last_report: QueuePlaybackFrameReport,
    /// The request the last gesture sent across the shell boundary, kept for
    /// the headless tests (`queue_playback/tests.rs`).
    #[cfg(test)]
    last_request: Option<Msg>,
}

impl Default for QueuePlaybackPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl QueuePlaybackPanel {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            surface: QueuePlaybackSurface::new(),
            projection: QueuePlaybackProjection::default(),
            band: HitRegion::new(0.0, 0.0, 0.0, 0.0),
            seek_bar: HitRegion::new(0.0, 0.0, 0.0, 0.0),
            #[cfg(test)]
            header: HitRegion::new(0.0, 0.0, 0.0, 0.0),
            last_report: QueuePlaybackFrameReport {
                hit_map: HitMap::new(),
            },
            #[cfg(test)]
            last_request: None,
        }
    }

    /// Adopt one projection push from the `App` (design D5).
    pub(crate) fn sync(&mut self, projection: QueuePlaybackProjection) {
        self.projection = projection;
    }

    /// The height the band reserves this frame: the header row, always present,
    /// and the transport section while playback shows, inside the panel padding
    /// above and below.
    #[must_use]
    pub(crate) fn band_height(&self) -> f32 {
        2.0 * PANEL_PADDING + HEADER_HEIGHT + self.transport_height()
    }

    /// The transport section's height this frame: zero while idle, which
    /// collapses it (task 3.6).
    #[must_use]
    fn transport_height(&self) -> f32 {
        if self.projection.shown {
            TRANSPORT_HEIGHT
        } else {
            0.0
        }
    }

    /// Whether the band claims a gesture anchored at `at`.
    #[must_use]
    pub(crate) fn claims(&self, at: Position) -> bool {
        self.band.contains(at)
    }

    /// Paint one frame over `rect` and report the frame's hit map. The header
    /// row paints in every state. The transport section paints only while
    /// playback shows, and it reports its regions only then.
    pub(crate) fn paint(&mut self, painter: &egui::Painter, rect: egui::Rect) {
        self.band = to_region(rect);
        self.seek_bar = HitRegion::new(0.0, 0.0, 0.0, 0.0);
        self.last_report = QueuePlaybackFrameReport {
            hit_map: HitMap::new(),
        };
        painter.rect_filled(
            rect,
            0.0,
            surface_fill(
                Surface::QueueOnlyPlaybackPanel,
                self.surface.transport_focused(),
            ),
        );
        let inner = rect.shrink(PANEL_PADDING);
        let header_row =
            egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), HEADER_HEIGHT));
        paint_header(painter, header_row, &self.projection);
        // The headless tests read the header row's paint (test evidence, not
        // production state).
        #[cfg(test)]
        let _ = std::mem::replace(&mut self.header, to_region(header_row));
        if !self.projection.shown {
            return;
        }
        let Some(title) = self.projection.now_playing_title.as_deref() else {
            return;
        };
        let title_row = header_row.translate(egui::vec2(0.0, HEADER_HEIGHT));
        let controls_row = title_row.translate(egui::vec2(0.0, ROW_HEIGHT));
        let seek_row = controls_row.translate(egui::vec2(0.0, ROW_HEIGHT));
        paint_elided(
            painter,
            title_row,
            title,
            role_colour(Role::PlaybackValueFg),
        );
        let mut hit_map = paint_controls(painter, controls_row, &self.projection);
        if let Some(bar) = paint_seek_row(painter, seek_row, &self.projection) {
            hit_map.push(to_region(bar), QueuePlaybackHitTarget::Seekbar);
            self.seek_bar = to_region(bar);
        }
        self.last_report = QueuePlaybackFrameReport { hit_map };
    }

    /// Resolve one recognized pointer gesture against the last completed
    /// frame's report and return the request it sends across the shell
    /// boundary. A gesture the painted band does not claim returns `None`.
    pub(crate) fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg> {
        let msg = match gesture {
            PointerGesture::Click { at, .. } => self.control_press(at),
            PointerGesture::DoubleClick(at) => self.seek_at(at).or_else(|| self.control_press(at)),
            PointerGesture::Drag { from, to } => self.scrub(from, to),
            PointerGesture::RightClick(_)
            | PointerGesture::Scroll { .. }
            | PointerGesture::DragEnd => None,
        };
        // The headless tests read the request the gesture sent (test evidence,
        // not production state).
        #[cfg(test)]
        let _ = std::mem::replace(&mut self.last_request, msg.clone());
        msg
    }

    fn target_at(&self, at: Position) -> Option<QueuePlaybackHitTarget> {
        self.last_report.hit_map.resolve(at).copied()
    }

    /// A press on a transport control sends the control's request. A control
    /// the playback state does not allow resolves to nothing.
    fn control_press(&self, at: Position) -> Option<Msg> {
        let available = self.projection.availability;
        let request = match self.target_at(at)? {
            QueuePlaybackHitTarget::PlayPause => PlaybackRequest::TogglePlayPause,
            QueuePlaybackHitTarget::Stop if available.stop => PlaybackRequest::Stop,
            QueuePlaybackHitTarget::Previous if available.previous => PlaybackRequest::Previous,
            QueuePlaybackHitTarget::Next if available.next => PlaybackRequest::Next,
            QueuePlaybackHitTarget::Stop
            | QueuePlaybackHitTarget::Previous
            | QueuePlaybackHitTarget::Next
            | QueuePlaybackHitTarget::Seekbar
            | QueuePlaybackHitTarget::PlaybackTarget
            | QueuePlaybackHitTarget::VisualSlot => return None,
        };
        Some(Msg::Playback(request))
    }

    /// A double-click on the seekbar seeks to the column's fraction of the bar
    /// (the TUI's double-click rule: a single press on the bar seeks nothing).
    fn seek_at(&self, at: Position) -> Option<Msg> {
        if self.target_at(at)? != QueuePlaybackHitTarget::Seekbar {
            return None;
        }
        self.surface.seek_target(self.seek_bar, at.x).map(seek_msg)
    }

    /// A left-drag anchored inside the bar scrubs it, admitted through the
    /// Surface state's throttle (the TUI's scrub rule).
    fn scrub(&mut self, from: Position, to: Position) -> Option<Msg> {
        self.surface
            .admit_scrub(self.seek_bar, from, to, Instant::now())
            .map(seek_msg)
    }
}

#[cfg(test)]
impl QueuePlaybackPanel {
    /// The last completed frame's report (test evidence).
    pub(crate) fn last_report(&self) -> &QueuePlaybackFrameReport {
        &self.last_report
    }

    /// Whether the band reserves rows this frame (test evidence).
    pub(crate) fn shown(&self) -> bool {
        self.projection.shown
    }

    /// Whether the header row painted in the last completed frame (test
    /// evidence).
    pub(crate) fn header_painted(&self) -> bool {
        self.header.width > 0.0
    }

    /// The request the last gesture sent across the shell boundary (test
    /// evidence).
    pub(crate) fn last_request(&self) -> Option<&Msg> {
        self.last_request.as_ref()
    }
}

/// The transport request a seek fraction sends.
fn seek_msg(fraction: f64) -> Msg {
    Msg::Playback(PlaybackRequest::SeekTo(fraction))
}

/// One line of `text`, elided to `rect`'s width and clipped to it.
fn paint_elided(painter: &egui::Painter, rect: egui::Rect, text: &str, colour: egui::Color32) {
    let mut job = egui::text::LayoutJob::simple_singleline(
        text.to_owned(),
        egui::FontId::proportional(TEXT_SIZE),
        colour,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width());
    let galley = painter.layout_job(job);
    let origin = egui::Align2::LEFT_CENTER
        .anchor_size(rect.left_center(), galley.size())
        .min;
    painter.with_clip_rect(rect).galley(origin, galley, colour);
}

/// The header row: the playback-target indicator at the left (the icon and,
/// while a remote target owns playback, the host name after it) and the status
/// word at the right. The colours are the TUI header's roles
/// (`chrome_player/title/header.rs`). The header reports no region: the TUI's
/// indicator click (the F3 sessions shortcut) is not wired in this slice.
fn paint_header(painter: &egui::Painter, row: egui::Rect, projection: &QueuePlaybackProjection) {
    let (word, status_role) = status_word(projection.status);
    let status = painter.text(
        row.right_center(),
        egui::Align2::RIGHT_CENTER,
        word,
        egui::FontId::proportional(TEXT_SIZE),
        role_colour(status_role),
    );
    let icon_role = if projection.host_is_remote {
        Role::PlaybackTargetRemoteFg
    } else {
        Role::PlaybackTargetLocalFg
    };
    let icon = painter.text(
        row.left_center(),
        egui::Align2::LEFT_CENTER,
        TARGET_ICON,
        egui::FontId::proportional(TEXT_SIZE),
        role_colour(icon_role),
    );
    if projection.host_is_remote && !projection.host.is_empty() {
        let zone = egui::Rect::from_min_max(
            egui::pos2(icon.right() + HEADER_GAP, row.top()),
            egui::pos2(status.left() - HEADER_GAP, row.bottom()),
        );
        paint_elided(
            painter,
            zone,
            &projection.host,
            role_colour(Role::TextPrimary),
        );
    }
}

/// The status word and its theme role for one now-playing status: the TUI's
/// header vocabulary (`header_status_word`, `status_color`). This crate does not
/// link the TUI render crate, so the pair is spelled here.
fn status_word(status: NowPlayingStatus) -> (&'static str, Role) {
    match status {
        NowPlayingStatus::Playing => ("PLAYING", Role::Accent),
        NowPlayingStatus::Paused => ("PAUSED", Role::TextFocusAccent),
        NowPlayingStatus::Idle => ("IDLE", Role::TextMuted),
    }
}

/// The four transport controls in their cells, with the TUI's ASCII glyph
/// fallbacks (`render/components/chrome.rs`). A control the playback state
/// does not allow still paints, muted, so its cell stays a hit region.
fn paint_controls(
    painter: &egui::Painter,
    row: egui::Rect,
    projection: &QueuePlaybackProjection,
) -> HitMap<QueuePlaybackHitTarget> {
    let available = projection.availability;
    let paused = projection.playback.paused;
    let controls = [
        (
            QueuePlaybackHitTarget::PlayPause,
            if paused { ">" } else { "||" },
            if paused {
                Role::Accent
            } else {
                Role::TextFocusAccent
            },
        ),
        (
            QueuePlaybackHitTarget::Stop,
            "X",
            if available.stop {
                Role::StatusError
            } else {
                Role::TextMuted
            },
        ),
        (
            QueuePlaybackHitTarget::Previous,
            "<<",
            if available.previous {
                Role::TextStrong
            } else {
                Role::TextMuted
            },
        ),
        (
            QueuePlaybackHitTarget::Next,
            ">>",
            if available.next {
                Role::TextStrong
            } else {
                Role::TextMuted
            },
        ),
    ];
    let mut hit_map = HitMap::new();
    let mut left = row.left();
    for (target, glyph, role) in controls {
        let cell = egui::Rect::from_min_size(
            egui::pos2(left, row.top()),
            egui::vec2(CONTROL_WIDTH, ROW_HEIGHT),
        );
        left += CONTROL_WIDTH;
        painter.text(
            cell.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(TEXT_SIZE),
            role_colour(role),
        );
        hit_map.push(to_region(cell), target);
    }
    hit_map
}

/// The seek row: the elapsed time on the left, the bar, and the total time on
/// the right, the TUI's queue band layout. The bar paints a track alone while
/// the controls do not show playback, and its span is `None` then, so no seek
/// resolves against it. Otherwise it returns the bar's full-row hit span.
fn paint_seek_row(
    painter: &egui::Painter,
    row: egui::Rect,
    projection: &QueuePlaybackProjection,
) -> Option<egui::Rect> {
    let labels = projection.show_controls.then(|| {
        (
            time_label(projection.playback.position_ticks),
            time_label(projection.playback.runtime_ticks),
        )
    });
    let (left_inset, right_inset) = match labels.as_ref() {
        Some((elapsed, total)) => (
            label_width(painter, elapsed) + TIME_GAP,
            label_width(painter, total) + TIME_GAP,
        ),
        None => (0.0, 0.0),
    };
    let left = row.left() + left_inset;
    let bar = egui::Rect::from_min_max(
        egui::pos2(left, row.top()),
        egui::pos2((row.right() - right_inset).max(left), row.bottom()),
    );
    let track = egui::Rect::from_center_size(bar.center(), egui::vec2(bar.width(), BAR_THICKNESS));
    painter.rect_filled(track, 0.0, role_colour(Role::ProgressTrack));
    let (elapsed, total) = labels?;
    let filled = track.width() * f32::from(played_permille(projection.playback)) / 1000.0;
    painter.rect_filled(
        egui::Rect::from_min_size(track.min, egui::vec2(filled, track.height())),
        0.0,
        role_colour(Role::Accent),
    );
    painter.text(
        egui::pos2(row.left(), row.center().y),
        egui::Align2::LEFT_CENTER,
        elapsed,
        egui::FontId::proportional(TEXT_SIZE),
        role_colour(Role::Duration),
    );
    painter.text(
        egui::pos2(row.right(), row.center().y),
        egui::Align2::RIGHT_CENTER,
        total,
        egui::FontId::proportional(TEXT_SIZE),
        role_colour(Role::Duration),
    );
    Some(egui::Rect::from_min_max(
        egui::pos2(bar.left(), row.top()),
        egui::pos2(bar.right(), row.bottom()),
    ))
}

/// A tick count as the TUI's short duration label.
fn time_label(ticks: i64) -> String {
    fmt_duration_short(ticks / TICKS_PER_SECOND)
}

/// The painted width of one time label.
fn label_width(painter: &egui::Painter, text: &str) -> f32 {
    painter
        .layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(TEXT_SIZE),
            role_colour(Role::Duration),
        )
        .size()
        .x
}

/// The played share of the runtime in tenths of a percent, clamped to
/// `0..=1000`. A non-positive runtime plays nothing. Integer arithmetic keeps
/// the fill width free of float-to-integer casts.
fn played_permille(playback: PlaybackState) -> u16 {
    if playback.runtime_ticks <= 0 {
        return 0;
    }
    let permille = playback.position_ticks.saturating_mul(1000) / playback.runtime_ticks;
    u16::try_from(permille.clamp(0, 1000)).unwrap_or(1000)
}

/// The Queue playback band as the shared router's pointer surface. The band
/// never holds the GUI's focus in the first slice (the queue column forces the
/// transport band unfocused), so no key reaches it and it claims none. Its
/// pointer method is the seam the recognized gestures resolve through.
impl GuiSurfaceLeaf for QueuePlaybackPanel {
    fn id(&self) -> ComponentId {
        ComponentId::QueuePlaybackPanel
    }

    fn visual_selection_active(&self) -> bool {
        false
    }

    fn key(&mut self, _key: KeyEvent) -> LeafKeyResult {
        LeafKeyResult::Unhandled
    }

    fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg> {
        QueuePlaybackPanel::pointer(self, gesture)
    }
}

#[cfg(test)]
mod tests;
