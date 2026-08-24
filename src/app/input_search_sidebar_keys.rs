use super::{App, SidebarId};

impl App {
    /// Activate a search result: navigate to the item and close the sidebar.
    /// Called by the shell `Model` when `SearchSidebarComponent` emits
    /// `Msg::Shell(SearchActivate { id, item_type })` (task 3.2). The
    /// component owns the cursor and results; the shell owns the library
    /// tabs and navigation spawn.
    pub(super) fn activate_search_result(&mut self, item_id: String, item_type: String) {
        let libs = self.library_tabs_for_nav();
        self.spawn_navigate_to_item(item_id, item_type, libs);
        self.close_sidebar(SidebarId::Search);
    }
}

// The debounce and key-handling tests moved to the `SearchSidebarComponent`
// unit tests in `src/app/components/search_sidebar.rs` (task 3.2). The
// debounce is now component-owned, not App-owned.
