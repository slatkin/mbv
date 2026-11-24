#[cfg(test)]
use super::*;
// The whole-module prelude above names core items that the core's own test
// build alone has; only the queue view crosses the feature gate (design D4e).
#[cfg(all(not(test), feature = "test"))]
use super::QueueView;

// The terminal-size helpers (`set_terminal_size`, `set_terminal_width`) moved
// to the TUI shell's `test_support` (task 4.7): they write the shell `Model`'s
// size and push the single-panel fact the core reads.

// The suites themselves run only in the core's own test build: they name
// dev-dependencies (`rstest`, `uuid`) that a Client's `test`-feature build does
// not carry. The fixtures and the prelude below stay reachable in both builds
// (design D4e).
#[cfg(test)]
mod audiobookshelf_book_activation_tests;
#[cfg(test)]
mod audiobookshelf_browse_actions_sibling_tests;
#[cfg(test)]
mod audiobookshelf_context_menu;
#[cfg(test)]
mod audiobookshelf_mark;
#[cfg(test)]
mod audiobookshelf_runtime;
#[cfg(test)]
mod auto_reconnect;
#[cfg(test)]
mod browse_cache_caps_tests;
#[cfg(test)]
mod context_actions;
#[cfg(test)]
mod daemon_bootstrap;
#[cfg(test)]
mod feeds;
#[cfg(test)]
mod library_navigate_reveal;
#[cfg(test)]
mod library_position;
#[cfg(test)]
mod music_grouping;
#[cfg(test)]
mod named_queues;
#[cfg(test)]
mod output_handoff;
#[cfg(test)]
mod pin_swap;
#[cfg(test)]
mod podcast;
/// The whole-`App` fixtures a Client's tests build (`design D4b rule 5`).
pub mod render_fixtures;
/// The daemon endpoint and reconnect stubs the queue-link tests share.
pub mod route_state;
#[cfg(test)]
mod services_settings_lifecycle;
#[cfg(test)]
mod session_connect;
#[cfg(test)]
mod settings_activation;
#[cfg(test)]
mod split_browse_state_browse_level_tests;
pub use mbv_emby::test_support::make_session;
// `TestBackend`, `Terminal`, and `confirm_replace_queue` moved to the TUI
// shell's `test_support` (task 4.6c): they name the toolkit.
#[cfg(test)]
use mbv_emby_model::test_support::make_item;

// The App stub construction moved to `app_stub` so the Clients' own tests
// can build it too; the `crate::app::tests::*` paths stay working.
pub use super::app_stub::built::make_built_app;
pub use super::app_stub::{gui_client_capabilities, make_app_stub};
// The whole-`App` fixtures moved into `app_stub` so a Client's own tests reach
// them; the `crate::app::tests::*` prelude the TUI tests share stays working.
pub use super::{
    close_initial_services, emby_unified_state, install_test_emby, live_owner_channel,
    make_audio_items, make_audio_only_remote_app_stub_with_cmd_rx, make_items,
    make_local_daemon_app_stub, make_local_daemon_app_stub_with_cmd_rx,
    make_local_daemon_app_stub_with_cmd_rx_and_compat, make_remote_app_stub,
    make_remote_app_stub_with_cmd_rx, remote_stub_config,
};

// ── test helpers ─────────────────────────────────────────────────────────

pub trait QueueViewTestExt {
    fn adopt_items(&mut self, items: Vec<mbv_emby_model::EmbyItem>, cursor: usize);
    fn emby_items(&self) -> Vec<mbv_emby_model::EmbyItem>;
    fn adopt_queue_items(&mut self, items: Vec<mbv_queue::QueueItem>, cursor: usize);
    fn adopt_queue_items_with_active(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
        active_index: usize,
    );
    fn adopt_source(&mut self, source: mbv_queue::QueueSource);
}

impl QueueViewTestExt for QueueView {
    fn adopt_items(&mut self, items: Vec<mbv_emby_model::EmbyItem>, cursor: usize) {
        adopt_queue_view(
            self,
            items
                .into_iter()
                .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor,
            None,
            mbv_queue::QueueSource::Unknown,
        );
    }

    fn emby_items(&self) -> Vec<mbv_emby_model::EmbyItem> {
        self.slots()
            .iter()
            .filter_map(|slot| slot.item.as_emby().cloned())
            .collect()
    }

    fn adopt_queue_items(&mut self, items: Vec<mbv_queue::QueueItem>, cursor: usize) {
        adopt_queue_view(self, items, cursor, None, mbv_queue::QueueSource::Unknown);
    }

    fn adopt_queue_items_with_active(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
        active_index: usize,
    ) {
        adopt_queue_view(
            self,
            items,
            cursor,
            Some(active_index),
            mbv_queue::QueueSource::Unknown,
        );
    }

    fn adopt_source(&mut self, source: mbv_queue::QueueSource) {
        adopt_queue_view(
            self,
            self.slots().iter().map(|slot| slot.item.clone()).collect(),
            self.cursor(),
            None,
            source,
        );
    }
}

fn adopt_queue_view(
    view: &mut QueueView,
    items: Vec<mbv_queue::QueueItem>,
    cursor: usize,
    active_index: Option<usize>,
    source: mbv_queue::QueueSource,
) {
    let slots: Vec<_> = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: index as u64 + 1,
            item,
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        queue: None,
        start_slot: None,
        status: mbv_ctrl::player::PlayerStatus {
            current_idx: active_index.unwrap_or_default(),
            ..Default::default()
        },
        active_slot: active_index.and_then(|index| slots.get(index).map(|slot| slot.slot_id)),
        slots,
        revision: 1,
        source,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    view.adopt(
        &state,
        crate::app::state::queue_view::AdoptCause::Replacement,
    );
    view.set_cursor(cursor);
}

#[test]
fn emby_completion_applies_bootstrap_and_ready_state() {
    let mut app = make_app_stub();
    let client = mbv_emby::EmbyClient::new(crate::app::config::Config::default());
    let item = make_item("Ready item", "Audio");
    // Completion computes the Continue Watching snapshot; the shell assigns it.
    let content = app
        .apply_emby_completion_for_test(
            crate::app::dispatch::session::service_startup::Completion {
                generation: app.emby_setup_generation_for_test(),
                result: Ok(crate::app::dispatch::session::service_startup::Startup {
                    client,
                    bootstrap: mbv_emby::EmbyBootstrap {
                        continue_items: vec![item],
                        views: Vec::new(),
                    },
                    setup: mbv_config::EmbySetup::default(),
                }),
            },
        )
        .expect("Ok startup must bootstrap the Returned Home content");
    assert_eq!(
        app.emby_service_state_for_test(),
        mbv_core::service_runtime::ServiceState::Ready
    );
    assert!(app.emby_client_present_for_test());
    assert_eq!(content.continue_items.len(), 1);
    assert!(!content.loading);
}

#[test]
fn stale_emby_completion_does_not_change_runtime_or_home() {
    let mut app = make_app_stub();
    app.begin_emby_setup_for_test();
    let stale_generation = mbv_core::service_runtime::SetupGeneration::default();
    // Home content lives on the App (row 6.1): a stale completion returns no
    // snapshot, so the stored `home_content` stays untouched — the invariance
    // that used to be asserted on the Home content's `continue_items` here.
    let content = app.apply_emby_completion_for_test(
        crate::app::dispatch::session::service_startup::Completion {
            generation: stale_generation,
            result: Ok(crate::app::dispatch::session::service_startup::Startup {
                client: mbv_emby::EmbyClient::new(crate::app::config::Config::default()),
                bootstrap: mbv_emby::EmbyBootstrap::default(),
                setup: mbv_config::EmbySetup::default(),
            }),
        },
    );
    assert_eq!(
        app.emby_service_state_for_test(),
        mbv_core::service_runtime::ServiceState::Connecting
    );
    assert!(!app.emby_client_present_for_test());
    assert!(
        content.is_none(),
        "stale completion must not deliver a Home content snapshot"
    );
}
