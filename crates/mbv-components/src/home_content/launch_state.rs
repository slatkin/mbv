//! Home launch-state extraction and restoration. Legacy Latest selectors
//! decode but resolve to the Home tree .

use mbv_config::{HomeSelectorKey, LibraryItemIdentity, SelectorIdentity, TuiLaunchState};

use super::HomeContent;

impl HomeContent {
    pub fn reanchor_launch_state_impl(&mut self, _state: &TuiLaunchState) -> bool {
        if self.loading && self.carrier.is_empty() {
            return false;
        }
        // Home's former Latest sections remain decodable in old launch
        // snapshots, but now resolve explicitly to the Home tree (design D8).
        // A saved item never restores (spec: every restart lands on the first
        // row), so a legacy snapshot's item is ignored here too.
        self.project_rows(self.now_secs);
        self.carrier.select_first();
        true
    }

    #[must_use]
    pub fn launch_snapshot_impl(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        // Home's persisted scope is fixed at Continue Watching; the selected
        // row is never recorded, so restoration always lands on the first
        // row.
        let selector = Some(SelectorIdentity::Home {
            key: HomeSelectorKey::Continue,
        });
        (selector, None)
    }
}
