//! The Settings screen's test seams (`add-gui-settings-screen` rows 3.2 and
//! 3.3): the first-start request's App-side half, and the `[keys]` override a
//! rebound chord's test routes. The seams sit in their own child module so
//! `test_reads.rs` stays within its line ceiling, as the GUI Feeds and Search
//! seams do.

use super::super::App;

impl App {
    /// The configuration the Settings screen's edits land in (row 5.1): an
    /// accepted `activate` changes it, a refused one leaves it alone.
    #[cfg(feature = "test")]
    #[must_use]
    pub fn gui_settings_config_for_test(&self) -> mbv_config::Config {
        self.config_snapshot()
    }

    /// Whether the deferred settings save is armed (row 5.1): an accepted
    /// `activate` arms it, a refused one does not.
    #[cfg(feature = "test")]
    #[must_use]
    pub fn gui_settings_save_armed_for_test(&self) -> bool {
        self.settings_save_at.is_some()
    }

    /// The open setup form's server URL (row 5.2), whichever form holds it.
    /// The password and API key never leave the core, so nothing here carries
    /// one.
    #[cfg(feature = "test")]
    #[must_use]
    pub fn gui_setup_server_url_for_test(&self) -> Option<String> {
        self.setup
            .emby_setup_form
            .as_ref()
            .map(|form| form.fields[0].clone())
            .or_else(|| {
                self.setup
                    .audiobookshelf_setup_form
                    .as_ref()
                    .map(|form| form.fields[0].clone())
            })
    }

    /// Whether the open setup form reports its validation in progress
    /// (row 5.2).
    #[cfg(feature = "test")]
    #[must_use]
    pub fn gui_setup_busy_for_test(&self) -> bool {
        self.setup
            .emby_setup_form
            .as_ref()
            .map(|form| form.busy)
            .or_else(|| {
                self.setup
                    .audiobookshelf_setup_form
                    .as_ref()
                    .map(|form| form.busy)
            })
            .unwrap_or(false)
    }

    /// Whether the open setup form started a setup generation (row 5.2).
    #[cfg(feature = "test")]
    #[must_use]
    pub fn gui_setup_generation_started_for_test(&self) -> bool {
        self.setup
            .emby_setup_form
            .as_ref()
            .and_then(|form| form.generation)
            .or_else(|| {
                self.setup
                    .audiobookshelf_setup_form
                    .as_ref()
                    .and_then(|form| form.generation)
            })
            .is_some()
    }

    /// Put the Emby Service in `state`, for a Client's test that asserts a
    /// Service action (row 5.2): a removal needs a configured Service.
    #[cfg(feature = "test")]
    pub fn set_gui_emby_service_state_for_test(
        &mut self,
        state: mbv_core::service_runtime::ServiceState,
    ) {
        self.emby_runtime.state = state;
    }

    /// Make the Emby Service Ready with `client`, for a Client's test whose
    /// action fetches through it (row 5.5). Built over the in-memory mock
    /// transport, so no request leaves the process.
    #[cfg(feature = "test")]
    pub fn set_gui_emby_client_for_test(&mut self, client: mbv_emby::EmbyClient) {
        self.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );
    }

    /// Seed the configured feed subscriptions (row 5.3) with `(name, url)`
    /// rows, as a configuration that already holds subscriptions loads them.
    ///
    /// # Panics
    /// Panics if the `config` lock is poisoned: a previous holder panicked
    /// while holding it.
    #[cfg(feature = "test")]
    pub fn seed_gui_feed_subscriptions_for_test(&mut self, subscriptions: &[(&str, &str)]) {
        self.config.lock().unwrap().feeds = subscriptions
            .iter()
            .map(|(name, url)| mbv_config::FeedSubscription {
                name: (*name).to_string(),
                url: (*url).to_string(),
                kind: mbv_queue::FeedKind::Audio,
            })
            .collect();
    }

    /// Seed My languages (row 9.2): a configuration that already holds them
    /// projects the Subtitle language and Audio language pick lists as `""`
    /// (Any) plus these entries.
    ///
    /// # Panics
    /// Panics if the `config` lock is poisoned: a previous holder panicked
    /// while holding it.
    #[cfg(feature = "test")]
    pub fn seed_gui_my_languages_for_test(&mut self, languages: &[&str]) {
        self.config.lock().unwrap().my_languages = languages
            .iter()
            .map(|language| (*language).to_string())
            .collect();
    }

    /// Seed the saved playlist list with `(id, name)` rows (row 5.5), as the
    /// Playlists section lists them.
    #[cfg(feature = "test")]
    pub fn seed_gui_playlists_for_test(&mut self, playlists: &[(&str, &str)]) {
        self.playlists = playlists
            .iter()
            .map(|(id, name)| {
                let mut item = mbv_emby_model::test_support::make_item(name, "Playlist");
                item.id = (*id).into();
                item
            })
            .collect();
    }

    /// Inject one successful feed-add result as the worker's channel would
    /// deliver it (row 5.3): the pending id matches, so the next tick host
    /// drain appends the subscription.
    ///
    /// # Panics
    /// Panics if the injected result's channel is closed, which it cannot be:
    /// the receiver was created with the sender in the same popup.
    #[cfg(feature = "test")]
    pub fn inject_gui_feed_add_result_for_test(
        &mut self,
        name: &str,
        url: &str,
        kind: mbv_queue::FeedKind,
    ) {
        let mut popup = mbv_ui_model::feeds_manage::FeedsManagePopup::new();
        let id = popup.next_add_id;
        popup.next_add_id += 1;
        popup.pending_add = Some(id);
        popup
            .add_tx
            .send(mbv_ui_model::feeds_manage::FeedAddResult {
                id,
                name: name.to_string(),
                url: url.to_string(),
                kind,
                result: Ok(()),
            })
            .expect("the injected result's channel is alive");
        self.feeds_manage = Some(popup);
    }
}

impl App {
    /// Arm the first-start Settings request (`add-gui-settings-screen`
    /// design D2): the App-side half of `construct/remote.rs`'s
    /// `should_open_services` open, as a start with no configured Remote
    /// Service and no feed subscriptions lands it. A Client's test drains it
    /// through the production tick path.
    #[cfg(feature = "test")]
    pub fn seed_first_start_settings_for_test(&mut self) {
        self.open_services_settings();
    }

    /// Fire `action_id` on `chord` instead of its default (`add-gui-settings-screen`
    /// rows 3.2, 3.3): the `[keys]` override a user's rebind writes, for a
    /// Client's test that routes a rebound chord.
    ///
    /// # Panics
    /// Panics if `chord` does not parse or `action_id` is not a declared
    /// action: a test's chord and action are literals.
    #[cfg(feature = "test")]
    pub fn rebind_chord_for_test(&mut self, action_id: &'static str, chord: &str) {
        let parsed = mbv_keybinds::Chord::parse(chord).expect("test chord must parse");
        let action = mbv_keybinds::action_by_id(action_id)
            .unwrap_or_else(|| panic!("{action_id} is a declared action"));
        let keybinds = &mut self.config.lock().unwrap().keybinds;
        match keybinds
            .sections
            .iter_mut()
            .find(|(section, _)| *section == action.section)
        {
            Some((_, bindings)) => bindings.router.push((action_id, parsed)),
            None => keybinds.sections.push((
                action.section,
                mbv_keybinds::SectionBindings {
                    router: vec![(action_id, parsed)],
                    prefix: vec![],
                },
            )),
        }
    }
}
