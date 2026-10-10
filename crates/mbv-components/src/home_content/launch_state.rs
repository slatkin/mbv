//! Home launch-state extraction and restoration. Legacy Latest selectors
//! decode but resolve to the first Home item
//! (`openspec/specs/home-latest-sections`: "Home restoration lands on the
//! first item").

use mbv_config::{HomeSelectorKey, LibraryItemIdentity, SelectorIdentity, TuiLaunchState};

use super::HomeContent;

impl HomeContent {
    pub fn reanchor_launch_state_impl(&mut self, _state: &TuiLaunchState) -> bool {
        if self.loading && self.carrier.is_empty() {
            return false;
        }
        // Home's former Latest sections remain decodable in old launch
        // snapshots. A saved item never restores (spec: every restart lands
        // on the first item), so a legacy snapshot's item is ignored here too.
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

#[cfg(test)]
mod tests {
    use super::super::tests::{NOW, recent_row};
    use super::*;
    use crate::library_panel::owner::LibraryContentOwner;
    use mbv_config::{LaunchPanelFocus, TUI_LAUNCH_STATE_VERSION, TabIdentity};
    use mbv_emby_model::test_support::make_item;
    use mbv_queue::QueueItem;
    use mbv_ui_model::targets::HomeItemTarget;

    fn continue_item(name: &str) -> QueueItem {
        let mut item = make_item(name, "Movie");
        item.id = name.to_string();
        QueueItem::Emby(Box::new(item))
    }

    /// `Continue watching` holds alpha and beta; `New` holds gamma.
    fn owner_with_rows() -> HomeContent {
        let mut owner = HomeContent::new();
        owner.set_content(vec![continue_item("alpha"), continue_item("beta")], false);
        owner.set_recent_rows(vec![recent_row("gamma", 1)], NOW);
        owner
    }

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

    #[test]
    fn the_snapshot_reports_the_fixed_scope_and_no_item() {
        assert_eq!(
            HomeContent::new().launch_snapshot_impl(),
            (
                Some(SelectorIdentity::Home {
                    key: HomeSelectorKey::Continue,
                }),
                None,
            )
        );
    }

    #[test]
    fn a_legacy_latest_selector_lands_on_the_first_item() {
        let mut owner = owner_with_rows();
        owner.carrier.select_last();

        let restored = owner.reanchor_launch_state_impl(&legacy_latest_state());

        assert!(restored);
        assert_eq!(
            owner.selected_item_target(),
            Some(HomeItemTarget::continue_item("alpha"))
        );
    }

    #[test]
    fn reset_presentation_lands_on_the_first_item() {
        let mut owner = owner_with_rows();
        owner.carrier.select_last();

        owner.reset_presentation();

        assert_eq!(
            owner.selected_item_target(),
            Some(HomeItemTarget::continue_item("alpha"))
        );
    }

    #[test]
    fn the_hero_is_the_selected_item() {
        let mut owner = owner_with_rows();
        owner
            .carrier
            .select_target(&HomeItemTarget::continue_item("beta"));

        let hero = owner.hero_data();

        assert_eq!(hero.expect("an item row has a hero").facts.title, "beta");
    }
}
