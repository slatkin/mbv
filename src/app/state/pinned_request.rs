//! The one panel request that may be in flight, and the drain that applies
//! its outcome (design D4, D6, D7).
//!
//! A request runs on a worker thread, so the TUI never waits for pinwin's
//! bounded child wait. The pending record is both the data the outcome needs
//! (the `x` width or the F2 row and its candidate) and the guard that ignores
//! further `x` and F2 steps until that outcome arrives.

use crate::app::App;
use mbv_config::PanelConfig;
use mbv_ui_model::settings::{PinnedWidth, SettingKey};

/// What the running `pinwin` request will apply when its outcome arrives.
#[derive(Debug)]
pub(in crate::app) enum PinnedRequestPending {
    /// One `x` press: the view to switch to.
    View(PinnedWidth),
    /// One F2 Panel step: the row, the width it validated (if any), and the
    /// candidate that is saved only when the request is accepted.
    Setting {
        key: SettingKey,
        edited_width: Option<PinnedWidth>,
        candidate: PanelConfig,
    },
}

impl App {
    /// Start one panel request and hold its pending record. A spawn failure
    /// clears the record and returns its reason, so the caller flashes it
    /// through the same failure handler a refused request uses.
    pub(in crate::app) fn begin_pinned_request(
        &mut self,
        pending: PinnedRequestPending,
        config: &PanelConfig,
        request: crate::pin::PinnedSettingRequest,
    ) -> Result<(), String> {
        self.pinned_request_pending = Some(pending);
        match crate::pin::spawn_pinned_request(config, request, self.channels.pinned_tx.clone()) {
            Ok(()) => Ok(()),
            Err(reason) => {
                self.pinned_request_pending = None;
                Err(reason)
            }
        }
    }

    /// Drain a queued panel outcome and apply it through the same success and
    /// failure handlers the request used before it moved to a worker.
    pub(in crate::app) fn drain_pinned_requests(&mut self) -> bool {
        let mut produced = false;
        while let Ok(outcome) = self.channels.pinned_rx.try_recv() {
            produced = true;
            self.apply_pinned_outcome(outcome);
        }
        produced
    }

    fn apply_pinned_outcome(&mut self, outcome: Result<(), String>) {
        let Some(pending) = self.pinned_request_pending.take() else {
            return;
        };
        match pending {
            PinnedRequestPending::View(width) => self.apply_pinned_view(width, outcome),
            PinnedRequestPending::Setting {
                key,
                edited_width,
                candidate,
            } => self.apply_panel_setting_outcome(true, key, edited_width, candidate, outcome),
        }
    }
}
