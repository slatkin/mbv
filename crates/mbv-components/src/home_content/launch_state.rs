//! Home launch-state extraction and restoration. Legacy Latest selectors
//! decode but resolve to the Home tree (design D8; row 5.3 owns the full
//! restoration rewrite and its tests).

use mbv_config::{HomeSelectorKey, LibraryItemIdentity, SelectorIdentity, TuiLaunchState};
use mbv_ui_model::targets::HomeTreeTarget;

use super::HomeContent;
use crate::list::tree_browser::TreeOperation;

impl HomeContent {
    pub fn reanchor_launch_state_impl(&mut self, _state: &TuiLaunchState) -> bool {
        if self.loading && self.tree.roots().is_empty() {
            return false;
        }
        // Home's former Latest sections remain decodable in old launch
        // snapshots, but now resolve explicitly to the Home tree (design D8).
        // A saved item never restores (spec: every restart lands on the first
        // row), so a legacy snapshot's item is ignored here too.
        self.project_tree();
        self.select_first_item_target();
        true
    }

    /// Design D8: the first item row of the tree in display order. A stemless
    /// tree (both roots empty) leaves the selection untouched.
    pub(super) fn select_first_item_target(&mut self) {
        let first_item = self
            .tree
            .visible_targets()
            .into_iter()
            .find(|target| matches!(target, HomeTreeTarget::Item(_)));
        if let Some(target) = first_item {
            self.tree.apply(TreeOperation::Select(target));
        }
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

/// The launch snapshot's fixed scope; the tree-path restoration rules are
/// rows 5.3's interactive tests (the carrier tests they replaced exercised
/// the retired flat list).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continue_section_reports_the_fixed_scope_and_no_item() {
        // The persisted scope is fixed at Continue Watching and the selected
        // row is never recorded: restoration always lands on the first row.
        let owner = HomeContent::new();
        assert_eq!(
            owner.launch_snapshot_impl(),
            (
                Some(SelectorIdentity::Home {
                    key: HomeSelectorKey::Continue,
                }),
                None,
            )
        );
    }
}
