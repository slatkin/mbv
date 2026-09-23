//! The Home owner's bounded launch-state extraction and restoration (task
//! 2.1): the current section pill as a stable source key — Continue Watching
//! as the fixed scope, a latest section as its persisted `pref_key` — plus
//! the shared carrier's stable item target. No pill index or title crosses;
//! an empty section reports no item.

use mbv_core::config::{HomeSelectorKey, LibraryItemIdentity, SelectorIdentity, TuiLaunchState};

use super::HomeContent;

impl HomeContent {
    pub(super) fn reanchor_launch_state_impl(&mut self, state: &TuiLaunchState) -> bool {
        if self.loading && self.carrier.rows().is_empty() {
            return false;
        }
        // Home's former Latest sections remain decodable in old launch
        // snapshots, but now resolve explicitly to Continue Watching.
        let section = 0;
        self.section = section;
        self.clamp_section();
        self.project_active_section();
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
        let selector = if self.section == 0 {
            Some(SelectorIdentity::Home {
                key: HomeSelectorKey::Continue,
            })
        } else {
            self.source_for_section(self.section)
                .map(|source| SelectorIdentity::Home {
                    key: HomeSelectorKey::Section(source.pref_key()),
                })
        };
        let item = self
            .carrier
            .selected_target()
            .cloned()
            .map(|id| LibraryItemIdentity::Home { id });
        (selector, item)
    }
}

/// Task 2.1: the bounded read-only launch-state query on the Home owner.
/// The section pill resolves to a stable source key — Continue Watching as
/// the fixed scope, a latest section as its persisted `pref_key` — and the
/// item to the shared carrier's stable target. No section index or title
/// crosses; an empty section reports no item.
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use mbv_core::playback_queue::{FeedEntry, QueueItem};

    use super::*;
    use crate::app::tests::make_item;
    use crate::app::types_playback::{HomeLatestSection, HomeLatestSource};

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
            Vec::new(),
            false,
            HashMap::new(),
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
    fn reanchor_launch_state_falls_back_to_first_section_and_item() {
        let mut owner = HomeContent::new();
        let mut item = make_item("Latest", "Movie");
        item.id = "latest-1".into();
        owner.set_content(
            vec![QueueItem::Emby(Box::new(item))],
            Vec::new(),
            false,
            HashMap::new(),
        );
        let state = TuiLaunchState {
            version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
            tab: mbv_core::config::TabIdentity::Home,
            panel_focus: mbv_core::config::LaunchPanelFocus::Library,
            selector: Some(SelectorIdentity::Home {
                key: HomeSelectorKey::Section("emby:gone".into()),
            }),
            item: Some(LibraryItemIdentity::Home { id: "gone".into() }),
        };
        assert!(owner.reanchor_launch_state_impl(&state));
        assert_eq!(owner.section(), 0);
        assert_eq!(
            owner.launch_snapshot_impl().1,
            Some(LibraryItemIdentity::Home {
                id: "latest-1".into()
            })
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
        assert_eq!(owner.section(), 0);
        assert_eq!(
            owner.launch_snapshot_impl().1,
            Some(LibraryItemIdentity::Home {
                id: "continue-1".into()
            })
        );
    }

    #[test]
    fn latest_section_reports_its_source_key_not_its_index_or_title() {
        let mut episode = make_item("Pilot", "Episode");
        episode.id = "ep1".into();
        let mut owner = HomeContent::new();
        owner.set_content(
            Vec::new(),
            vec![HomeLatestSection::new(
                "Latest Movies".into(),
                HomeLatestSource::Emby("lib-movies".into()),
                vec![QueueItem::Emby(Box::new(episode))],
            )],
            false,
            HashMap::new(),
        );
        assert!(
            owner.restore_section(&HomeLatestSource::Emby("lib-movies".into())),
            "the pushed section must exist"
        );
        assert_eq!(
            owner.launch_snapshot_impl(),
            (
                Some(SelectorIdentity::Home {
                    key: HomeSelectorKey::Section("emby:lib-movies".to_string()),
                }),
                Some(LibraryItemIdentity::Home {
                    id: "ep1".to_string(),
                })
            )
        );
    }

    #[test]
    fn empty_home_reports_the_continue_scope_with_no_item() {
        let owner = continue_owner(&[]);
        assert_eq!(
            owner.launch_snapshot_impl(),
            (
                Some(SelectorIdentity::Home {
                    key: HomeSelectorKey::Continue,
                }),
                None,
            ),
            "an empty section still paints its pill, but selects no item"
        );
    }

    #[test]
    fn feeds_latest_section_reports_the_feeds_source_key() {
        let mut owner = HomeContent::new();
        owner.set_content(
            Vec::new(),
            vec![HomeLatestSection::new(
                "Latest Episodes".into(),
                HomeLatestSource::Feeds,
                vec![QueueItem::Feed(FeedEntry {
                    guid: "guid-Entry Title".into(),
                    title: "Entry Title".into(),
                    enclosure_url: None,
                    link: None,
                    mime_type: None,
                    duration_ticks: None,
                    pub_date_secs: None,
                    feed_kind: None,
                    feed_id: Some("https://example.com/feed.xml".into()),
                    position_ticks: 0,
                    played: false,
                })],
            )],
            false,
            HashMap::new(),
        );
        assert!(
            owner.restore_section(&HomeLatestSource::Feeds),
            "the pushed section must exist"
        );
        let (selector, item) = owner.launch_snapshot_impl();
        assert_eq!(
            selector,
            Some(SelectorIdentity::Home {
                key: HomeSelectorKey::Section("feeds".to_string()),
            })
        );
        assert_eq!(
            item,
            Some(LibraryItemIdentity::Home {
                id: "guid-Entry Title".to_string(),
            })
        );
    }
}
