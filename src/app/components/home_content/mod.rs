//! The Home destination's embedded content owner (task 5.11, design D2/D3).
//! Home is the first migrated owner: a plain type — never mounted, focused,
//! subscribed, or given a `ComponentId` — that keeps its Service content, the
//! section (pill) selection, and the one shared canonical `MediaList` owner of
//! the active section's rows. It produces the panel's
//! [`LibraryPanelContent`] per frame and translates the panel's slot events
//! and forwarded chords into the same typed `Msg`s the mounted
//! `HomeComponent` emitted (its keyboard interpretation and mouse-gesture
//! resolution moved into the panel's forwarding/slot-event boundary).
//!
//! The selected item's hero comes from the shared `hero_content` producer for
//! its content type (Emby, ABS or Feeds), and its image state is the shell
//! projection's (task 5.10): this owner never fetches.

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::library_panel::content::{HeroContent, HeroImageState, LibraryPanelContent, ListSlot};
use super::library_panel::hero::hero_content_queue;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaListTransition, MediaSemanticState, RowIntent,
};
use crate::app::state::types::context_menu::ContextMenuTargets;

use super::msg::{LeafKeyResult, Msg, ShellRequest};
use mbv_core::config::{LibraryItemIdentity, SelectorIdentity};
use mbv_core::playback_queue::QueueItem;

mod launch_state;

/// The embedded content owner for the Home destination (design D2). Plain
/// type; the mounted `LibraryPanel` borrows it for content and slot events.
pub(in crate::app) struct HomeContent {
    continue_items: Vec<QueueItem>,
    /// The shared canonical owner of Continue Watching rows.
    carrier: MediaListCarrier<String>,
    loading: bool,
    /// The projection's image state for the current hero (task 5.10): set by
    /// the shell, read by the painters through the panel content.
    hero_image: HeroImageState,
}

impl HomeContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            carrier: MediaListCarrier::new(),
            loading: false,
            hero_image: HeroImageState::None,
        }
    }

    /// Replace the shell-owned Continue Watching snapshot. Refresh preserves
    /// the selected target when it remains in the content.
    pub(in crate::app) fn set_content(&mut self, continue_items: Vec<QueueItem>, loading: bool) {
        self.continue_items = continue_items;
        self.loading = loading;
        self.project_continue_rows();
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier.cursor()
    }

    /// The flat cursor's `QueueItem`, the hero's item.
    fn current_item(&self) -> Option<QueueItem> {
        self.continue_items.get(self.cursor()).cloned()
    }

    /// The hero item's content from the shared producer for its content type
    /// (design D5): Emby, ABS and Feeds items all flow through the one
    /// producer, so Home's row and its source tab render one set of facts.
    fn hero_content_data(&self) -> Option<HeroContentData> {
        self.current_item().as_ref().map(hero_content_queue)
    }

    /// Project Continue Watching items as canonical `Item` rows.
    fn project_continue_rows(&mut self) {
        let items = &self.continue_items;
        let mut rows: Vec<MediaListRow<String>> = items
            .iter()
            .map(|item| {
                let parts = item.playback_title_parts(None);
                let (primary, secondary) = match parts.context {
                    Some(context) => (context.text, Some(parts.title.text)),
                    None => (parts.title.text, None),
                };
                MediaListRow::Item {
                    primary,
                    secondary,
                    // Stable item identity preserves selection across refreshes.
                    target: item.id().to_owned(),
                    trailing: None,
                    // Library lists carry no time column (only the Queue list
                    // and the sessions modal show one).
                    duration: None,
                    kind: MediaKind::Media,
                    semantic_state: MediaSemanticState::from_queue_item(item),
                }
            })
            .collect();
        // One group header over the flat Continue list; non-selectable, so it
        // leaves the carrier's selectable-index cursor mapping untouched.
        if !rows.is_empty() {
            rows.insert(
                0,
                MediaListRow::Heading {
                    text: "Continue watching".into(),
                },
            );
        }
        self.carrier.set_content(rows);
    }

    /// The one seam through which Home offers an already-normalized row-local
    /// key or pointer gesture to the shared owner carrying its active section.
    fn delegate_row_local_input(
        &mut self,
        input: MediaListSurfaceInput,
        pointer_target: Option<String>,
    ) -> MediaListTransition<String> {
        input
            .into_operation(pointer_target)
            .map_or_else(MediaListTransition::unhandled, |operation| {
                self.carrier.delegate_operation(operation)
            })
    }

    /// Home's typed request target for a stable item identity the shared
    /// owner resolved, or the owner's current selection for a local effect.
    fn home_row_target(&self, item_id: Option<String>) -> super::msg::HomeRowTarget {
        super::msg::HomeRowTarget {
            item_id,
            source: None,
            from_continue_watching: true,
        }
    }

    /// The typed effect target for Home's current selection (the shared
    /// owner's stable target, never a cursor-minus-section-index lookup).
    fn row_target(&self) -> super::msg::HomeRowTarget {
        self.home_row_target(self.carrier.selected_target().cloned())
    }

    /// Home's local key interpretation, forwarded by the focused panel (the
    /// mounted `HomeComponent::handle_key` contract, unchanged — the router
    /// owns every global chord and keeps precedence).
    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        match key.code {
            Key::Up => {
                self.delegate_row_local_input(MediaListSurfaceInput::Move(-1), None);
                None
            }
            Key::Down => {
                self.delegate_row_local_input(MediaListSurfaceInput::Move(1), None);
                None
            }
            Key::PageUp => {
                self.delegate_row_local_input(MediaListSurfaceInput::Page(-1), None);
                None
            }
            Key::PageDown => {
                self.delegate_row_local_input(MediaListSurfaceInput::Page(1), None);
                None
            }
            Key::Home => {
                self.delegate_row_local_input(MediaListSurfaceInput::First, None);
                None
            }
            Key::End => {
                self.delegate_row_local_input(MediaListSurfaceInput::Last, None);
                None
            }
            Key::Char('.') => {
                let targets = match self
                    .delegate_row_local_input(MediaListSurfaceInput::Context, None)
                    .external_intent
                {
                    Some(RowIntent::Context(target)) => {
                        vec![self.home_row_target(Some(target))]
                    }
                    Some(RowIntent::ContextSelection(targets)) => targets
                        .into_iter()
                        .map(|target| self.home_row_target(Some(target)))
                        .collect(),
                    _ => vec![self.row_target()],
                };
                Some(Msg::Shell(ShellRequest::RowContextMenu(
                    ContextMenuTargets::Home(targets),
                    None,
                )))
            }
            Key::Enter if ctrl => Some(Msg::Shell(ShellRequest::HomeEnqueue(self.row_target()))),
            Key::Enter => match self
                .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
                .external_intent
            {
                Some(RowIntent::Activate(target)) => Some(Msg::Shell(ShellRequest::HomePlay(
                    self.home_row_target(Some(target)),
                ))),
                _ => None,
            },
            Key::Char('a') if ctrl => {
                Some(Msg::Shell(ShellRequest::HomeEnqueue(self.row_target())))
            }
            Key::Char('w') if ctrl => Some(Msg::Shell(ShellRequest::HomeToggleWatched(
                self.row_target(),
            ))),
            Key::Delete => Some(Msg::Shell(ShellRequest::HomeDelete(self.row_target()))),
            _ => None,
        }
    }

    /// The active carrier's resting scroll offset.
    #[cfg(test)]
    pub(in crate::app) fn test_active_scroll(&self) -> usize {
        self.carrier.scroll()
    }

    #[cfg(test)]
    pub(in crate::app) fn test_multi_selection_len(&self) -> usize {
        self.carrier.multi_selection().len()
    }

    /// Continue Watching rows for projection tests.
    #[cfg(test)]
    pub(in crate::app) fn test_active_rows(&self) -> &[MediaListRow<String>] {
        self.carrier.rows()
    }
}

impl LibraryContentOwner for HomeContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
    }

    fn set_selection_origin(
        &mut self,
        origin: crate::app::components::media_list::SelectionOrigin,
    ) {
        self.carrier.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
        Some(self.carrier.selection_summary())
    }

    /// This frame's Home panel content. An empty list renders its placeholder.
    fn content(&mut self) -> LibraryPanelContent<'_> {
        // Hero first: it only reads shell-owned snapshots, while the list
        // slot borrows the shared carrier mutably for the rest of the frame.
        let hero = self.hero_content_data().map(|data| {
            let mut facts = data.facts;
            facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts,
                overview: data.overview,
                credits: data.credits,
                workspace: None,
            }
        });
        let selector = None;
        let empty = self.continue_items.is_empty();
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
            LibrarySlotEvent::SelectorPicked(_) => None,
            LibrarySlotEvent::List(input) => {
                let at = match input {
                    MediaListSurfaceInput::Click(at)
                    | MediaListSurfaceInput::ToggleClick(at)
                    | MediaListSurfaceInput::RangeClick(at)
                    | MediaListSurfaceInput::DoubleClick(at)
                    | MediaListSurfaceInput::ContextClick(at) => Some(at),
                    _ => None,
                };
                // Pointer target resolution and local selection: the same
                // `claim_row` contract (a blank/gap click leaves the
                // selection unchanged).
                let target = at.and_then(|at| self.carrier.resolve_current_point(at).cloned());
                if matches!(
                    input,
                    MediaListSurfaceInput::Click(_)
                        | MediaListSurfaceInput::ToggleClick(_)
                        | MediaListSurfaceInput::RangeClick(_)
                ) {
                    self.carrier
                        .delegate_operation(input.into_operation(target.clone())?);
                    let _ = ();
                }
                match input {
                    MediaListSurfaceInput::Wheel { .. } => {
                        self.delegate_row_local_input(input, None);
                        Some(Msg::TerminalEvent(
                            super::msg::TerminalObserverEvent::MouseClaimed,
                        ))
                    }
                    MediaListSurfaceInput::DoubleClick(_) => {
                        self.carrier
                            .delegate_operation(MediaListOperation::Activate(target?));
                        Some(Msg::Shell(ShellRequest::HomeRowActivate {
                            target: self.row_target(),
                        }))
                    }
                    MediaListSurfaceInput::ContextClick(at) => {
                        let outcome = {
                            let target = target?;
                            self.carrier
                                .delegate_operation(MediaListOperation::Context(target))
                        };
                        let _ = ();
                        let targets = match outcome.external_intent {
                            Some(RowIntent::Context(target)) => {
                                vec![self.home_row_target(Some(target))]
                            }
                            Some(RowIntent::ContextSelection(targets)) => targets
                                .into_iter()
                                .map(|target| self.home_row_target(Some(target)))
                                .collect(),
                            _ => vec![self.row_target()],
                        };
                        Some(Msg::Shell(ShellRequest::RowContextMenu(
                            ContextMenuTargets::Home(targets),
                            Some((at.x, at.y)),
                        )))
                    }
                    MediaListSurfaceInput::Click(_)
                    | MediaListSurfaceInput::ToggleClick(_)
                    | MediaListSurfaceInput::RangeClick(_) => {
                        Some(Msg::Shell(ShellRequest::HomeRowClick {
                            target: self.row_target(),
                        }))
                    }
                    _ => None,
                }
            }
            // Home has no Workspace and no hero-pane input of its own.
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => match self
                .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
                .external_intent
            {
                Some(RowIntent::Activate(target)) => Some(Msg::Shell(ShellRequest::HomePlay(
                    self.home_row_target(Some(target)),
                ))),
                _ => None,
            },
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
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
    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
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

/// Task 3.2 (home-rows-playback-palette): the Home row projection applies
/// the now-playing title-parts mapping — the container/context part is the
/// primary, the item's own title the secondary, and title-only rows carry no
/// secondary part. The core media-type mapping itself is pinned in
/// `playback_queue_tests_title_parts`; these tests pin the Home wiring
/// (including the shell-resolved feed-name lookup by `feed_id`).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;
    use mbv_core::playback_queue::QueueItem;

    fn owner_with_items(items: Vec<QueueItem>) -> HomeContent {
        let mut owner = HomeContent::new();
        owner.set_content(items, false);
        owner
    }

    fn row_parts(owner: &HomeContent, index: usize) -> (String, Option<String>) {
        // Item rows only: the projection's leading group heading is
        // non-selectable and outside these assertions' concern.
        let row = owner
            .test_active_rows()
            .iter()
            .filter(|r| matches!(r, MediaListRow::Item { .. }))
            .nth(index)
            .unwrap_or_else(|| panic!("expected item row {index}, got none"));
        match row {
            MediaListRow::Item {
                primary, secondary, ..
            } => (primary.clone(), secondary.clone()),
            other => panic!("expected an item row, got {other:?}"),
        }
    }

    #[test]
    fn the_continue_list_opens_with_a_keep_watching_group_heading() {
        let owner = owner_with_items(vec![QueueItem::Emby(Box::new(make_item("Film", "Movie")))]);
        let rows = owner.test_active_rows();
        assert_eq!(rows.len(), 2);
        assert!(
            matches!(&rows[0], MediaListRow::Heading { text } if text == "Continue watching"),
            "the first row is the Continue watching group heading: {:?}",
            rows[0]
        );
    }

    #[test]
    fn an_empty_continue_list_paints_no_group_heading() {
        let owner = owner_with_items(vec![]);
        assert!(owner.test_active_rows().is_empty());
    }

    #[test]
    fn episode_rows_project_the_series_as_context_and_the_episode_title() {
        let mut episode = make_item("Pilot", "Episode");
        episode.id = "ep1".into();
        episode.series_name = "Series Name".into();
        let owner = owner_with_items(vec![QueueItem::Emby(Box::new(episode))]);
        assert_eq!(
            row_parts(&owner, 0),
            ("Series Name".into(), Some("Pilot".into()))
        );
    }

    /// Home rows use the one canonical state derivation: a finished item is
    /// `Played`, an in-progress item is `Active` with its resume percentage,
    /// and a music row is always `Ordinary`.

    #[test]
    fn home_rows_use_the_canonical_state_derivation() {
        let mut played = make_item("Finished Film", "Movie");
        played.id = "played".into();
        played.played = true;
        let mut in_progress = make_item("Half-Watched Film", "Movie");
        in_progress.id = "in-progress".into();
        in_progress.runtime_ticks = 1000;
        in_progress.playback_position_ticks = 500;
        let mut track = make_item("Album Track", "Audio");
        track.id = "track".into();
        track.played = true;
        track.runtime_ticks = 1000;
        track.playback_position_ticks = 500;
        let owner = owner_with_items(vec![
            QueueItem::Emby(Box::new(played)),
            QueueItem::Emby(Box::new(in_progress)),
            QueueItem::Emby(Box::new(track)),
        ]);
        let states: Vec<MediaSemanticState> = owner
            .test_active_rows()
            .iter()
            .filter_map(|row| match row {
                MediaListRow::Item { semantic_state, .. } => Some(semantic_state.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(states[0], MediaSemanticState::Played);
        assert_eq!(states[1], MediaSemanticState::active(Some(50)));
        assert_eq!(states[2], MediaSemanticState::Ordinary);
    }

    /// Truncation is the canonical painter's contract (a split row is cut
    /// as one string, context first); the projection's side of it is to hand
    /// over both parts untruncated so the painter can decide.

    #[test]
    fn split_rows_carry_their_full_parts_for_the_painters_truncation_priority() {
        let mut episode = make_item("A Very Long Episode Title That Must Survive", "Episode");
        episode.id = "ep1".into();
        episode.series_name = "A Very Long Series Name That May Ellipsise First".into();
        let owner = owner_with_items(vec![QueueItem::Emby(Box::new(episode))]);
        assert_eq!(
            row_parts(&owner, 0),
            (
                "A Very Long Series Name That May Ellipsise First".into(),
                Some("A Very Long Episode Title That Must Survive".into())
            )
        );
    }
}
