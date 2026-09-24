//! Home Continue Watching sync and typed effects for the shell `Model`.

use super::components::home_content::HomeContent;
use super::components::library_panel::LibraryKey;
use super::components::ShellRequest;
use super::shell::Model;
use mbv_core::playback_queue::QueueItem;

impl Model {
    pub(super) fn handle_home_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::HomePlay(target) => {
                if let Some((item, from_cw)) = self.home_stable_target(&target) {
                    self.app.home_play_target(item, from_cw);
                }
            }
            ShellRequest::HomeEnqueue(target) => {
                if let Some((item, from_cw)) = self.home_stable_target(&target) {
                    self.app.home_enqueue_target(item, from_cw);
                }
            }
            ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Home(targets),
                anchor,
            ) => {
                let origin = crate::app::components::media_list::SelectionOrigin::Library(
                    crate::app::components::media_list::LibrarySelectionOrigin::Home,
                );
                self.context_menu_origin = Some(origin.clone());
                self.context_action_snapshot = Some(
                    crate::app::state::types::context_menu::ContextActionSnapshot {
                        origin,
                        values: vec![
                            crate::app::state::types::context_menu::ContextMenuTargets::Home(
                                targets.clone(),
                            ),
                        ],
                    },
                );
                let mut items = Vec::new();
                let mut removes = Vec::new();
                for target in &targets {
                    if let Some((QueueItem::Emby(item), true)) = self.home_stable_target(target) {
                        removes.push(
                            crate::app::state::types::context_menu::BulkRemoveTarget::ContinueWatching(
                                item.clone(),
                            ),
                        );
                        items.push(*item);
                    }
                }
                if targets.len() > 1 {
                    let capabilities = items
                        .iter()
                        .map(crate::app::state::context_menu_capabilities::emby_item_capabilities)
                        .collect();
                    self.app.open_context_menu_for_selection(
                        items,
                        anchor,
                        crate::app::PanelFocus::Library,
                        capabilities,
                        removes,
                    );
                    return;
                }
                let target = targets.into_iter().next();
                let item = target
                    .as_ref()
                    .and_then(|target| self.home_stable_target(target))
                    .and_then(|(item, _)| match item {
                        QueueItem::Emby(item) => Some(*item),
                        _ => None,
                    });
                self.home_context_item = item.clone();
                let cw_selected = target.is_some();
                if let Some((x, y)) = anchor {
                    self.app.open_context_menu_at(x, y, cw_selected, item);
                } else {
                    self.app.open_context_menu(cw_selected, item);
                }
            }
            ShellRequest::HomeDelete(target) => {
                if let Some((QueueItem::Emby(item), true)) = self.home_stable_target(&target) {
                    self.app.remove_from_continue_watching(*item);
                }
            }
            ShellRequest::HomeToggleWatched(target) => {
                if let Some((QueueItem::Emby(item), true)) = self.home_stable_target(&target) {
                    self.app.cw_toggle_watched(*item);
                }
            }
            _ => {}
        }
    }

    pub(super) fn update_home_owner<R>(
        &mut self,
        f: impl FnOnce(&mut HomeContent) -> R,
    ) -> Option<R> {
        self.update_library_owner(LibraryKey::Home, || Box::new(HomeContent::new()), f)
    }

    pub(super) fn push_home_content(&mut self) {
        let continue_items = self
            .home_content
            .continue_items
            .iter()
            .cloned()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        let loading = self.home_content.loading;
        self.update_home_owner(|home| home.set_content(continue_items, loading));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::msg::HomeRowTarget;
    use crate::app::tests::{make_app_stub, make_item, make_items};

    fn target(id: &str) -> HomeRowTarget {
        HomeRowTarget {
            item_id: Some(id.into()),
            source: None,
            from_continue_watching: true,
        }
    }

    #[test]
    fn continue_watching_effects_use_the_resolved_item_target() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        model.home_content.continue_items = make_items(3);

        model.handle_home_request(ShellRequest::HomeEnqueue(target("id2")));
        assert_eq!(model.app.player_tab.emby_items()[0].id, "id2");

        model.app.status.clear();
        model.handle_home_request(ShellRequest::HomePlay(target("id0")));
        assert_eq!(model.app.status, "Emby is unavailable");
    }

    #[test]
    fn continue_watching_context_menu_uses_explicit_target() {
        let mut model = Model::new(make_app_stub());
        let item = make_item("cw-target", "Movie");
        model.home_content.continue_items = vec![item.clone()];
        model.handle_home_request(ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Home(vec![target(
                &item.id,
            )]),
            None,
        ));
        let Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(menu)) =
            model.app.pending_overlay
        else {
            panic!("Home context-menu request must open a menu");
        };
        assert!(menu.entries.iter().any(|entry| {
            entry
                .action
                .as_ref()
                .is_some_and(|action| matches!(action, crate::app::ContextAction::Play))
        }));
    }
}
