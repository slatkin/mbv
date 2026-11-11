//! The Settings screen's test seams (`add-gui-settings-screen` rows 3.2 and
//! 3.3): the first-start request's App-side half, and the `[keys]` override a
//! rebound chord's test routes. The seams sit in their own child module so
//! `test_reads.rs` stays within its line ceiling, as the GUI Feeds and Search
//! seams do.

use super::super::App;

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
