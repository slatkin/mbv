//! The configuration and Settings facts a Client reads.
//!
//! Every read here takes the configuration lock for the one answer it builds,
//! so a Client never holds it while it paints.

use crate::app::App;
use mbv_ui_model::settings::SettingsDestination;

/// The fields a mounted setup form carries, as the Settings screen and its
/// tests read them: the form's own values, its focused field, its busy bit,
/// and the error it reports. The field count is the form's own (three for
/// Emby, two for Audiobookshelf), so the type keeps each Service's shape
/// rather than a length any caller could index past.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupFormDraft {
    Emby {
        fields: [String; 3],
        focus: usize,
        busy: bool,
        error: String,
    },
    Audiobookshelf {
        fields: [String; 2],
        focus: usize,
        busy: bool,
        error: String,
    },
}

impl SetupFormDraft {
    /// The error the form reports, empty while it reports none.
    #[must_use]
    pub fn error(&self) -> &str {
        match self {
            SetupFormDraft::Emby { error, .. } | SetupFormDraft::Audiobookshelf { error, .. } => {
                error
            }
        }
    }
}

impl App {
    /// The Emby setup form's draft, while Settings shows one.
    #[must_use]
    pub fn emby_setup_draft(&self) -> Option<SetupFormDraft> {
        self.setup
            .emby_setup_form
            .as_ref()
            .map(|form| SetupFormDraft::Emby {
                fields: form.fields.clone(),
                focus: form.focus,
                busy: form.busy,
                error: form.error.clone(),
            })
    }

    /// The Audiobookshelf setup form's draft, while Settings shows one.
    #[must_use]
    pub fn audiobookshelf_setup_draft(&self) -> Option<SetupFormDraft> {
        self.setup
            .audiobookshelf_setup_form
            .as_ref()
            .map(|form| SetupFormDraft::Audiobookshelf {
                fields: form.fields.clone(),
                focus: form.focus,
                busy: form.busy,
                error: form.error.clone(),
            })
    }

    /// The Settings screen's destination: which pane it shows.
    #[must_use]
    pub fn settings_destination(&self) -> SettingsDestination {
        self.settings_destination
    }

    /// Whether a Settings save is deferred and still owed: the write waits for
    /// the save deadline an explicit save or the teardown flushes.
    #[must_use]
    pub fn settings_save_armed(&self) -> bool {
        self.settings_save_at.is_some()
    }

    /// The mounted Emby setup form, while Settings shows it.
    #[must_use]
    pub fn emby_setup_form_present(&self) -> bool {
        self.setup.emby_setup_form.is_some()
    }

    /// The mounted Audiobookshelf setup form, while Settings shows it.
    #[must_use]
    pub fn audiobookshelf_setup_form_present(&self) -> bool {
        self.setup.audiobookshelf_setup_form.is_some()
    }

    /// The configured feed subscriptions.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn configured_feeds(&self) -> Vec<mbv_config::FeedSubscription> {
        self.config.lock().unwrap().feeds.clone()
    }

    /// How many feed subscriptions are configured.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn configured_feed_count(&self) -> usize {
        self.config.lock().unwrap().feeds.len()
    }

    /// The configured feed subscription at `index`.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn configured_feed_at(&self, index: usize) -> Option<mbv_config::FeedSubscription> {
        self.config.lock().unwrap().feeds.get(index).cloned()
    }

    /// The configured feed subscription's URL at `index`.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn configured_feed_url_at(&self, index: usize) -> Option<String> {
        self.config
            .lock()
            .unwrap()
            .feeds
            .get(index)
            .map(|subscription| subscription.url.clone())
    }

    /// Whether a feed-manage surface is open.
    #[must_use]
    pub fn feeds_manage_open(&self) -> bool {
        self.feeds_manage.is_some()
    }

    /// The in-flight add-feed marker the feed-manage surface paints.
    #[must_use]
    pub fn feeds_manage_pending_add(&self) -> Option<u64> {
        self.feeds_manage.as_ref().and_then(|p| p.pending_add)
    }

    /// The keybinding set the configuration names, as a Client captures it at
    /// construction: later config saves never rewrite a running session's
    /// bindings.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn configured_keybinds(&self) -> mbv_keybinds::Keybinds {
        self.config.lock().unwrap().keybinds.clone()
    }

    /// The configured Emby username, as the status row shows it, or an empty
    /// string when none is configured.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn configured_username(&self) -> String {
        self.config.lock().unwrap().username.clone()
    }

    /// Whether this launch is configured to stay alive after its last Client
    /// quits.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn stay_alive_configured(&self) -> bool {
        self.config.lock().unwrap().stay_alive
    }

    /// Whether the terminal mouse capture preference is on.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn mouse_support_configured(&self) -> bool {
        self.config.lock().unwrap().mouse_support
    }

    /// How long a quit waits for the Owner to answer before the process
    /// exits anyway.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn quit_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.config.lock().unwrap().quit_timeout_secs)
    }

    /// A copy of the configuration this App was built with.
    ///
    /// The lock is held only for the clone, so a Client never holds it while
    /// it builds a surface. Every write goes through the core, which keeps the
    /// config consistent with the persisted sections it saves.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned, as every core reader
    /// of it already does.
    #[must_use]
    pub fn config_snapshot(&self) -> crate::app::config::Config {
        self.config.lock().unwrap().clone()
    }
}
