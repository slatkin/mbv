use mbv_core::api::EmbyClient;

use crate::app::App;

impl App {
    pub(in crate::app) fn spawn_search_sidebar_query(&self, client: EmbyClient, query: String) {
        let tx = self.search_tx.clone();
        std::thread::spawn(move || {
            let result = client.search_items(&query, 100);
            let _ = tx.send((query, result));
        });
    }
}
