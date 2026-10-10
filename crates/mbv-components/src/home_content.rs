//! The Home destination's embedded content owner (design D2 of
//! `home-recently-added-tree`): a plain type — never mounted, focused,
//! subscribed, or given a `ComponentId` — that keeps its Service content and
//! the one shared canonical [`TreeBrowser`] owner of the Home tree (row 5.1).
//! The shell pushes the Continue Watching snapshot, the merged
//! `Recently added` rows, and the launch window; the owner projects them
//! into the shared tree (`home_content/tree.rs`) and translates the panel's
//! slot events and forwarded chords into the same typed `Msg`s the mounted
//! `HomeComponent` emitted (`home_content/keyboard.rs` holds the input
//! translation, row 5.2; row 5.3 owns the restoration/hero landing rules).
//!
//! The selected item's hero comes from the shared `hero_content` producer for
//! its content type (Emby, ABS or Feeds), and its image state is the shell
//! projection's (task 5.10): this owner never fetches.

use tuirealm::event::KeyEvent;

use super::library_panel::HeroContentData;
use super::library_panel::content::{HeroContent, LibraryPanelContent, ListSlot};
use super::library_panel::hero::hero_content_queue;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::list::tree_browser::{TreeBrowser, TreeOperation};
use mbv_config::{LibraryItemIdentity, SelectorIdentity};
use mbv_queue::QueueItem;
use mbv_render::components::tv_wide::HeroImageState;
use mbv_ui_model::home_latest::{HomeLatestLaunchWindow, HomeRecentRow};
use mbv_ui_model::media_list::SelectionOrigin;
use mbv_ui_model::targets::{HomeItemTarget, HomeTreeTarget};
use mbv_ui_msg::{LeafKeyResult, Msg};

mod keyboard;
mod launch_state;
mod tree;

use tree::TREE_STEMS;

#[cfg(test)]
mod tests;

/// The embedded content owner for the Home destination (design D2). Plain
/// type; the mounted `LibraryPanel` borrows it for content and slot events.
#[derive(Debug)]
pub struct HomeContent {
    continue_items: Vec<QueueItem>,
    /// The shared canonical owner of the Home tree (design D6): roots, age
    /// branches, items, selection, expansion, viewport, and marks.
    tree: TreeBrowser<HomeTreeTarget>,
    /// Shell-owned Home launch window; the `Recently added` projection
    /// branches and cuts against it (design D2).
    launch_window: HomeLatestLaunchWindow,
    /// Shell-owned `Recently added` rows, merged and interleaved newest
    /// first (design D2); the tree projection reads them.
    recent_rows: Vec<HomeRecentRow>,
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
            tree: TreeBrowser::new(),
            launch_window: HomeLatestLaunchWindow {
                previous: None,
                current: 0,
            },
            recent_rows: Vec::new(),
            loading: false,
            selection_origin: None,
            hero_image: HeroImageState::None,
        }
    }

    /// Replace the shell-owned Continue Watching snapshot. The tree
    /// reconciliation preserves the selected target when it remains in the
    /// content.
    pub fn set_content(&mut self, continue_items: Vec<QueueItem>, loading: bool) {
        self.continue_items = continue_items;
        self.loading = loading;
        self.project_tree();
    }

    /// Replace the shell-owned `Recently added` rows and the launch window
    /// they were merged against (design D2).
    pub fn set_recent_rows(
        &mut self,
        recent_rows: Vec<HomeRecentRow>,
        launch_window: HomeLatestLaunchWindow,
    ) {
        self.recent_rows = recent_rows;
        self.launch_window = launch_window;
        self.project_tree();
    }

    /// The shell-merged `Recently added` rows in the order they arrive from
    /// the shell (newest first).
    #[must_use]
    pub fn recent_rows(&self) -> &[HomeRecentRow] {
        &self.recent_rows
    }

    /// The shell-owned launch window the rows were merged against.
    #[must_use]
    pub fn launch_window(&self) -> HomeLatestLaunchWindow {
        self.launch_window
    }

    fn project_tree(&mut self) {
        // Expansion bookkeeping (design D6): a stem the tree did not hold
        // before this reconciliation starts expanded; a stem it already held
        // keeps whatever expansion state the user left it in.
        let newly_projected: Vec<HomeTreeTarget> = TREE_STEMS
            .iter()
            .filter(|target| self.tree.node(target).is_none())
            .cloned()
            .collect();
        let entries =
            tree::tree_entries(&self.continue_items, &self.recent_rows, self.launch_window);
        let reconciled = self.tree.reconcile(entries);
        // A rejected projection leaves the previous tree unchanged; the
        // projection is generated from settled shell snapshots, so a
        // rejection is a destination bug. Surface it loudly in debug builds
        // instead of freezing the tree behind a swallowed error.
        debug_assert!(
            reconciled.is_ok(),
            "home tree projection rejected: {:?}",
            reconciled.as_ref().err()
        );
        self.expand_collapsed_stems(&newly_projected);
    }

    /// Open every listed root and age branch that is projected and collapsed.
    /// `reset_presentation` passes every stem (the tree reset clears the
    /// expanded set); `project_tree` passes only the newly projected ones.
    fn expand_collapsed_stems(&mut self, stems: &[HomeTreeTarget]) {
        for target in stems {
            if self.tree.node(target).is_some() && !self.tree.is_expanded(target) {
                self.tree
                    .apply(TreeOperation::ToggleExpansionTarget(target.clone()));
            }
        }
    }

    /// The typed effect target for Home's current selection, resolved
    /// locally (design D1): `Root`/`Age` targets never cross the boundary,
    /// so only the selected item target is emitted. The action chords read
    /// the marked set through `keyboard.rs` (`action_item_targets`).
    pub(super) fn selected_item_target(&self) -> Option<HomeItemTarget> {
        match self.tree.selected_target()? {
            HomeTreeTarget::Item(target) => Some(target.clone()),
            HomeTreeTarget::Root(_) | HomeTreeTarget::Age(_) => None,
        }
    }

    /// Reset Home's local presentation without replacing its content: drop
    /// marks and any filter state, re-expand every root and branch, and land
    /// the selection on the first item row (design D8) — the launch
    /// presentation, never a structural row.
    pub fn reset_presentation(&mut self) {
        self.tree.reset_presentation();
        self.expand_collapsed_stems(&TREE_STEMS);
        self.select_first_item_target();
    }

    /// The selection's flow position in the visible row order (row 5.3's
    /// fate for the transitional flat-cursor read): the tree owner has no
    /// flat cursor, and the shell's prefix-mode tick tests read this as a
    /// movement counter — its only consumer.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.tree
            .visible_targets()
            .iter()
            .position(|target| Some(target) == self.tree.selected_target())
            .unwrap_or(0)
    }

    /// The selected row's first item: the row's own item, or — for a root or
    /// branch row — its first descendant item, so
    /// the hero never blanks while the selection moves over structural rows
    /// (design D8).
    fn first_item_below(&self, target: &HomeTreeTarget) -> Option<QueueItem> {
        match target {
            HomeTreeTarget::Item(HomeItemTarget::Continue(item_id)) => self
                .continue_items
                .iter()
                .find(|item| item.id() == item_id.as_str())
                .cloned(),
            HomeTreeTarget::Item(HomeItemTarget::Recent(content_id)) => self
                .recent_rows
                .iter()
                .find(|row| row.item.content_id() == *content_id)
                .map(|row| row.item.clone()),
            HomeTreeTarget::Root(_) | HomeTreeTarget::Age(_) => {
                let children = self.tree.children_of(target)?;
                children
                    .iter()
                    .find_map(|child| self.first_item_below(child))
            }
        }
    }

    /// The hero item: the selection's row item (design D8).
    fn current_item(&self) -> Option<QueueItem> {
        self.tree
            .selected_target()
            .and_then(|target| self.first_item_below(target))
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
        self.tree.apply(TreeOperation::ClearMarks);
    }

    fn set_selection_origin(&mut self, origin: SelectionOrigin) {
        self.selection_origin = Some(origin);
    }

    fn selection_summary(&self) -> Option<mbv_ui_msg::SelectionSummary> {
        // The tree keeps membership locally; expose only the count over the
        // marked items, the same read-only projection the Grouped Music
        // tree reports (the status bar never reconstructs membership).
        if self.tree.marked_targets().is_empty() {
            return None;
        }
        let count = self.tree.action_targets().len();
        let origin = self.selection_origin.clone()?;
        (count > 0).then_some(mbv_ui_msg::SelectionSummary { count, origin })
    }

    /// This frame's Home panel content. A tree with no projected roots
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
        let empty = self.tree.roots().is_empty();
        let list = if empty {
            ListSlot::Empty {
                loading: self.loading,
                text: " (empty)".into(),
            }
        } else {
            ListSlot::Media(&mut self.tree)
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
