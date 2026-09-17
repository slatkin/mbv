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

use std::collections::HashMap;

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow,
};
use super::library_panel::hero::hero_content_queue;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaListTrailing, MediaListTransition, MediaSemanticState, RowIntent,
};
use crate::app::types_context_menu::ContextMenuTargets;

use super::msg::{LeafKeyResult, Msg, ShellRequest};
use crate::app::types_playback::HomeLatestSource;
use crate::app::ui_util::trunc_str;
use mbv_core::playback_queue::QueueItem;

/// The resume-percentage badge Home rows draw next to the title (rendered as
/// canonical `trailing`). Only for in-progress, unfinished items with a
/// non-zero rounded percentage.
fn home_progress_badge(item: &QueueItem) -> Option<String> {
    let (position, runtime) = (item.playback_position_ticks(), item.runtime_ticks());
    (position > 0 && !item.played() && runtime > 0)
        .then(|| (position as i128 * 100 / runtime as i128) as u16)
        .filter(|pct| *pct > 0)
        .map(|pct| format!("{pct}%"))
}

/// The embedded content owner for the Home destination (design D2). Plain
/// type; the mounted `LibraryPanel` borrows it for content and slot events.
pub(in crate::app) struct HomeContent {
    continue_items: Vec<QueueItem>,
    latest: Vec<(String, HomeLatestSource, Vec<QueueItem>)>,
    /// The one shared canonical owner of the active section's rows; the panel
    /// drives its Wide/Inline presentation from its own breakpoint.
    carrier: MediaListCarrier<String>,
    loading: bool,
    /// Shell-resolved feed-id → display-name lookup (design D2): `Config`
    /// never enters components, so the shell resolves at assignment and the
    /// projection reads entries up by `feed_id`.
    feed_names: HashMap<String, String>,
    section: usize,
    /// The projection's image state for the current hero (task 5.10): set by
    /// the shell, read by the painters through the panel content.
    hero_image: HeroImageState,
}

impl HomeContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            latest: Vec::new(),
            carrier: MediaListCarrier::new(),
            loading: false,
            feed_names: HashMap::new(),
            section: 0,
            hero_image: HeroImageState::None,
        }
    }

    /// Replace the shell-owned content snapshot (task 5.3d's projection
    /// shape, now addressed by `LibraryKey`): section/cursor clamp to the new
    /// content; an ordinary refresh preserves the selected target through
    /// `MediaList::set_content` and locally clamps.
    pub(in crate::app) fn set_content(
        &mut self,
        continue_items: Vec<QueueItem>,
        latest: Vec<(String, HomeLatestSource, Vec<QueueItem>)>,
        loading: bool,
        feed_names: HashMap<String, String>,
    ) {
        self.continue_items = continue_items;
        self.latest = latest;
        self.loading = loading;
        self.feed_names = feed_names;
        self.clamp_section();
        self.project_active_section();
    }

    /// The flat cursor (Continue Watching + every latest section) the shell's
    /// `home_stable_target` resolves. Derived from the shared owner's
    /// selectable index over the active section's rows; no cursor mirror.
    pub(in crate::app) fn cursor(&self) -> usize {
        let index = self.carrier.cursor();
        self.visible_indices().get(index).copied().unwrap_or(0)
    }

    pub(in crate::app) fn section(&self) -> usize {
        self.section
    }

    /// The semantic `HomeLatestSource` of a numeric section index: `None` for
    /// Continue Watching (section 0, the empty-string persistence sentinel),
    /// otherwise the selected latest section's source. Resolving by section
    /// here keeps the off-by-one rule in the owner; the shell persists this
    /// identity, never the index (task 5.3d).
    pub(in crate::app) fn source_for_section(&self, section: usize) -> Option<HomeLatestSource> {
        if section == 0 {
            return None;
        }
        self.latest
            .get(section - 1)
            .map(|(_, source, _)| source.clone())
    }

    /// Restore a persisted pill selection once a section matching `source`
    /// exists (the shell applies `home_section_pending` on
    /// `push_home_content`). Returns `true` once restored (the shell clears
    /// the pending marker afterward).
    pub(in crate::app) fn restore_section(&mut self, source: &HomeLatestSource) -> bool {
        if let Some(idx) = self.latest.iter().position(|(_, s, _)| s == source) {
            self.section = idx + 1;
            self.clamp_section();
            self.project_active_section();
            self.delegate_row_local_input(MediaListSurfaceInput::First, None);
            true
        } else {
            false
        }
    }

    /// The flat cursor's `QueueItem`, the hero's item.
    fn current_item(&self) -> Option<QueueItem> {
        self.continue_items
            .iter()
            .chain(self.latest.iter().flat_map(|(_, _, i)| i.iter()))
            .nth(self.cursor())
            .cloned()
    }

    /// The hero item's content from the shared producer for its content type
    /// (design D5): Emby, ABS and Feeds items all flow through the one
    /// producer, so Home's row and its source tab render one set of facts.
    fn hero_content_data(&self) -> Option<HeroContentData> {
        self.current_item().as_ref().map(hero_content_queue)
    }

    // ── Section state (numeric section owned here, task 5.3d) ────────────

    fn new_sections(&self) -> Vec<usize> {
        (0..self.latest.len()).map(|idx| idx + 1).collect()
    }

    fn section_is_valid(&self, section_idx: usize) -> bool {
        section_idx == 0 || self.new_sections().contains(&section_idx)
    }

    fn section_range(&self, section_idx: usize) -> Option<(usize, usize)> {
        if section_idx == 0 {
            return Some((0, self.continue_items.len()));
        }
        let mut pos = self.continue_items.len();
        for (idx, (_, _, items)) in self.latest.iter().enumerate() {
            if idx + 1 == section_idx {
                return Some((pos, items.len()));
            }
            pos += items.len();
        }
        None
    }

    fn visible_indices(&self) -> Vec<usize> {
        let selected = if self.section_is_valid(self.section) {
            self.section
        } else {
            self.new_sections().first().copied().unwrap_or(0)
        };
        self.section_range(selected)
            .map(|(start, len)| (start..start + len).collect())
            .unwrap_or_default()
    }

    fn clamp_section(&mut self) {
        if !self.section_is_valid(self.section) {
            self.section = self.new_sections().first().copied().unwrap_or(0);
        }
    }

    /// Project only the active Home section's items as canonical `Item` rows
    /// (Home has no `Heading`/`Spacer` vocabulary, so structural-row index
    /// equals selectable index). Feeds the shared owner; an ordinary refresh
    /// preserves the selected target through `MediaList::set_content`.
    fn project_active_section(&mut self) {
        let items = if self.section == 0 {
            &self.continue_items
        } else {
            self.latest
                .get(self.section - 1)
                .map(|(_, _, items)| items)
                .unwrap_or(&self.continue_items)
        };
        let rows: Vec<MediaListRow<String>> = items
            .iter()
            .map(|item| {
                // Split rows share the now-playing mapping (design D2/D4):
                // primary is the container/context, secondary the item's own
                // name; feed names come from the shell-resolved `feed_id`
                // lookup (`Config` never enters components).
                let feed_name = item
                    .as_feed()
                    .and_then(|entry| entry.feed_id.as_deref())
                    .and_then(|feed_id| self.feed_names.get(feed_id))
                    .map(String::as_str);
                let parts = item.playback_title_parts(feed_name);
                let (primary, secondary) = match parts.context {
                    Some(context) => (context.text, Some(parts.title.text)),
                    None => (parts.title.text, None),
                };
                MediaListRow::Item {
                    primary,
                    secondary,
                    // Stable per-item identity (Emby id / feed guid / ABS
                    // episode id) — the same id the queue/shell treat as
                    // canonical — so an ordinary refresh retains the selection
                    // by identity, not by a title that can collide across
                    // episodes.
                    target: item.id().to_owned(),
                    trailing: home_progress_badge(item).map(MediaListTrailing::Progress),
                    // Library lists carry no time column (only the Queue list
                    // and the sessions modal show one).
                    duration: None,
                    kind: MediaKind::Media,
                    semantic_state: MediaSemanticState::Ordinary,
                }
            })
            .collect();
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
            source: self
                .latest
                .get(self.section.saturating_sub(1))
                .map(|(_, source, _)| source.pref_key()),
            from_continue_watching: self.section == 0,
        }
    }

    /// The typed effect target for Home's current selection (the shared
    /// owner's stable target, never a cursor-minus-section-index lookup).
    fn row_target(&self) -> super::msg::HomeRowTarget {
        self.home_row_target(self.carrier.selected_target().cloned())
    }

    /// Select `section_idx` (clamped to the nearest valid section). Returns
    /// `true` when the selection actually changed, so the caller emits the
    /// persist `Msg` only on a real change.
    fn select_section(&mut self, section_idx: usize) -> bool {
        let resolved = if self.section_is_valid(section_idx) {
            section_idx
        } else if let Some(first) = self.new_sections().first() {
            *first
        } else {
            self.section = 0;
            return false;
        };
        if resolved == self.section {
            return false;
        }
        self.section = resolved;
        // A discrete section change re-projects the active section and parks
        // the shared owner at its first row (no per-section cursor cache).
        self.project_active_section();
        self.delegate_row_local_input(MediaListSurfaceInput::First, None);
        true
    }

    fn move_section(&mut self, dir: i64) -> bool {
        let mut sections = vec![0];
        sections.extend(self.new_sections());
        let pos = sections.iter().position(|&s| s == self.section);
        let next_pos = match pos {
            Some(p) => {
                let n = sections.len() as i64;
                (((p as i64 + dir) % n + n) % n) as usize
            }
            None => 0,
        };
        self.select_section(sections[next_pos])
    }

    fn section_msg(&self, changed: bool) -> Option<Msg> {
        changed.then_some(Msg::Shell(ShellRequest::HomeSectionSelected(self.section)))
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
            Key::Char('[') if !ctrl => {
                let changed = self.move_section(-1);
                self.section_msg(changed)
            }
            Key::Char(']') if !ctrl => {
                let changed = self.move_section(1);
                self.section_msg(changed)
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
            Key::Char('.') if self.section == 0 => {
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
            Key::Char('.') => None,
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
            Key::Char('w') if ctrl && self.section == 0 => Some(Msg::Shell(
                ShellRequest::HomeToggleWatched(self.row_target()),
            )),
            Key::Char('w') if ctrl => None,
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

    /// Active section rows for projection tests (untruncated by design).
    #[cfg(test)]
    pub(in crate::app) fn test_active_rows(&self) -> &[MediaListRow<String>] {
        self.carrier.rows()
    }
}

impl LibraryContentOwner for HomeContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_selection();
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

    /// This frame's panel content (design D3): the section pills as the
    /// Selector row, the shared carrier in the list slot, and the selected
    /// item's hero from the shared producer with the projected image state.
    /// An empty section renders the list placeholder; an absent item renders
    /// no hero.
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
        let selector = if self.latest.is_empty() {
            // No latest sections: the Continue pill alone still paints the
            // one Selector row (an empty section renders as a real,
            // discoverable pill).
            Some(SelectorRow {
                pills: vec!["Continue".into()],
                active: Some(0),
            })
        } else {
            Some(SelectorRow {
                pills: std::iter::once("Continue".to_string())
                    .chain(
                        self.latest
                            .iter()
                            .map(|(title, _, _)| trunc_str(title, 18).to_string()),
                    )
                    .collect(),
                active: Some(self.section),
            })
        };
        let empty = if self.section == 0 {
            self.continue_items.is_empty()
        } else {
            self.latest
                .get(self.section - 1)
                .is_none_or(|(_, _, items)| items.is_empty())
        };
        let list = if empty {
            ListSlot::Empty {
                loading: false,
                text: " (empty)".into(),
            }
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            controls: None,
            list,
            hero,
        }
    }

    /// Translate one resolved slot event into Home's existing typed `Msg`s
    /// (design D2): pills select sections, pointer gestures resolve their
    /// typed target through the shared carrier exactly as the mounted
    /// component's own gesture path did.
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(section_idx) => {
                self.select_section(section_idx);
                Some(Msg::Shell(ShellRequest::HomePillClick {
                    target: section_idx,
                }))
            }
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
            // Home has no List-controls row, no Workspace and no hero-pane
            // input of its own.
            LibrarySlotEvent::ControlPicked(_)
            | LibrarySlotEvent::WorkspaceSelectorPicked(_)
            | LibrarySlotEvent::HeroPane(_) => None,
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
    use mbv_core::playback_queue::{
        AudiobookshelfBookQueueItem, AudiobookshelfQueueItem, FeedEntry,
    };

    const SUBSCRIPTION_URL: &str = "https://example.com/feed.xml";

    fn feed_entry(title: &str, feed_id: Option<&str>) -> QueueItem {
        QueueItem::Feed(FeedEntry {
            guid: format!("guid-{title}"),
            title: title.into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: feed_id.map(Into::into),
            position_ticks: 0,
            played: false,
        })
    }

    fn owner_with_section(
        source: HomeLatestSource,
        items: Vec<QueueItem>,
        feed_names: &[(&str, &str)],
    ) -> HomeContent {
        let mut owner = HomeContent::new();
        let names = feed_names
            .iter()
            .map(|(id, name)| (id.to_string(), name.to_string()))
            .collect();
        owner.set_content(
            Vec::new(),
            vec![("Latest".into(), source.clone(), items)],
            false,
            names,
        );
        assert!(
            owner.restore_section(&source),
            "the pushed section must exist"
        );
        owner
    }

    fn row_parts(owner: &HomeContent, index: usize) -> (String, Option<String>) {
        match &owner.test_active_rows()[index] {
            MediaListRow::Item {
                primary, secondary, ..
            } => (primary.clone(), secondary.clone()),
            other => panic!("expected an item row, got {other:?}"),
        }
    }

    #[test]
    fn episode_rows_project_the_series_as_context_and_the_episode_title() {
        let mut episode = make_item("Pilot", "Episode");
        episode.id = "ep1".into();
        episode.series_name = "Series Name".into();
        let owner = owner_with_section(
            HomeLatestSource::Emby("emby".into()),
            vec![QueueItem::Emby(Box::new(episode))],
            &[],
        );
        assert_eq!(
            row_parts(&owner, 0),
            ("Series Name".into(), Some("Pilot".into()))
        );
    }

    #[test]
    fn audio_track_rows_project_the_artist_as_context() {
        let mut track = make_item("Track Name", "Audio");
        track.id = "a1".into();
        track.media_type = "Audio".into();
        track.artist = "Artist Name".into();
        let owner = owner_with_section(
            HomeLatestSource::Emby("emby".into()),
            vec![QueueItem::Emby(Box::new(track))],
            &[],
        );
        assert_eq!(
            row_parts(&owner, 0),
            ("Artist Name".into(), Some("Track Name".into()))
        );
    }

    #[test]
    fn feed_rows_project_the_resolved_subscription_as_context() {
        let owner = owner_with_section(
            HomeLatestSource::Feeds,
            vec![feed_entry("Entry Title", Some(SUBSCRIPTION_URL))],
            &[(SUBSCRIPTION_URL, "Example Daily")],
        );
        assert_eq!(
            row_parts(&owner, 0),
            ("Example Daily".into(), Some("Entry Title".into()))
        );
    }

    #[test]
    fn feed_rows_without_a_resolved_subscription_stay_single_part() {
        let owner = owner_with_section(
            HomeLatestSource::Feeds,
            vec![feed_entry("Entry Title", Some(SUBSCRIPTION_URL))],
            &[],
        );
        assert_eq!(row_parts(&owner, 0), ("Entry Title".into(), None));
    }

    #[test]
    fn abs_podcast_rows_project_the_show_as_context() {
        let owner = owner_with_section(
            HomeLatestSource::Audiobookshelf("lib".into()),
            vec![QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
                library_item_id: "show".into(),
                episode_id: "ep1".into(),
                title: "Episode Five".into(),
                show_title: Some("Show Title".into()),
                ..abs_episode_defaults()
            })],
            &[],
        );
        assert_eq!(
            row_parts(&owner, 0),
            ("Show Title".into(), Some("Episode Five".into()))
        );
    }

    #[test]
    fn movie_and_book_rows_stay_single_part() {
        let mut movie = make_item("The Film", "Movie");
        movie.id = "m1".into();
        let book = QueueItem::AudiobookshelfBook(AudiobookshelfBookQueueItem {
            library_item_id: "book".into(),
            title: "The Book".into(),
            author: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            is_finished: false,
            cover_path: None,
        });
        let owner = owner_with_section(
            HomeLatestSource::Emby("emby".into()),
            vec![QueueItem::Emby(Box::new(movie)), book],
            &[],
        );
        assert_eq!(row_parts(&owner, 0), ("The Film".into(), None));
        assert_eq!(row_parts(&owner, 1), ("The Book".into(), None));
    }

    /// Truncation is the canonical painter's contract (a split row is cut
    /// as one string, context first); the projection's side of it is to hand
    /// over both parts untruncated so the painter can decide.
    #[test]
    fn split_rows_carry_their_full_parts_for_the_painters_truncation_priority() {
        let mut episode = make_item("A Very Long Episode Title That Must Survive", "Episode");
        episode.id = "ep1".into();
        episode.series_name = "A Very Long Series Name That May Ellipsise First".into();
        let owner = owner_with_section(
            HomeLatestSource::Emby("emby".into()),
            vec![QueueItem::Emby(Box::new(episode))],
            &[],
        );
        assert_eq!(
            row_parts(&owner, 0),
            (
                "A Very Long Series Name That May Ellipsise First".into(),
                Some("A Very Long Episode Title That Must Survive".into())
            )
        );
    }

    fn abs_episode_defaults() -> AudiobookshelfQueueItem {
        AudiobookshelfQueueItem {
            library_item_id: String::new(),
            episode_id: String::new(),
            title: String::new(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        }
    }
}
