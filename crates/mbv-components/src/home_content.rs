//! The Home destination's embedded content owner: a plain type — never
//! mounted, focused, subscribed, or given a `ComponentId` — that keeps its
//! Service content and the one shared canonical [`MediaListCarrier`] owner of
//! the Home flat list. The shell pushes the Continue Watching snapshot and the
//! merged `Recently added` rows with the current time; the owner projects them
//! into headings and item rows (`home_content/flat.rs`) and translates the
//! panel's slot events and forwarded chords into typed `Msg`s
//! (`home_content/keyboard.rs`).
//!
//! The selected item's hero comes from the shared `hero_content` producer for
//! its content type (Emby, ABS or Feeds), and its image state is the shell
//! projection's (task 5.10): this owner never fetches.

use tuirealm::event::KeyEvent;

use super::library_panel::HeroContentData;
use super::library_panel::content::{HeroContent, LibraryPanelContent, ListSlot};
use super::library_panel::hero::hero_content_queue;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::media_list::MediaListCarrier;
use mbv_config::{LibraryItemIdentity, SelectorIdentity};
use mbv_queue::QueueItem;
use mbv_render::components::tv_wide::HeroImageState;
use mbv_ui_model::home_latest::HomeRecentRow;
use mbv_ui_model::media_list::SelectionOrigin;
use mbv_ui_model::targets::HomeItemTarget;
use mbv_ui_msg::{LeafKeyResult, Msg};

mod flat;
mod keyboard;
mod launch_state;

#[cfg(test)]
mod tests;

/// The embedded content owner for the Home destination (design D2). Plain
/// type; the mounted `LibraryPanel` borrows it for content and slot events.
#[derive(Debug)]
pub struct HomeContent {
    continue_items: Vec<QueueItem>,
    /// The shared canonical owner of the Home rows: selection, viewport, and
    /// marks.
    carrier: MediaListCarrier<HomeItemTarget>,
    /// Shell-owned `Recently added` rows, merged and interleaved newest
    /// first; the projection reads them.
    recent_rows: Vec<HomeRecentRow>,
    /// The time the `Recently added` age groups were last measured from.
    now_secs: u64,
    loading: bool,
    /// The selection origin the panel records (status-bar summary only).
    selection_origin: Option<SelectionOrigin>,
    /// The projection's image state for the current hero (task 5.10): set by
    /// the shell, read by the painters through the panel content.
    hero_image: HeroImageState,
}

impl Default for HomeContent {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeContent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            carrier: MediaListCarrier::new(),
            recent_rows: Vec::new(),
            now_secs: 0,
            loading: false,
            selection_origin: None,
            hero_image: HeroImageState::None,
        }
    }

    /// Replace the shell-owned Continue Watching snapshot. The carrier
    /// preserves the selected target when it remains in the content.
    pub fn set_content(&mut self, continue_items: Vec<QueueItem>, loading: bool) {
        self.continue_items = continue_items;
        self.loading = loading;
        self.project_rows(self.now_secs);
    }

    /// Replace the shell-owned `Recently added` rows and the time their age
    /// groups are measured from.
    pub fn set_recent_rows(&mut self, recent_rows: Vec<HomeRecentRow>, now_secs: u64) {
        self.recent_rows = recent_rows;
        self.now_secs = now_secs;
        self.project_rows(now_secs);
    }

    /// The shell-merged `Recently added` rows in the order they arrive from
    /// the shell (newest first).
    #[must_use]
    pub fn recent_rows(&self) -> &[HomeRecentRow] {
        &self.recent_rows
    }

    fn project_rows(&mut self, now_secs: u64) {
        let rows = flat::flat_rows(&self.continue_items, &self.recent_rows, now_secs);
        self.carrier.set_content(rows);
    }

    /// Reset Home's local presentation without replacing its content: drop
    /// marks and land the selection on the first item row.
    pub fn reset_presentation(&mut self) {
        self.carrier.reset_presentation();
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.carrier.cursor()
    }

    /// The hero item: the selected row's item.
    fn current_item(&self) -> Option<QueueItem> {
        match self.carrier.selected_target()? {
            HomeItemTarget::Continue(item_id) => self
                .continue_items
                .iter()
                .find(|item| item.id() == item_id.as_str())
                .cloned(),
            HomeItemTarget::Recent(content_id) => self
                .recent_rows
                .iter()
                .find(|row| row.item.content_id() == *content_id)
                .map(|row| row.item.clone()),
        }
    }

    /// The hero item's content from the shared producer for its content type
    /// (design D5): Emby, ABS and Feeds items all flow through the one
    /// producer, so Home's row and its source tab render one set of facts.
    fn hero_content_data(&self) -> Option<HeroContentData> {
        self.current_item().as_ref().map(hero_content_queue)
    }
}

impl LibraryContentOwner for HomeContent {
    /// Forwards to the inherent content-preserving reset (design D3).
    fn reset_presentation(&mut self) {
        self.reset_presentation();
    }

    fn clear_selection(&mut self) {
        // The destination switch clears the shared owner's ordered marks so
        // no stale selection mark survives into a new destination.
        self.carrier.clear_owner_selection();
    }

    fn set_selection_origin(&mut self, origin: SelectionOrigin) {
        self.selection_origin = Some(origin);
    }

    fn selection_summary(&self) -> Option<mbv_ui_msg::SelectionSummary> {
        let count = self.carrier.multi_selection().len();
        let origin = self.selection_origin.clone()?;
        (count > 0).then_some(mbv_ui_msg::SelectionSummary { count, origin })
    }

    /// This frame's Home panel content. An empty list
    /// renders its placeholder.
    fn content(&mut self) -> LibraryPanelContent<'_> {
        let hero = self.hero_content_data().map(|data| {
            let mut facts = data.facts;
            facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts,
                overview: data.overview,
                credits: data.credits,
                overview_title: None,
                workspace: None,
            }
        });
        let selector = None;
        let empty = self.carrier.is_empty();
        let list = if empty {
            ListSlot::Empty {
                loading: self.loading,
                text: " (empty)".into(),
            }
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            list,
            hero,
        }
    }

    /// Translate resolved list gestures into Home's typed requests.
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::List(input) => self.on_list_slot_event(input),
            // Home has no Workspace and no hero-pane input of its own; the
            // selector pill is likewise inert here.
            LibrarySlotEvent::SelectorPicked(_)
            | LibrarySlotEvent::WorkspaceSelectorPicked(_)
            | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => self.activate_selected_row(),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        self.on_key_result(key)
    }

    /// Bounded read-only Continue Watching launch state.
    fn reanchor_launch_state(&mut self, state: &mbv_config::TuiLaunchState) -> bool {
        self.reanchor_launch_state_impl(state)
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        self.launch_snapshot_impl()
    }

    /// The current hero's content data, for the shell's image projection
    /// (task 5.10).
    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_content_data()
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
