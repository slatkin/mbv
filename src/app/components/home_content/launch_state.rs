//! Home launch-state extraction and restoration. Legacy Latest selectors
//! decode but resolve to Continue Watching.

use mbv_core::config::{HomeSelectorKey, LibraryItemIdentity, SelectorIdentity, TuiLaunchState};

use super::HomeContent;

impl HomeContent {
    pub(super) fn reanchor_launch_state_impl(&mut self, state: &TuiLaunchState) -> bool {
        if self.loading && self.carrier.rows().is_empty() {
            return false;
        }
        // Home's former Latest sections remain decodable in old launch
        // snapshots, but now resolve explicitly to Continue Watching.
        self.project_continue_rows();
        let selected = match state.item.as_ref() {
            Some(LibraryItemIdentity::Home { id }) => self.carrier.select_target(id),
            _ => false,
        };
        if !selected {
            self.carrier.select_first();
        }
        true
    }

    pub(super) fn launch_snapshot_impl(
        &self,
    ) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let selector = Some(SelectorIdentity::Home {
            key: HomeSelectorKey::Continue,
        });
        let item = self
            .carrier
            .selected_target()
            .cloned()
            .map(|id| LibraryItemIdentity::Home { id });
        (selector, item)
    }
}

/// Home always persists Continue Watching; old section selectors remain
/// accepted by the config type and fall back to this fixed scope.
#[cfg(test)]
mod tests {
    use mbv_core::playback_queue::QueueItem;

    use super::*;
    use crate::app::tests::make_item;

    fn continue_owner(ids: &[&str]) -> HomeContent {
        let mut owner = HomeContent::new();
        owner.set_content(
            ids.iter()
                .map(|id| {
                    let mut item = make_item("Continue item", "Movie");
                    item.id = id.to_string();
                    QueueItem::Emby(Box::new(item))
                })
                .collect(),
            false,
        );
        owner
    }

    #[test]
    fn continue_section_reports_the_fixed_scope_and_first_item_target() {
        let owner = continue_owner(&["cw-1", "cw-2"]);
        assert_eq!(
            owner.launch_snapshot_impl(),
            (
                Some(SelectorIdentity::Home {
                    key: HomeSelectorKey::Continue,
                }),
                Some(LibraryItemIdentity::Home {
                    id: "cw-1".to_string(),
                })
            )
        );
    }

    #[test]
    fn saved_latest_section_falls_back_to_continue_watching() {
        let mut owner = continue_owner(&["continue-1"]);
        let state = TuiLaunchState {
            version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
            tab: mbv_core::config::TabIdentity::Home,
            panel_focus: mbv_core::config::LaunchPanelFocus::Library,
            selector: Some(SelectorIdentity::Home {
                key: HomeSelectorKey::Section("emby:removed-latest".into()),
            }),
            item: Some(LibraryItemIdentity::Home { id: "stale".into() }),
        };

        assert!(owner.reanchor_launch_state_impl(&state));
        assert_eq!(
            owner.launch_snapshot_impl().1,
            Some(LibraryItemIdentity::Home {
                id: "continue-1".into()
            })
        );
    }
}
