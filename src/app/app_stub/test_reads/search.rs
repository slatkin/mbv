//! The GUI Search data path's test seam (add-gui-library-panel row 12.1): one
//! search worker's completion on the App's own search channel, so a Client's
//! test owns every result its assertions see and no worker thread or network
//! runs behind it.
//!
//! The production query path spawns the TUI's own search worker, so a test
//! that needs a landed result without waiting on a thread pushes it here. The
//! App-side drain (`App::drain_gui_search_results`) is the code that applies
//! it, exactly as the GUI tick host runs it.

use super::super::App;
use mbv_emby::EmbyError;
use mbv_emby_model::EmbyItem;

impl App {
    /// Push one search worker's success onto the App's search channel (row
    /// 12.1), as `App::spawn_search_sidebar_query` sends it: the query the
    /// worker answered and the items it found. The result is applied only if
    /// `query` is the store's live query, which is the guard under test in the
    /// stale-result case.
    #[cfg(any(test, feature = "test"))]
    pub fn send_gui_search_result_for_test(&mut self, query: &str, items: Vec<EmbyItem>) {
        let _ = self.channels.search_tx.send((query.to_string(), Ok(items)));
    }

    /// Push one search worker's failure onto the App's search channel (row
    /// 12.1), so a test owns the error the store records without a network
    /// request behind it.
    #[cfg(any(test, feature = "test"))]
    pub fn send_gui_search_failure_for_test(&mut self, query: &str, error: EmbyError) {
        let _ = self
            .channels
            .search_tx
            .send((query.to_string(), Err(error)));
    }
}
