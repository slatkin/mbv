//! The Movies/HomeVideos/Generic Emby destinations' embedded content owner
//! (task 6.1, design D2/D3). This owner serves Generic, Movies, and HomeVideos;
//! TV and Music have their own owners. A plain type — never mounted, focused,
//! subscribed, or given a `ComponentId` — that keeps the shell-projected
//! browse rows, the letter/feed-group pill state, the one shared canonical
//! `MediaList` owner of the active level's rows, and the embedded Inline
//! Search session. It produces the panel's [`LibraryPanelContent`] per frame
//! and translates the panel's slot events and forwarded chords into the same
//! typed `Msg`s the former BrowserComponent emitted for these three kinds
//! (`shell_emby_library.rs::handle_emby_library_request` and `shell_messages.rs`'s
//! `Browser*`/`EmbyLibrary*` dispatch are unchanged and keyed only by the
//! active tab, so they apply unmodified to messages this owner emits).
//!
//! TV's browsing uses its own owner; nothing here is shared state with it.
//!
//! The selected item's hero comes from the shared `hero_content_emby`
//! producer (design D5) and is rendered by the Library Hero overlay — this
//! owner never builds a banner layout or fetches an image itself (task 5.10's shell
//! projection does that, generically, for every migrated owner).

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use mbv_core::api::EmbyItem;

use super::inline_search::{InlineSearch, InlineSearchAction, InlineSearchHost};
use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListControls, ListSlot, SelectorRow,
};
use super::library_panel::hero::hero_content_emby;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::library_panel::LibraryKind;
use super::media_list::{
    letter_grouped_rows, MediaKind, MediaListCarrier, MediaListOperation, MediaListRow,
    MediaListSurfaceInput, MediaListTrailing, MediaSemanticState, RowIntent,
};
use super::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::render::{effective_sort_str, LetterFilter};

/// Browse identity used to decide when a projected position should be applied.
#[derive(Clone, Default, PartialEq, Eq)]
pub(in crate::app) struct EmbyLibraryIdentity {
    pub(in crate::app) depth: usize,
    pub(in crate::app) parent_id: String,
    pub(in crate::app) letter_filter: Option<usize>,
    pub(in crate::app) sort_by: String,
    pub(in crate::app) sort_order: String,
    pub(in crate::app) unplayed_only: bool,
    pub(in crate::app) feed_group: Option<usize>,
}

/// Derives the Emby-specific semantic state for a browse row (mirrors
/// the prior Emby semantic-state helper; the provider-neutral `media_list` layer
/// deliberately stays free of `EmbyItem`, so both projection sites — this
/// owner and TV's `TvContent` — carry their own copy).
fn emby_semantic_state(item: &EmbyItem) -> MediaSemanticState {
    if item.playback_position_ticks > 0 && !item.played {
        let progress = if item.runtime_ticks > 0 {
            Some(
                ((item.playback_position_ticks as u64 * 100) / item.runtime_ticks as u64).min(100)
                    as u16,
            )
        } else {
            None
        };
        MediaSemanticState::active(progress)
    } else if item.played {
        MediaSemanticState::Played
    } else {
        MediaSemanticState::Ordinary
    }
}

fn row_for(item: &EmbyItem) -> MediaListRow<String> {
    let primary = if item.is_folder && item.item_type == "Folder" && item.total_count > 0 {
        format!("{} \u{b7} {} items", item.display_name(), item.total_count)
    } else if item.is_folder && item.unplayed_item_count > 0 && item.item_type != "Series" {
        format!("{} [{}]", item.display_name(), item.unplayed_item_count)
    } else {
        item.display_name()
    };
    MediaListRow::Item {
        target: item.id.clone(),
        primary,
        secondary: None,
        trailing: (!item.is_folder && item.production_year > 0)
            .then(|| MediaListTrailing::Year(item.production_year.to_string())),
        duration: None,
        kind: MediaKind::Collection,
        semantic_state: emby_semantic_state(item),
    }
}

/// One shell content push (mirrors the Emby library owner's content push, plus the
/// letter/feed-group pill and home-video-count facts the old
/// `NarrowBrowseExtras`/wide-Movies pill row carried separately; task 6.1
/// unifies them into the Selector row and List controls row, design D8).
pub(in crate::app) struct BrowserOwnerPush {
    pub items: Vec<EmbyItem>,
    pub total_count: usize,
    pub library_total: Option<usize>,
    pub letter_filter: Option<LetterFilter>,
    pub loading: bool,
    /// Whether `items` is a feed/home-video group's selected videos
    /// (`is_feed_home_video_group_view`): the Selector row shows feed-group
    /// pills instead of letter pills, and `[`/`]` cycles groups.
    pub group_pills: bool,
    /// Whether the List controls row shows the home-video item-count label.
    pub home_video: bool,
    pub show_letter_pills: bool,
    pub feed_groups: Vec<String>,
    pub feed_group_cursor: usize,
}

/// The embedded content owner for Movies, HomeVideos and Generic Emby
/// libraries (design D2, task 6.1). Plain type; the mounted `LibraryPanel`
/// borrows it for content and slot events.
pub(in crate::app) struct EmbyLibraryContent {
    kind: LibraryKind,
    items: Vec<EmbyItem>,
    total_count: usize,
    library_total: Option<usize>,
    letter_filter: Option<LetterFilter>,
    loading: bool,
    group_pills: bool,
    home_video: bool,
    show_letter_pills: bool,
    feed_groups: Vec<String>,
    feed_group_cursor: usize,
    /// The one shared canonical owner of the active level's rows; the panel
    /// drives its Wide/Inline presentation from its own breakpoint (design
    /// D3/D4) — this owner never chooses a presentation itself.
    carrier: MediaListCarrier<String>,
    last_identity: Option<EmbyLibraryIdentity>,
    /// The rows the last `feed_owner` projection produced: identical
    /// projections skip `carrier.set_content`, so an ordinary no-op sync
    /// never invalidates the presentation's painted frame (design D6 — a
    /// configured-but-unpainted owner claims nothing; the Home owner avoids
    /// this by pushing at writer seams only, this owner re-projects per
    /// sync, so the skip must live here).
    last_projected_rows: Option<Vec<MediaListRow<String>>>,
    /// The projection's image state for the current hero (task 5.10): set by
    /// the shell, read by the painters through the panel content.
    hero_image: HeroImageState,
    hero_scroll: usize,
    /// The embedded Inline Search control (design D3): this owner is its
    /// sole event boundary; the panel places its box in the Selector row's
    /// rect and its results in the list box (`ListSlot::Search`).
    inline_search: InlineSearch,
}

impl EmbyLibraryContent {
    pub(in crate::app) fn new(kind: LibraryKind) -> Self {
        Self {
            kind,
            items: Vec::new(),
            total_count: 0,
            library_total: None,
            letter_filter: None,
            loading: false,
            group_pills: false,
            home_video: false,
            show_letter_pills: false,
            feed_groups: Vec::new(),
            feed_group_cursor: 0,
            carrier: MediaListCarrier::new(),
            last_identity: None,
            last_projected_rows: None,
            hero_image: HeroImageState::None,
            hero_scroll: 0,
            inline_search: InlineSearch::new(),
        }
    }

    /// Replace the shell-owned position-free content snapshot: the shared
    /// owner's `set_content` preserves the selected target and locally
    /// clamps otherwise (design D3); position re-seeds only through the
    /// separately identity-gated `apply_position`.
    pub(in crate::app) fn set_content(&mut self, push: BrowserOwnerPush) {
        self.items = push.items;
        self.total_count = push.total_count;
        self.library_total = push.library_total;
        self.letter_filter = push.letter_filter;
        self.loading = push.loading;
        self.group_pills = push.group_pills;
        self.home_video = push.home_video;
        self.show_letter_pills = push.show_letter_pills;
        self.feed_groups = push.feed_groups;
        self.feed_group_cursor = push.feed_group_cursor;
        self.feed_owner();
    }

    /// Explicit, identity-gated resting-position re-seed (mirrors
    /// the former BrowserComponent's position application): the shell calls this only when
    /// `note_browse_identity` reports a real identity change (drill-in,
    /// go-back, letter-filter reset, sort change, feed/home-video group
    /// switch). Within one identity no position crosses the boundary.
    pub(in crate::app) fn apply_position(&mut self, cursor: usize, scroll: usize) {
        let target = self
            .items
            .get(cursor.min(self.items.len().saturating_sub(1)))
            .map(|item| item.id.clone());
        if let Some(target) = target.as_ref() {
            self.carrier.select_target(target);
        }
        self.carrier.set_scroll(scroll);
    }

    /// Records the browse identity of the current shell content push and
    /// reports whether it differs from the previous push (mirrors
    /// the former BrowserComponent's identity tracking).
    pub(in crate::app) fn note_browse_identity(&mut self, identity: EmbyLibraryIdentity) -> bool {
        let changed = self.last_identity.as_ref() != Some(&identity);
        self.last_identity = Some(identity);
        if changed {
            self.hero_scroll = 0;
        }
        changed
    }

    /// The authoritative selection of the shared owner, as an `items` index
    /// (the owner is the only cursor store).
    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier
            .selected_target()
            .and_then(|target| self.items.iter().position(|item| item.id == *target))
            .unwrap_or(0)
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier.scroll()
    }

    fn true_total(&self) -> usize {
        self.library_total.unwrap_or(self.total_count)
    }

    /// Project the mirrored items into provider-neutral rows: letter-grouped
    /// `Heading`/`Spacer`/`Item` rows for a large library (or an active
    /// letter pill), natural-sorted plain rows otherwise (mirrors
    /// the former BrowserComponent's row projection).
    fn feed_owner(&mut self) {
        let grouped = self.true_total() >= 50 || self.letter_filter.is_some();
        let rows: Vec<MediaListRow<String>> = if grouped {
            let pairs: Vec<(String, MediaListRow<String>)> = self
                .items
                .iter()
                .map(|item| (effective_sort_str(item).to_string(), row_for(item)))
                .collect();
            letter_grouped_rows(pairs, self.true_total(), self.letter_filter.is_some())
        } else {
            self.items.iter().map(row_for).collect()
        };
        // Ordinary refresh (design D3): an unchanged projection preserves
        // the shared owner's painted frame instead of re-issuing it.
        if self.last_projected_rows.as_ref() != Some(&rows) {
            self.carrier.set_content(rows.clone());
            self.last_projected_rows = Some(rows);
        }
    }

    /// The selected item, gated the same way the old wide Movies hero was
    /// (no hero for a folder; Movies libraries additionally require the
    /// selected item to actually be a `Movie`, not e.g. a BoxSet folder).
    fn hero_item(&self) -> Option<&EmbyItem> {
        let item = self.items.get(self.cursor())?;
        (!item.is_folder && (self.kind != LibraryKind::Movies || item.item_type == "Movie"))
            .then_some(item)
    }

    fn selected_effect_item(&self) -> Option<EmbyItem> {
        self.items.get(self.cursor()).cloned()
    }

    /// Test-only cursor seed, mirroring the embedded owner's test cursor seed:
    /// tests position the authoritative owner selection directly before
    /// exercising navigation.
    #[cfg(test)]
    pub(in crate::app) fn set_cursor_for_test(&mut self, cursor: usize) {
        if let Some(item) = self.items.get(cursor) {
            let target = item.id.clone();
            self.carrier.select_target(&target);
        }
    }

    /// Ctrl+P/S/A on the selected Inline Search result (mirrors
    /// the former BrowserComponent's inline-search result action): reuses the ordinary
    /// result-row shell effects, resolved against the search cursor rather
    /// than the ordinary browse cursor.
    fn inline_search_result_action(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        let item = self.inline_search.selected_item()?;
        let request = match key.code {
            Key::Char('p') => ShellRequest::EmbyLibraryPlay { item },
            Key::Char('s') => ShellRequest::EmbyLibraryShuffle { item },
            Key::Char('a') => ShellRequest::EmbyLibraryEnqueue { item },
            _ => return None,
        };
        self.inline_search.close();
        Some(Msg::Shell(request))
    }

    /// Pointer input against the active Inline Search session (design.md D4):
    /// the panel-normalized `MediaListSurfaceInput` delegates to the session's
    /// embedded carrier like every other list — a click selects, a double-click
    /// activates, a right-click resolves the row's ordinary item-based
    /// context-menu intent, and a wheel over the painted rows is claimed.
    fn handle_search_pointer(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        let search = &mut self.inline_search;
        match input {
            MediaListSurfaceInput::Wheel { at, delta } => {
                if !search.results_mut().claims_current_point(at) {
                    return None;
                }
                search.results_mut().delegate_operation(
                    MediaListSurfaceInput::Wheel { at, delta }
                        .into_operation(None)
                        .expect("wheel converts without a target"),
                );
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MediaListSurfaceInput::Click(at)
            | MediaListSurfaceInput::ToggleClick(at)
            | MediaListSurfaceInput::RangeClick(at) => {
                let target = search.results_mut().resolve_current_point(at)?.clone();
                search.results_mut().delegate_operation(
                    input
                        .into_operation(Some(target))
                        .expect("resolved media-list pointer target"),
                );
                None
            }
            MediaListSurfaceInput::DoubleClick(at) => {
                let target = search.results_mut().resolve_current_point(at)?.clone();
                search.results_mut().delegate_operation(
                    MediaListSurfaceInput::DoubleClick(at)
                        .into_operation(Some(target))
                        .expect("resolved media-list pointer target"),
                );
                search.selected_item().map(|item| {
                    Msg::Shell(ShellRequest::InlineSearchActivate {
                        id: item.id,
                        item_type: item.item_type,
                    })
                })
            }
            MediaListSurfaceInput::ContextClick(at) => {
                let target = search.results_mut().resolve_current_point(at)?.clone();
                let outcome = search.results_mut().delegate_operation(
                    MediaListSurfaceInput::ContextClick(at)
                        .into_operation(Some(target))
                        .expect("resolved media-list pointer target"),
                );
                match outcome.external_intent {
                    Some(RowIntent::Context(target)) => {
                        Some(Msg::Shell(ShellRequest::RowContextMenu(
                            crate::app::types_context_menu::ContextMenuTargets::Browser(vec![
                                target,
                            ]),
                            None,
                        )))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// This owner's local key interpretation, forwarded by the focused panel
    /// (the embedded owner's local key-handling contract, unchanged —
    /// the router owns every global chord and keeps precedence).
    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.inline_search.is_active() {
            return match self.inline_search.handle_key(key) {
                Some(InlineSearchAction::Activate { id, item_type }) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                        id,
                        item_type,
                    }))
                }
                Some(InlineSearchAction::Dismiss) => {
                    self.inline_search.close();
                    None
                }
                Some(InlineSearchAction::QueryStarted) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchQueryStarted))
                }
                None => self.inline_search_result_action(key),
            };
        }
        if key.modifiers.is_empty() && matches!(key.code, Key::Char('/')) {
            self.inline_search.open();
            return Some(Msg::Shell(ShellRequest::OpenInlineSearch));
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
        }
        if alt && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down) {
            return None;
        }
        // Local keyboard navigation routes through the same typed
        // `ShellRequest` the embedded owner emits
        // (`browser/keyboard.rs`): this owner mutates only its own selection,
        // then returns the resolved index in place of the raw key so the
        // shell drives persistence/pagination through the same arm as TV's
        // still-mounted browser — never by recomputing the App cursor here
        // (design D3: local movement sends the resolved value).
        match key.code {
            Key::Up | Key::Char('k') => {
                self.carrier
                    .delegate_operation(MediaListOperation::Move(-1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::Down | Key::Char('j') => {
                self.carrier.delegate_operation(MediaListOperation::Move(1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::PageUp => {
                self.carrier
                    .delegate_operation(MediaListOperation::Page(-1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::PageDown => {
                self.carrier.delegate_operation(MediaListOperation::Page(1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::Home => {
                self.carrier.delegate_operation(MediaListOperation::First);
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::End => {
                self.carrier.delegate_operation(MediaListOperation::Last);
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            _ => {}
        }
        let selected = self.selected_effect_item();
        let request = match key.code {
            Key::Enter => selected.map(|item| ShellRequest::EmbyLibraryActivate { item }),
            Key::Char('p') if ctrl => selected.map(|item| ShellRequest::EmbyLibraryPlay { item }),
            Key::Char('a') if ctrl => {
                selected.map(|item| ShellRequest::EmbyLibraryEnqueue { item })
            }
            Key::Char('w') if ctrl => {
                selected.map(|item| ShellRequest::EmbyLibraryToggleWatched { item })
            }
            Key::Char('.') if key.modifiers.is_empty() => match self
                .carrier
                .delegate_operation(MediaListOperation::ContextCurrent)
                .external_intent
            {
                Some(RowIntent::ContextSelection(targets)) => Some(ShellRequest::RowContextMenu(
                    crate::app::types_context_menu::ContextMenuTargets::Browser(targets),
                    None,
                )),
                Some(RowIntent::Context(target)) => Some(ShellRequest::RowContextMenu(
                    crate::app::types_context_menu::ContextMenuTargets::Browser(vec![target]),
                    None,
                )),
                _ => None,
            },
            Key::Char('s') if ctrl => {
                selected.map(|item| ShellRequest::EmbyLibraryShuffle { item })
            }
            Key::Char('r') if ctrl => Some(ShellRequest::EmbyLibraryRescan),
            Key::Char('r') => Some(ShellRequest::EmbyLibraryRefresh),
            Key::Esc | Key::Backspace => Some(ShellRequest::EmbyLibraryBack),
            Key::Char(c @ ('[' | ']')) if !ctrl && !alt => {
                let delta = if c == '[' { -1 } else { 1 };
                Some(if self.group_pills {
                    ShellRequest::EmbyLibraryCycleGroup { delta }
                } else {
                    ShellRequest::EmbyLibraryCycleLetterPill { delta }
                })
            }
            _ => None,
        };
        request.map(Msg::Shell)
    }
}

impl InlineSearchHost for EmbyLibraryContent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }

    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }
}
impl LibraryContentOwner for EmbyLibraryContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_selection();
    }

    fn hero_overlay_target_available(&mut self) -> bool {
        self.hero_item().is_some()
    }

    fn inline_search_session(&mut self) -> Option<&mut dyn InlineSearchHost> {
        Some(self)
    }

    fn inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        Some(self)
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

    fn scroll_position(&self) -> Option<(usize, usize)> {
        Some((self.cursor(), self.scroll()))
    }

    fn hero_scroll_offset(&self) -> usize {
        self.hero_scroll
    }

    fn hero_scroll(&mut self, delta: i16, max_offset: usize) -> bool {
        let next = if delta < 0 {
            self.hero_scroll.saturating_sub((-delta) as usize)
        } else {
            self.hero_scroll.saturating_add(delta as usize)
        }
        .min(max_offset);
        let changed = next != self.hero_scroll;
        self.hero_scroll = next;
        changed
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        let hero = self.hero_item().map(|item| {
            let data = hero_content_emby(item);
            let mut facts = data.facts;
            facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts,
                overview: data.overview,
                credits: data.credits,
                workspace: None,
            }
        });
        let selector = if self.group_pills {
            let pills: Vec<String> = std::iter::once("All".to_string())
                .chain(
                    self.feed_groups
                        .iter()
                        .map(|s| crate::app::ui_util::trunc_str(s, 12)),
                )
                .collect();
            Some(SelectorRow {
                pills,
                active: Some(self.feed_group_cursor),
            })
        } else if self.show_letter_pills {
            Some(SelectorRow {
                pills: LetterFilter::labels(),
                active: Some(self.letter_filter.as_ref().map(|f| f.index).unwrap_or(0)),
            })
        } else {
            None
        };
        let controls = self.home_video.then(|| ListControls {
            label: format!("{} items", self.total_count),
        });
        let list = if self.inline_search.is_active() {
            ListSlot::Search(&mut self.inline_search)
        } else if self.items.is_empty() {
            ListSlot::Empty {
                loading: self.loading,
                text: " (empty)".into(),
            }
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            controls,
            list,
            hero,
        }
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                Some(Msg::Shell(ShellRequest::EmbyLibraryPillClick {
                    target: index,
                }))
            }
            LibrarySlotEvent::List(input) => {
                if self.inline_search.is_active() {
                    return self.handle_search_pointer(input);
                }
                // Row-local claim gate (mirrors the owner's list-point claim):
                // only a point that resolves to a painted selectable row claims the
                // click; empty list space claims nothing.
                let target = match input {
                    MediaListSurfaceInput::Click(at)
                    | MediaListSurfaceInput::ToggleClick(at)
                    | MediaListSurfaceInput::RangeClick(at)
                    | MediaListSurfaceInput::DoubleClick(at)
                    | MediaListSurfaceInput::ContextClick(at) => {
                        self.carrier.resolve_current_point(at).cloned()
                    }
                    _ => None,
                };
                match input {
                    MediaListSurfaceInput::Wheel { .. } => {
                        // The resolved wheel echo drives the shell's
                        // `video_cursor`/resting-cursor write and pagination
                        // through the same typed arm as keyboard movement
                        // (`shell_emby_library.rs::handle_emby_library_request`).
                        self.carrier
                            .delegate_operation(MediaListOperation::Move(match input {
                                MediaListSurfaceInput::Wheel { delta, .. } => delta,
                                _ => 0,
                            }));
                        Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                            index: self.cursor(),
                        }))
                    }
                    MediaListSurfaceInput::Click(_at)
                    | MediaListSurfaceInput::ToggleClick(_at)
                    | MediaListSurfaceInput::RangeClick(_at) => {
                        let target = target?;
                        self.carrier
                            .delegate_operation(input.into_operation(Some(target.clone()))?);
                        let _ = ();
                        Some(Msg::Shell(ShellRequest::EmbyLibraryRowClick {
                            target: Some(target),
                        }))
                    }
                    MediaListSurfaceInput::DoubleClick(_at) => {
                        let target = target?;
                        self.carrier
                            .delegate_operation(MediaListOperation::Activate(target.clone()));
                        Some(Msg::Shell(ShellRequest::EmbyLibraryRowActivate {
                            target: Some(target),
                        }))
                    }
                    MediaListSurfaceInput::ContextClick(at) => {
                        let target = target?;
                        let outcome = self
                            .carrier
                            .delegate_operation(MediaListOperation::Context(target.clone()));
                        let targets = match outcome.external_intent {
                            Some(RowIntent::Context(target)) => vec![target],
                            Some(RowIntent::ContextSelection(targets)) => targets,
                            _ => vec![target],
                        };
                        Some(Msg::Shell(ShellRequest::RowContextMenu(
                            crate::app::types_context_menu::ContextMenuTargets::Browser(targets),
                            Some((at.x, at.y)),
                        )))
                    }
                    _ => None,
                }
            }
            // The Browser owner has no List-controls row, no Workspace and
            // no hero-pane input of its own.
            LibrarySlotEvent::ControlPicked(_)
            | LibrarySlotEvent::WorkspaceSelectorPicked(_)
            | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => self
                .selected_effect_item()
                .map(|item| Msg::Shell(ShellRequest::EmbyLibraryActivate { item })),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        let active = self.inline_search.is_active();
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if active
                && matches!(
                    key.code,
                    Key::Esc
                        | Key::Enter
                        | Key::Backspace
                        | Key::Up
                        | Key::Down
                        | Key::Left
                        | Key::Right
                        | Key::Char(_)
                ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    fn inline_search_active(&self) -> bool {
        self.inline_search.is_active()
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_item().map(hero_content_emby)
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
