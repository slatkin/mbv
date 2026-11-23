//! The Service setup hand-offs the drain tests drive: the test seeds that
//! install a completed worker's result receiver on the `App` a test builds.

use super::*;
use crate::app::dispatch::session::service_startup::AudiobookshelfCatalogReceiver;

impl App {
    /// Hold `rx` as the Audiobookshelf setup attempt's result receiver, as the
    /// attempt's worker hand-off leaves it.
    #[cfg(any(test, feature = "test"))]
    pub(in crate::app) fn seed_audiobookshelf_setup_receiver_for_test(
        &mut self,
        rx: std::sync::mpsc::Receiver<AudiobookshelfSetupCompletion>,
    ) {
        self.setup.audiobookshelf_setup_rx = Some(rx);
    }

    /// Whether the Audiobookshelf setup attempt still holds a result receiver:
    /// whether the drain put a live worker's receiver back.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub(in crate::app) fn audiobookshelf_setup_receiver_present_for_test(&self) -> bool {
        self.setup.audiobookshelf_setup_rx.is_some()
    }

    /// Hold `rx` as the Audiobookshelf catalog load's result receiver, as the
    /// load's worker hand-off leaves it.
    #[cfg(any(test, feature = "test"))]
    pub(in crate::app) fn seed_audiobookshelf_catalog_receiver_for_test(
        &mut self,
        rx: AudiobookshelfCatalogReceiver,
    ) {
        self.setup.audiobookshelf_catalog_rx = Some(rx);
    }
}
