use crate::app::dispatch::session::service_startup::{
    AudiobookshelfCatalogReceiver, AudiobookshelfPendingReplacement, AudiobookshelfSetupCompletion,
    AudiobookshelfStartupReceiver, SetupCompletion, Startup, StartupReceiver,
};
use crate::app::dispatch::session::services_settings::{AudiobookshelfSetupForm, EmbySetupForm};
use std::sync::mpsc;

/// Startup, setup, and replacement state for the remote Services.
pub(in crate::app) struct ServiceSetup {
    /// Worker for an Emby startup attempt, initial or retried.
    pub(in crate::app) emby_startup_rx: Option<StartupReceiver>,
    /// Config and generation queued for Emby startup.
    pub(in crate::app) emby_startup_request: Option<(
        crate::config::Config,
        mbv_core::service_runtime::SetupGeneration,
    )>,
    /// Worker for an Audiobookshelf startup attempt, initial or retried.
    pub(in crate::app) audiobookshelf_startup_rx: Option<AudiobookshelfStartupReceiver>,
    /// Config and generation queued for Audiobookshelf startup.
    pub(in crate::app) audiobookshelf_startup_request: Option<(
        crate::config::Config,
        mbv_core::service_runtime::SetupGeneration,
    )>,
    /// Result channel for loading the Audiobookshelf catalog.
    pub(in crate::app) audiobookshelf_catalog_rx: Option<AudiobookshelfCatalogReceiver>,
    /// Result channel for testing Audiobookshelf credentials.
    pub(in crate::app) audiobookshelf_test_rx: Option<AudiobookshelfStartupReceiver>,
    /// Result channel for saving Audiobookshelf setup.
    pub(in crate::app) audiobookshelf_setup_rx:
        Option<mpsc::Receiver<AudiobookshelfSetupCompletion>>,
    /// Active Emby setup form, if one is open.
    pub(in crate::app) emby_setup_form: Option<EmbySetupForm>,
    /// Active Audiobookshelf setup form, if one is open.
    pub(in crate::app) audiobookshelf_setup_form: Option<AudiobookshelfSetupForm>,
    /// Result channel for completing Emby setup.
    pub(in crate::app) emby_setup_rx: Option<mpsc::Receiver<SetupCompletion>>,
    /// Validated Emby setup awaiting replacement confirmation.
    pub(in crate::app) pending_emby_replacement: Option<Startup>,
    /// Validated Audiobookshelf setup awaiting replacement confirmation.
    pub(in crate::app) pending_audiobookshelf_replacement: Option<AudiobookshelfPendingReplacement>,
}

impl ServiceSetup {
    pub(in crate::app) fn new() -> Self {
        Self {
            emby_startup_rx: None,
            emby_startup_request: None,
            audiobookshelf_startup_rx: None,
            audiobookshelf_startup_request: None,
            audiobookshelf_catalog_rx: None,
            audiobookshelf_test_rx: None,
            audiobookshelf_setup_rx: None,
            emby_setup_form: None,
            audiobookshelf_setup_form: None,
            emby_setup_rx: None,
            pending_emby_replacement: None,
            pending_audiobookshelf_replacement: None,
        }
    }
}
