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

/// Row 5.3's tests (design D8): a legacy Latest launch snapshot lands on the
/// first item target, `reset_presentation` returns the launch presentation,
/// and the hero follows the selection's row item (a stem row's first
/// descendant item). The retired carrier-era tests are not ported.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_panel::owner::LibraryContentOwner;
    use mbv_config::{LaunchPanelFocus, TUI_LAUNCH_STATE_VERSION, TabIdentity};
    use mbv_emby_model::test_support::make_item;
    use mbv_queue::QueueItem;
    use mbv_ui_model::home_latest::{HomeLatestLaunchWindow, HomeRecentRow};
    use mbv_ui_model::targets::{HomeItemTarget, RecentAge};

    /// A recent epoch keeps the fixture arithmetic in range.
    fn window() -> HomeLatestLaunchWindow {
        let day = 24 * 60 * 60;
        HomeLatestLaunchWindow {
            previous: Some(1_500_000_000 - day),
            current: 1_500_000_000,
        }
    }

    fn continue_item(name: &str) -> QueueItem {
        let mut item = make_item(name, "Movie");
        item.id = name.to_string();
        QueueItem::Emby(Box::new(item))
    }

    /// One Emby item dated inside the launch window's `New` branch.
    fn new_item(name: &str) -> QueueItem {
        let mut item = make_item(name, "Movie");
        item.id = name.to_string();
        item.date_added = iso_secs(window().current - 60);
        QueueItem::Emby(Box::new(item))
    }

    fn iso_secs(secs: u64) -> String {
        use time::format_description::well_known::Iso8601;
        time::OffsetDateTime::from_unix_timestamp(i64::try_from(secs).unwrap_or(i64::MAX))
            .expect("in-range fixture timestamp")
            .format(&Iso8601::DEFAULT)
            .expect("ISO-8601 formatting always succeeds")
    }

    fn recent_row(item: QueueItem) -> HomeRecentRow {
        HomeRecentRow {
            item,
            feed_name: None,
        }
    }

    /// An owner whose `Continue watching` root holds the given items and
    /// whose `Recently added` root holds the `New` branch with one item.
    fn owner_with_tree() -> HomeContent {
        let mut owner = HomeContent::new();
        owner.set_content(vec![continue_item("alpha"), continue_item("beta")], false);
        owner.set_recent_rows(vec![recent_row(new_item("gamma"))], window());
        owner
    }

    /// A pre-tree launch snapshot: a Home Latest section selector and a
    /// saved item identity that no longer exists in the tree vocabulary.
    fn legacy_latest_state() -> TuiLaunchState {
        TuiLaunchState {
            version: TUI_LAUNCH_STATE_VERSION,
            tab: TabIdentity::Home,
            panel_focus: LaunchPanelFocus::Library,
            selector: Some(SelectorIdentity::Home {
                key: HomeSelectorKey::Section("emby:legacy".into()),
            }),
            item: Some(LibraryItemIdentity::Home { id: "beta".into() }),
        }
    }

    fn first_continue_target() -> HomeTreeTarget {
        HomeTreeTarget::Item(HomeItemTarget::continue_item("alpha"))
    }

    /// Design D8: the persisted scope is fixed at Continue Watching and the
    /// selected row is never recorded, so restoration always lands on the
    /// first row.
    #[test]
    fn continue_section_reports_the_fixed_scope_and_no_item() {
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

    /// Design D8 (spec scenarios "Legacy Home selection" and "Restart on
    /// Home"): a legacy Latest selector and its saved item never restore;
    /// the first Continue item target is selected.
    #[test]
    fn a_legacy_latest_selector_lands_on_the_first_continue_item() {
        let mut owner = owner_with_tree();

        let restored = owner.reanchor_launch_state_impl(&legacy_latest_state());

        assert!(restored);
        assert_eq!(owner.tree.selected_target(), Some(&first_continue_target()));
    }

    /// Design D8 / row 5.3: `reset_presentation` re-expands every stem and
    /// lands the selection on the first item target, whatever the user left
    /// collapsed or selected.
    #[test]
    fn reset_presentation_re_expands_branches_and_lands_on_the_first_item() {
        let mut owner = owner_with_tree();
        owner
            .tree
            .apply(TreeOperation::ToggleExpansionTarget(HomeTreeTarget::Age(
                RecentAge::New,
            )));
        owner.tree.apply(TreeOperation::Select(HomeTreeTarget::Item(
            HomeItemTarget::continue_item("beta"),
        )));

        owner.reset_presentation();

        assert!(owner.tree.is_expanded(&HomeTreeTarget::Age(RecentAge::New)));
        assert_eq!(owner.tree.selected_target(), Some(&first_continue_target()));
    }

    /// Design D8: a stem row's hero is its first descendant item, so the
    /// hero never blanks while the selection sits on a root or branch row.
    #[test]
    fn a_root_row_heros_its_first_descendant_item() {
        let mut owner = owner_with_tree();
        owner.tree.apply(TreeOperation::Select(HomeTreeTarget::Root(
            mbv_ui_model::targets::HomeRoot::Continue,
        )));

        let hero = owner.hero_data();

        assert_eq!(hero.expect("a stem row keeps a hero").facts.title, "alpha");
    }

    /// Design D8: a branch row's hero is its first descendant item, through
    /// the root → branch → item recursion.
    #[test]
    fn a_branch_row_heros_its_first_descendant_item() {
        let mut owner = owner_with_tree();
        owner
            .tree
            .apply(TreeOperation::Select(HomeTreeTarget::Age(RecentAge::New)));

        let hero = owner.hero_data();

        assert_eq!(hero.expect("a stem row keeps a hero").facts.title, "gamma");
    }

    /// Design D8: an item row's hero is that row's own item.
    #[test]
    fn an_item_row_heros_its_own_item() {
        let mut owner = owner_with_tree();
        owner.tree.apply(TreeOperation::Select(HomeTreeTarget::Item(
            HomeItemTarget::continue_item("beta"),
        )));

        let hero = owner.hero_data();

        assert_eq!(hero.expect("an item row has a hero").facts.title, "beta");
    }
}
