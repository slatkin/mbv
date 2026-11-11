//! The daemon-lost recovery dialog as the GUI presents it
//! (`daemon-disconnect-handling`: "An unannounced connection loss raises a
//! recovery dialog"): the projected diagnostics and button row, the chords
//! and presses that answer it, and where a failed restart's diagnostic line
//! lands.
//!
//! The answers themselves are the shared `App::apply_daemon_lost_intent` —
//! the same action the TUI's modal handler runs — so the GUI adds no
//! recovery path of its own. This module owns only the GUI's own
//! presentation: its held overlay and its press targets.

use mbv_ui_msg::{DaemonLostIntent, Key};

use super::overlay::GuiOverlay;
use super::projection::{GuiConfirmButtonProjection, GuiConfirmButtonTone};
use crate::app::App;
use crate::app::dispatch::session::daemon_lost::DaemonLostAnswer;

/// The dialog's choices in paint order: the intent the choice answers with,
/// the button's label, the key hint inside the pill, and the tone it paints
/// in. The TUI's hint line reads `[R] Restart and resume  [S] Restart, don't
/// resume  [Q] Quit`, and this one table feeds both the projected buttons
/// and the chords that answer them, so the pills and the keys cannot name
/// different choices.
const DAEMON_LOST_CHOICES: [(DaemonLostIntent, &str, &str, GuiConfirmButtonTone); 3] = [
    (
        DaemonLostIntent::RestartWithTray,
        "Restart and resume",
        "R",
        GuiConfirmButtonTone::Affirmative,
    ),
    (
        DaemonLostIntent::RestartWithoutTray,
        "Restart, don't resume",
        "S",
        GuiConfirmButtonTone::Affirmative,
    ),
    // Quit paints in the destructive tone, as the confirm modal's cancelling
    // button does: it ends this Client with no daemon to return to.
    (
        DaemonLostIntent::Quit,
        "Quit",
        "Q",
        GuiConfirmButtonTone::Cancel,
    ),
];

/// The dialog's button row as the frame carries it, in the same order as
/// [`DAEMON_LOST_CHOICES`].
pub(super) fn daemon_lost_buttons() -> Vec<GuiConfirmButtonProjection> {
    DAEMON_LOST_CHOICES
        .iter()
        .map(|(_, label, keys, tone)| GuiConfirmButtonProjection {
            label: (*label).to_string(),
            keys: (*keys).to_string(),
            tone: *tone,
        })
        .collect()
}

/// The choice a press on the button at `index` answers: the same entry the
/// frame painted there, and `None` for an index the row does not have.
pub(super) fn daemon_lost_choice(index: usize) -> Option<DaemonLostIntent> {
    DAEMON_LOST_CHOICES.get(index).map(|(intent, ..)| *intent)
}

/// The button index a chord answers, read from the table's key hints so the
/// chord and the painted pill name the same choice. The hint letters are
/// upper case and the chord arrives in either case (the TUI's component
/// accepts both), so the comparison folds case. Every other chord — Enter
/// and Esc included — names no choice: the dialog blocks all other input
/// until the user picks one, and the TUI binds no default answer.
pub(super) fn daemon_lost_entry_index(code: Key) -> Option<usize> {
    let Key::Char(typed) = code else {
        return None;
    };
    DAEMON_LOST_CHOICES.iter().position(|(_, _, hint, _)| {
        hint.chars()
            .next()
            .is_some_and(|hint| hint.eq_ignore_ascii_case(&typed))
    })
}

impl App {
    /// Answer the held daemon-lost dialog with the choice at `index`: the
    /// shared `App::apply_daemon_lost_intent`, whose success dismisses the
    /// dialog (`restart_local_daemon`'s own dismissal, which now clears the
    /// GUI's held overlay too) and whose failure leaves it up with the
    /// `Restart failed:` diagnostic the restart reported. Returns whether the
    /// Client's run should end, which only the quit answer asks. Gated on the
    /// open overlay being the dialog, so a stale press while another overlay
    /// or none holds the slot answers nothing.
    pub(in crate::app) fn apply_gui_daemon_lost_choice(&mut self, index: usize) -> bool {
        if !matches!(self.gui_overlay, Some(GuiOverlay::DaemonLost(_))) {
            return false;
        }
        let Some(intent) = daemon_lost_choice(index) else {
            return false;
        };
        match self.apply_daemon_lost_intent(intent) {
            DaemonLostAnswer::Restarted => false,
            DaemonLostAnswer::RestartFailed(error) => {
                if let Some(GuiOverlay::DaemonLost(modal)) = &mut self.gui_overlay {
                    modal.restart_error = Some(error);
                }
                false
            }
            DaemonLostAnswer::Quit(quit) => {
                self.gui_overlay = None;
                quit
            }
        }
    }

    /// Execute the daemon-lost button at `index` — the press on the region
    /// the front end reported. The press and the button's key hint answer the
    /// same choice (`apply_gui_daemon_lost_choice`), and Returns whether the
    /// Client's run should end.
    pub fn execute_gui_daemon_lost_button(&mut self, index: usize) -> bool {
        self.apply_gui_daemon_lost_choice(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dialog's chords are read from the painted pills' own hints, so the
    /// window's key hints cannot name a choice its keys do not answer.
    #[test]
    fn the_dialog_chords_answer_the_painted_buttons() {
        assert_eq!(
            daemon_lost_entry_index(Key::Char('r')),
            Some(0),
            "R restarts and resumes"
        );
        assert_eq!(
            daemon_lost_entry_index(Key::Char('S')),
            Some(1),
            "S restarts without resuming, in either case"
        );
        assert_eq!(daemon_lost_entry_index(Key::Char('q')), Some(2), "Q quits");
        assert_eq!(
            daemon_lost_choice(0),
            Some(DaemonLostIntent::RestartWithTray)
        );
        assert_eq!(
            daemon_lost_choice(1),
            Some(DaemonLostIntent::RestartWithoutTray)
        );
        assert_eq!(daemon_lost_choice(2), Some(DaemonLostIntent::Quit));
    }

    /// The dialog blocks every other input until a choice: the TUI binds no
    /// default answer, so Enter and Esc name none, and the buttons carry the
    /// label and hint the TUI's hint line paints.
    #[test]
    fn the_dialog_binds_no_default_answer() {
        assert_eq!(daemon_lost_entry_index(Key::Enter), None);
        assert_eq!(daemon_lost_entry_index(Key::Esc), None);
        assert_eq!(daemon_lost_entry_index(Key::Char('x')), None);
        let buttons = daemon_lost_buttons();
        let labels: Vec<&str> = buttons.iter().map(|b| b.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Restart and resume", "Restart, don't resume", "Quit"]
        );
        assert_eq!(buttons[0].keys, "R");
    }
}
