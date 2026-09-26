use crate::app::dispatch::session::service_startup::{
    AudiobookshelfCatalogReceiver, AudiobookshelfPendingReplacement, AudiobookshelfSetupCompletion,
    AudiobookshelfStartupReceiver, SetupCompletion, Startup, StartupReceiver,
};
use crate::app::dispatch::session::services_settings::{AudiobookshelfSetupForm, EmbySetupForm};
use crate::app::state::app_init::AppInit;
use std::sync::mpsc;

/// Startup, setup, and replacement state for the remote Services.
pub(in crate::app) struct ServiceSetup {
    /// Worker for the initial Emby setup attempt.
    pub(in crate::app) emby_startup_rx: Option<StartupReceiver>,
    /// Config and generation queued for initial Emby startup.
    pub(in crate::app) emby_startup_request: Option<(
        crate::config::Config,
        mbv_core::service_runtime::SetupGeneration,
    )>,
    /// Worker for the initial Audiobookshelf setup attempt.
    pub(in crate::app) audiobookshelf_startup_rx: Option<AudiobookshelfStartupReceiver>,
    /// Config and generation queued for initial Audiobookshelf startup.
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
    pub(in crate::app) fn new(init: &mut AppInit) -> Self {
        Self {
            emby_startup_rx: init.emby_startup_rx.take(),
            emby_startup_request: init.emby_startup_request.take(),
            audiobookshelf_startup_rx: init.audiobookshelf_startup_rx.take(),
            audiobookshelf_startup_request: init.audiobookshelf_startup_request.take(),
            audiobookshelf_catalog_rx: None,
            audiobookshelf_test_rx: init.audiobookshelf_test_rx.take(),
            audiobookshelf_setup_rx: init.audiobookshelf_setup_rx.take(),
            emby_setup_form: init.emby_setup_form.take(),
            audiobookshelf_setup_form: None,
            emby_setup_rx: init.emby_setup_rx.take(),
            pending_emby_replacement: None,
            pending_audiobookshelf_replacement: None,
        }
    }
}
