//! The Home destination's embedded content owner (design D2 of
//! `home-recently-added-tree`): a plain type — never mounted, focused,
//! subscribed, or given a `ComponentId` — that keeps its Service content and
//! the one shared canonical [`TreeBrowser`] owner of the Home tree (row 5.1).
//! The shell pushes the Continue Watching snapshot, the merged
//! `Recently added` rows, and the launch window; the owner projects them
//! into the shared tree (`home_content/tree.rs`) and translates the panel's
//! slot events and forwarded chords into the same typed `Msg`s the mounted
//! `HomeComponent` emitted (rows 5.2 and 5.3 finish the port: the visual-mode
//! chords the flat carrier interpreted, the marked-selection requests, and
//! the restoration/hero landing rules still live here as stated seams).
//!
//! The selected item's hero comes from the shared `hero_content` producer for
//! its content type (Emby, ABS or Feeds), and its image state is the shell
//! projection's (task 5.10): this owner never fetches.

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::library_panel::HeroContentData;
use super::library_panel::content::{HeroContent, LibraryPanelContent, ListSlot};
use super::library_panel::hero::hero_content_queue;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::list::tree_browser::{TreeBrowser, TreeExternalIntent, TreeOperation};
use super::media_list::MediaListSurfaceInput;
use mbv_config::{LibraryItemIdentity, SelectorIdentity};
use mbv_queue::QueueItem;
use mbv_render::components::tv_wide::HeroImageState;
use mbv_ui_model::context_menu::ContextMenuTargets;
use mbv_ui_model::home_latest::{HomeLatestLaunchWindow, HomeRecentRow};
use mbv_ui_model::media_list::SelectionOrigin;
use mbv_ui_model::targets::{HomeItemTarget, HomeTreeTarget};
use mbv_ui_msg::{LeafKeyResult, Msg, ShellRequest};

mod launch_state;
mod tree;

use tree::TREE_STEMS;

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
        // keeps whatever expansion state the user left it in. The tree's own
        // reconciliation preserves its expanded set for surviving nodes, so
        // only the newly projected stems need a toggle here.
        let held_before: Vec<bool> = TREE_STEMS
            .iter()
            .map(|target| self.tree.node(target).is_some())
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
        for (held_before, target) in held_before.iter().zip(TREE_STEMS.iter()) {
            if !held_before && self.tree.node(target).is_some() && !self.tree.is_expanded(target) {
                self.tree
                    .apply(TreeOperation::ToggleExpansionTarget(target.clone()));
            }
        }
    }

    /// Re-expand every projected root and age branch (the
    /// `reset_presentation` rule user 5.3 refines): the tree reset clears
    /// the expanded set, so the stems re-open and the first item stays the
    /// presentation default.
    fn expand_projected_stems(&mut self) {
        for target in &TREE_STEMS {
            if self.tree.node(target).is_some() && !self.tree.is_expanded(target) {
                self.tree
                    .apply(TreeOperation::ToggleExpansionTarget(target.clone()));
            }
        }
    }

    /// Reset Home's local presentation without replacing its content: drop
    /// marks and any filter state, re-expand every root and branch, and let
    /// the tree return its selection to the first visible row.
    pub fn reset_presentation(&mut self) {
        self.tree.reset_presentation();
        self.expand_projected_stems();
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        // Transitional flat-cursor read (row 5.2/5.3 own the moving parts);
        // the tree owner has no flat cursor, so the selection's flow position
        // is reported instead. No consumer of Home's flat cursor remains.
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

    /// The typed effect target for Home's current selection, resolved
    /// locally (design D1): `Root`/`Age` targets never cross the boundary,
    /// so only the selected item target is emitted. 5.2 replaces the
    /// selection-only reads with the marked-in-display-order reads.
    fn selected_item_target(&self) -> Option<HomeItemTarget> {
        match self.tree.selected_target()? {
            HomeTreeTarget::Item(target) => Some(target.clone()),
            HomeTreeTarget::Root(_) | HomeTreeTarget::Age(_) => None,
        }
    }

    /// The item targets a resolved activate or context intent carries:
    /// an `Item` resolves to its section-scoped target; a `Root`/`Age`
    /// target stays local to the owner (no request crosses the boundary).
    fn item_targets(intent: Option<TreeExternalIntent<HomeTreeTarget>>) -> Vec<HomeItemTarget> {
        let Some(intent) = intent else {
            return Vec::new();
        };
        match intent {
            TreeExternalIntent::Activate(HomeTreeTarget::Item(target))
            | TreeExternalIntent::Context(HomeTreeTarget::Item(target)) => {
                vec![target]
            }
            TreeExternalIntent::ContextSelection(targets) => targets
                .into_iter()
                .filter_map(|target| match target {
                    HomeTreeTarget::Item(item_target) => Some(item_target),
                    _ => None,
                })
                .collect(),
            TreeExternalIntent::Activate(_) | TreeExternalIntent::Context(_) => Vec::new(),
        }
    }

    /// Home's local key interpretation, forwarded by the focused panel (the
    /// mounted `HomeComponent::handle_key` contract, unchanged — the router
    /// owns every global chord and keeps precedence). 5.2 ports the
    /// visual-mode chords (`Ctrl+A`, Esc, Space) to the tree's mark surface;
    /// they stay unhandled here.
    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        match key.code {
            Key::Up => {
                self.handle_tree_operation(TreeOperation::Move(-1));
                None
            }
            Key::Down => {
                self.handle_tree_operation(TreeOperation::Move(1));
                None
            }
            Key::PageUp => {
                self.handle_tree_operation(TreeOperation::Page(-1));
                None
            }
            Key::PageDown => {
                self.handle_tree_operation(TreeOperation::Page(1));
                None
            }
            Key::Home => {
                self.handle_tree_operation(TreeOperation::First);
                None
            }
            Key::End => {
                self.handle_tree_operation(TreeOperation::Last);
                None
            }
            Key::Left => {
                self.handle_tree_operation(TreeOperation::Parent);
                None
            }
            // Right never activates or collapses (design D6): the tree's
            // expanded-root `Activate` intent is consumed and dropped.
            Key::Right => {
                self.tree.apply(TreeOperation::Right);
                None
            }
            // Ctrl+A multi-select (visual mode) is a row-5.2 port: the
            // aggregate marking surface replaces the carrier's visual mode.
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => None,
            Key::Char('.') => {
                let targets =
                    Self::item_targets(self.tree.apply(TreeOperation::Context).external_intent);
                Some(Self::context_menu(targets, None))
            }
            Key::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item_target()
                .map(|target| Msg::Shell(Box::new(ShellRequest::HomeEnqueue(target)))),
            Key::Enter => self.activate_selected_row(),
            Key::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item_target()
                .map(|target| Msg::Shell(Box::new(ShellRequest::HomeToggleWatched(target)))),
            Key::Delete => self
                .selected_item_target()
                .map(|target| Msg::Shell(Box::new(ShellRequest::HomeDelete(target)))),
            _ => None,
        }
    }

    fn activate_selected_row(&mut self) -> Option<Msg> {
        // Design D6: Enter applies `Activate`; on a `Root`/`Age` target the
        // owner resolves it locally into an expansion toggle.
        match self.tree.selected_target() {
            Some(HomeTreeTarget::Item(item_target)) => {
                let target = item_target.clone();
                self.tree.apply(TreeOperation::Activate);
                Some(Msg::Shell(Box::new(ShellRequest::HomePlay(target))))
            }
            Some(root_or_age @ (HomeTreeTarget::Root(_) | HomeTreeTarget::Age(_))) => {
                let root_or_age = root_or_age.clone();
                self.tree
                    .apply(TreeOperation::ToggleExpansionTarget(root_or_age));
                None
            }
            None => None,
        }
    }

    fn handle_tree_operation(&mut self, operation: TreeOperation<HomeTreeTarget>) {
        self.tree.apply(operation);
    }

    fn context_menu(targets: Vec<HomeItemTarget>, anchor: Option<(u16, u16)>) -> Msg {
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            ContextMenuTargets::Home(targets),
            anchor,
        )))
    }

    fn on_list_slot_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        match input {
            MediaListSurfaceInput::Wheel { at, delta } => {
                // The wheel scrolls the viewport, not the selection
                // (wheel-scrolls-viewport D4); a claimed wheel still focuses
                // Library.
                if !self.tree.claims_current_point(at) {
                    return None;
                }
                self.tree.apply(TreeOperation::Scroll(delta));
                Some(Msg::TerminalEvent(
                    mbv_ui_msg::TerminalObserverEvent::MouseClaimed,
                ))
            }
            MediaListSurfaceInput::Click(at) | MediaListSurfaceInput::RangeClick(at) => {
                let target = self.tree.resolve_current_point(at).cloned()?;
                self.tree.apply(TreeOperation::Select(target.clone()));
                match target {
                    HomeTreeTarget::Item(item_target) => {
                        Some(Msg::Shell(Box::new(ShellRequest::HomeRowClick {
                            target: item_target,
                        })))
                    }
                    _ => None,
                }
            }
            MediaListSurfaceInput::ToggleClick(at) => {
                self.tree.resolve_current_point(at)?;
                self.tree.apply(TreeOperation::PointerToggleMark(at));
                None
            }
            MediaListSurfaceInput::DoubleClick(at) => {
                let target = self.tree.resolve_current_point(at).cloned()?;
                self.tree.apply(TreeOperation::Select(target.clone()));
                match target {
                    HomeTreeTarget::Item(item_target) => {
                        Some(Msg::Shell(Box::new(ShellRequest::HomeRowActivate {
                            target: item_target,
                        })))
                    }
                    root_or_age @ (HomeTreeTarget::Root(_) | HomeTreeTarget::Age(_)) => {
                        self.tree
                            .apply(TreeOperation::ToggleExpansionTarget(root_or_age));
                        None
                    }
                }
            }
            MediaListSurfaceInput::ContextClick(at) => {
                let target = self.tree.resolve_current_point(at).cloned()?;
                self.tree.apply(TreeOperation::Select(target));
                let intent = self.tree.apply(TreeOperation::Context).external_intent;
                let targets = Self::item_targets(intent);
                Some(Self::context_menu(targets, Some((at.x, at.y))))
            }
            _ => None,
        }
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
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None if matches!(
                key.code,
                Key::Up
                    | Key::Down
                    | Key::Left
                    | Key::Right
                    | Key::Home
                    | Key::End
                    | Key::PageUp
                    | Key::PageDown
                    | Key::Enter
            ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
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

/// Task 3.2 (`home-rows-playback-palette`): the Home row projection applies
/// the now-playing title-parts mapping — the container/context part is the
/// primary, the item's own title the secondary, and title-only rows carry no
/// secondary part. The tree rows reuse the same split through
/// `home_content/tree.rs`; the interactive tests against the tree land in
/// rows 5.2 and 5.3 (the carrier tests they replaced exercised the flat
/// list's own selection contract, which the shared tree owns its own tests
/// for in `mbv-components::list::tree_browser`).
#[cfg(test)]
mod tests {
    use super::*;
    use mbv_emby_model::test_support::make_item;

    fn owner_with_items(items: Vec<QueueItem>) -> HomeContent {
        let mut owner = HomeContent::new();
        owner.set_content(items, false);
        owner
    }

    #[test]
    fn key_intents_use_selected_home_item_and_ctrl_enter_enqueues() {
        let item = QueueItem::Emby(Box::new(make_item("Film", "Movie")));
        let mut owner = owner_with_items(vec![item]);
        // Launch restoration (design D8) lands the selection on the first
        // item row, so the interactive keys read the same target the
        // restored run shows.
        owner.select_first_item_target();
        let key = |code, modifiers| KeyEvent { code, modifiers };

        assert!(matches!(
            owner.handle_key(&key(Key::Enter, KeyModifiers::NONE)),
            Some(Msg::Shell(request)) if matches!(request.as_ref(), ShellRequest::HomePlay(_))
        ));
        assert!(matches!(
            owner.handle_key(&key(Key::Enter, KeyModifiers::CONTROL)),
            Some(Msg::Shell(request)) if matches!(request.as_ref(), ShellRequest::HomeEnqueue(_))
        ));
    }
}
