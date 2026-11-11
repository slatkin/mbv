//! A track chip's popover App-side state: the open toggle and painted-row
//! activation (add-tauri-gui-client rows 6.4 and 6.5), the orphaned-popover
//! reconciliation, and the live choice-list construction both the frame
//! projection and the execution share.

use mbv_ui_msg::TrackChipKind;

use super::overlay::{GuiOverlay, TrackPopoverState};
use super::projection::TrackPopoverChoice;
use crate::app::App;
use crate::app::queue_playback_projection::{TrackChip, TrackChips};

impl App {
    /// Toggle a track chip's popover (the `OpenTrackPopover` request's
    /// App-side effect): the chip's press opens its popover, and the same
    /// press on the open popover's chip closes it, as the target chooser's
    /// press toggles (add-tauri-gui-client row 6.4). The popover opens only
    /// over real choices: the chips' source is the local player's stream,
    /// so an inactive player or a remote output has nothing to list. The
    /// cursor starts on the chosen choice.
    pub(in crate::app) fn toggle_gui_track_popover(&mut self, chip: TrackChipKind) {
        if matches!(&self.gui_overlay, Some(GuiOverlay::TrackPopover(popover)) if popover.chip == chip)
        {
            self.gui_overlay = None;
            return;
        }
        let Some(chips) = self.track_chips() else {
            return;
        };
        let choices = track_popover_choices(chip, &chips);
        let cursor = choices.iter().position(|choice| choice.chosen).unwrap_or(0);
        self.gui_overlay = Some(GuiOverlay::TrackPopover(TrackPopoverState { chip, cursor }));
    }

    /// Activate the open track popover's painted choice by its track id (the
    /// `TrackPopoverSelect` request's App-side effect, add-tauri-gui-client
    /// rows 6.4 and 6.5). The press names the mpv id the frame painted, so a
    /// track-list change between the paint and the press selects the painted
    /// track or nothing, never the neighbour; the id must still be one of
    /// the chip's live choices. Gated on the open overlay being a track
    /// popover — a stale row press while another overlay or none holds the
    /// slot executes nothing.
    pub(in crate::app) fn select_gui_track_popover_track(&mut self, track_id: i64) {
        let Some(GuiOverlay::TrackPopover(popover)) = &self.gui_overlay else {
            return;
        };
        let chip = popover.chip;
        let Some(choices) = self.track_popover_choices(chip) else {
            return;
        };
        if !choices.iter().any(|choice| choice.id == track_id) {
            return;
        }
        self.apply_track_choice(chip, track_id);
        self.gui_overlay = None;
    }

    /// Switch the chip's track to `track_id`: the one switch a track popover's
    /// Enter and a painted row's press both run.
    pub(super) fn apply_track_choice(&mut self, chip: TrackChipKind, track_id: i64) {
        match chip {
            TrackChipKind::Audio => self.set_audio_track(track_id),
            TrackChipKind::Subtitles => self.set_subtitle_track(track_id),
        }
    }

    /// Drop a track popover whose chip source is gone: the popover's
    /// choices are the local player's stream, so once the player goes
    /// inactive the frame projects no overlay for it
    /// (`gui_overlay_projection`'s `None`) and the held state must match —
    /// otherwise the snapshot still reports the overlay focus and the
    /// overlay's key rules keep consuming input the window does not paint.
    /// The GUI input entry reconciles before it builds the snapshot.
    pub(in crate::app) fn close_orphaned_track_popover(&mut self) {
        if matches!(self.gui_overlay, Some(GuiOverlay::TrackPopover(_)))
            && self.track_chips().is_none()
        {
            self.gui_overlay = None;
        }
    }

    /// The track popover's choice list for `chip`, read live: the audio chip
    /// lists the player's audio tracks; the subtitle chip lists Off and then
    /// the subtitle tracks (the `gui-client` spec). `None` while the local
    /// player is inactive — the chips' source is the local player's stream.
    pub(in crate::app) fn track_popover_choices(
        &self,
        chip: TrackChipKind,
    ) -> Option<Vec<TrackPopoverChoice>> {
        let chips = self.track_chips()?;
        Some(track_popover_choices(chip, &chips))
    }
}

/// The track popover's choice list for `chip` from the chips' projection
/// data: the audio chip lists the audio tracks; the subtitle chip lists Off
/// and then the subtitle tracks (the `gui-client` spec). One construction
/// site serves both the frame projection and the choice execution.
#[must_use]
pub(super) fn track_popover_choices(
    chip: TrackChipKind,
    chips: &TrackChips,
) -> Vec<TrackPopoverChoice> {
    match chip {
        TrackChipKind::Audio => popover_choices(&chips.audio, false),
        TrackChipKind::Subtitles => {
            let mut choices = vec![TrackPopoverChoice {
                id: 0,
                label: "Off".to_string(),
                forced: false,
                chosen: chips.subtitles.chosen.is_none(),
            }];
            choices.extend(popover_choices(&chips.subtitles, true));
            choices
        }
    }
}

/// The chip's own choices as popover choices, the `forced` flag passed
/// through and the chosen mark resolved against the chip's chosen id.
fn popover_choices(chip: &TrackChip, forced: bool) -> Vec<TrackPopoverChoice> {
    chip.choices
        .iter()
        .map(|choice| TrackPopoverChoice {
            id: choice.id,
            label: choice.label.clone(),
            forced,
            chosen: chip.chosen == Some(choice.id),
        })
        .collect()
}
