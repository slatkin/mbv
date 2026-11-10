//! The GUI Library's Search data path (add-gui-library-panel design D12, row
//! 12.1): the search results the core owns — the live query, whether it is
//! still out, whether it failed, and the result rows — and the query, close,
//! activate, and drain actions the GUI's top-bar field and tick host run.
//!
//! The query itself is the TUI's own worker: `App::spawn_search_sidebar_query`
//! sends `(query, Result<Vec<EmbyItem>, EmbyError>)` on the App's search
//! channel, and the TUI shell drains that channel into the mounted
//! `SearchSidebarComponent`. The GUI mounts no search component, so its tick
//! host drains the same channel through [`App::drain_gui_search_results`],
//! which applies the results to the store this module owns. Each Client owns
//! its own `App` and therefore its own channel pair, so one host is the only
//! drainer of its channel: the TUI's drain never runs beside the GUI's, and
//! neither can starve the other. The TUI's component state and its drain are
//! unchanged, and the TUI never reads this store.

use mbv_emby::EmbyError;
use mbv_emby_model::EmbyItem;
use mbv_ui_model::search_sidebar::is_navigable_type;

use crate::app::App;

/// The GUI Library's Search store (row 12.1).
///
/// The live query, whether its result is still out, whether the last result
/// failed, and the rows the frame paints. The core owns it, so the frame
/// carries it back; the GUI mounts no search surface to hold this state, as
/// the TUI's `SearchSidebarComponent` does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiSearchStore {
    /// The query the field last sent. A result applies only while its own
    /// query equals this one — the TUI's stale-query guard
    /// (`SearchSidebar::apply_drain`).
    query: String,
    /// Whether the query's result is still out.
    loading: bool,
    /// Whether the last applied result failed. The failed query keeps no
    /// rows, as the TUI's drain leaves its own list untouched but records the
    /// error.
    failed: bool,
    /// The result rows, in the order the provider returned them, filtered to
    /// the types the TUI's search sidebar shows.
    rows: Vec<GuiSearchRow>,
}

/// One row of the GUI Search results (row 12.1). The identity is the Emby
/// item's own id and type, never a row position, so an activate command built
/// from a stale frame addresses the row that frame painted or nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSearchRow {
    /// The Emby item id: the identity `App::gui_search_activate` navigates
    /// from.
    pub id: String,
    /// The Emby item type: the second half of the activate identity, and the
    /// fact the navigation spawn reads to pick the target library.
    pub item_type: String,
    /// The row's primary label: the item's name, or an episode's series name
    /// (`EmbyItem::display_name_parts`).
    pub title: String,
    /// The row's secondary label: an episode's own title, else the item type
    /// — the fact that tells a Series row from a Movie row.
    pub subtitle: Option<String>,
}

/// The Search projection the GUI Library panel paints (row 12.1): the live
/// query, whether its result is still out, whether it failed, and the result
/// rows. The rows paint below the field in the list-row look, and the TUI's
/// type-filter chips are dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiSearchProjection {
    /// The query the field shows: the store's live query, empty when no query
    /// is open.
    pub query: String,
    /// Whether the query's result is still out.
    pub loading: bool,
    /// Whether the last applied result failed.
    pub failed: bool,
    /// The result rows, in the provider's order.
    pub rows: Vec<GuiSearchRow>,
}

impl App {
    /// Send a query from the GUI's Search field (row 12.1), or clear the
    /// results on an empty one.
    ///
    /// A non-empty query stores the live query and starts the TUI's own
    /// search worker through `App::spawn_search_sidebar_query`; the result
    /// lands on the search channel and the tick host applies it through
    /// [`App::drain_gui_search_results`]. A Client with no Emby Service
    /// attached runs no search and logs the ignored command, as every
    /// ignored Library command is logged — never a silent empty result.
    pub fn gui_search_query(&mut self, query: String) {
        if query.trim().is_empty() {
            self.gui_search = GuiSearchStore::default();
            return;
        }
        self.gui_search.query.clone_from(&query);
        self.gui_search.loading = false;
        self.gui_search.failed = false;
        self.gui_search.rows.clear();
        let Some(client) = self.emby_snapshot() else {
            Self::log_ignored_library_command(
                "searchQuery",
                format_args!("no Emby Service is attached"),
            );
            return;
        };
        self.gui_search.loading = true;
        self.spawn_search_sidebar_query(client, query);
    }

    /// Close the GUI's Search results (row 12.1): Esc in the field. The live
    /// query, its rows, and its flags go, so the next query starts clean.
    pub fn gui_search_close(&mut self) {
        self.gui_search = GuiSearchStore::default();
    }

    /// Activate a result (row 12.1): Enter on the row, a press, or the
    /// double-click a front end sends with this same command.
    ///
    /// The navigation is the TUI's own (`App::spawn_navigate_to_item` over the
    /// library tabs), and the results close as the TUI's sidebar dismiss
    /// unmounts them. The GUI takes no overlay request: `pending_overlay`
    /// stays as it was, because no Client-side overlay mounts from this
    /// command (design D2, the GUI fronts its own results). An id and type
    /// pair no stored row carries is a stale frame's row and navigates
    /// nowhere.
    pub fn gui_search_activate(&mut self, item_id: String, item_type: String) {
        let known = self
            .gui_search
            .rows
            .iter()
            .any(|row| row.id == item_id && row.item_type == item_type);
        if !known {
            Self::log_stale_library_item("searchActivate", &item_id);
            return;
        }
        let libs = self.library_tabs_for_nav();
        self.gui_search = GuiSearchStore::default();
        self.spawn_navigate_to_item(item_id, item_type, libs);
    }

    /// Apply the landed search results to the store (row 12.1): the GUI tick
    /// host's drain, in place of the TUI shell's
    /// `Model::drain_search_results`.
    ///
    /// The TUI's drain stays the TUI's: it downcasts the mounted component
    /// and pushes the results there. This entry is the App-side half the GUI
    /// host runs instead, because no component is mounted in that Client.
    /// Returns whether a result applied — a stale one changes nothing.
    pub fn drain_gui_search_results(&mut self) -> bool {
        let mut applied = false;
        while let Ok((query, result)) = self.channels.search_rx.try_recv() {
            applied |= self.apply_gui_search_result(&query, result);
        }
        applied
    }

    /// The Search projection the GUI Library panel paints (row 12.1).
    #[must_use]
    pub fn gui_search_projection(&self) -> GuiSearchProjection {
        GuiSearchProjection {
            query: self.gui_search.query.clone(),
            loading: self.gui_search.loading,
            failed: self.gui_search.failed,
            rows: self.gui_search.rows.clone(),
        }
    }

    /// Apply one landed result (row 12.1). A result answering an older query
    /// changes nothing, the guard the TUI's `SearchSidebar::apply_drain`
    /// applies: keystrokes can dispatch a newer query while an older one is
    /// still in flight, so responses race on arrival order. A result for the
    /// live query ends its load and lands its navigable rows, or records the
    /// failure and keeps no rows.
    fn apply_gui_search_result(
        &mut self,
        query: &str,
        result: Result<Vec<EmbyItem>, EmbyError>,
    ) -> bool {
        if query != self.gui_search.query {
            return false;
        }
        self.gui_search.loading = false;
        match result {
            Ok(items) => {
                self.gui_search.failed = false;
                self.gui_search.rows = items
                    .iter()
                    .filter(|item| is_navigable_type(&item.item_type))
                    .map(gui_search_row)
                    .collect();
            }
            Err(error) => {
                self.gui_search.failed = true;
                self.gui_search.rows.clear();
                tracing::info!(
                    name: "gui_search.query.failed",
                    target: "gui_library",
                    query,
                    error = %error,
                    "the GUI Search query failed"
                );
            }
        }
        true
    }
}

/// One row of the results (row 12.1): the item's stable id and type for the
/// activate command, its display name as the primary label, and an episode's
/// own title — else its type — as the secondary one.
fn gui_search_row(item: &EmbyItem) -> GuiSearchRow {
    let (title, episode_title) = item.display_name_parts();
    GuiSearchRow {
        id: item.id.clone(),
        item_type: item.item_type.clone(),
        title,
        subtitle: episode_title.or_else(|| Some(item.item_type.clone())),
    }
}
