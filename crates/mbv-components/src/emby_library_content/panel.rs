//! Library-panel boundary adapter for the Movies/HomeVideos/Generic Emby content
//! owner: per-frame panel content, slot events, and launch-state identities.
use tuirealm::event::{Key, KeyEvent};

use super::EmbyLibraryContent;
use crate::inline_search::{InlineSearch, InlineSearchHost};
use crate::library_panel::HeroContentData;
use crate::library_panel::content::{HeroContent, LibraryPanelContent, ListSlot, SelectorRow};
use crate::library_panel::hero::hero_content_emby;
use crate::library_panel::owner::{LaunchSelector, LibraryContentOwner, LibrarySlotEvent};
use crate::media_list::{MediaListOperation, MediaListSurfaceInput, RowIntent};
use mbv_config::{EmbySelectorKey, LibraryItemIdentity, SelectorIdentity};
use mbv_render::components::tv_wide::HeroImageState;
use mbv_ui_model::sort_filter::LetterFilter;
use mbv_ui_msg::{LeafKeyResult, Msg, ShellRequest};

impl InlineSearchHost for EmbyLibraryContent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }

    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }
}
impl LibraryContentOwner for EmbyLibraryContent {
    /// Forwards to the inherent content-preserving reset (design D3).
    fn reset_presentation(&mut self) {
        self.reset_presentation();
    }

    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
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

    fn set_selection_origin(&mut self, origin: mbv_ui_model::media_list::SelectionOrigin) {
        self.carrier.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<mbv_ui_msg::SelectionSummary> {
        Some(self.carrier.selection_summary())
    }

    fn scroll_position(&self) -> Option<(usize, usize)> {
        Some((self.cursor(), self.scroll()))
    }

    fn hero_scroll_offset(&self) -> usize {
        self.hero_scroll
    }

    fn hero_scroll(&mut self, delta: i16, max_offset: usize) -> bool {
        let step = usize::from(delta.unsigned_abs());
        let next = if delta < 0 {
            self.hero_scroll.saturating_sub(step)
        } else {
            self.hero_scroll.saturating_add(step)
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
                overview_title: None,
                workspace: None,
            }
        });
        let selector = if self.selector_mode == super::EmbySelectorMode::FeedGroups {
            let pills: Vec<String> = std::iter::once("Latest".to_string())
                .chain(std::iter::once("All".to_string()))
                .chain(
                    self.feed_groups
                        .iter()
                        .map(|s| mbv_ui_model::ui_util::trunc_str(s, 12)),
                )
                .collect();
            let markers = super::super::selector_markers(pills.len(), self.latest_marker);
            Some(SelectorRow {
                pills,
                markers,
                active: Some(if self.latest_mode {
                    0
                } else {
                    self.feed_group_cursor + 1
                }),
            })
        } else if self.selector_mode == super::EmbySelectorMode::Letters {
            let pills: Vec<String> = std::iter::once("Latest".to_string())
                .chain(LetterFilter::labels())
                .collect();
            let markers = super::super::selector_markers(pills.len(), self.latest_marker);
            Some(SelectorRow {
                pills,
                markers,
                active: Some(if self.latest_mode {
                    0
                } else {
                    self.letter_filter.as_ref().map_or(1, |f| f.index + 1)
                }),
            })
        } else {
            None
        };
        let list = if self.inline_search.is_active() {
            ListSlot::Search(&mut self.inline_search)
        } else if self.items().is_empty() {
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

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => Some(self.pick_selector(index)),
            LibrarySlotEvent::List(input) => self.on_list_event(input),
            // The Browser owner has no Workspace and no hero-pane input of
            // its own.
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => self
                .selected_effect_item()
                .map(|item| Msg::Shell(Box::new(ShellRequest::EmbyLibraryActivate { item }))),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        let active = self.inline_search.is_active();
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
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

    /// Bounded read-only launch-state identities (task 2.1): the current
    /// main-Selector pill as a stable key — the closed letter bucket's fixed
    /// value, or the selected feed/home-video group's folder content ID —
    /// plus the shared carrier's stable item target. The "All" group pill
    /// and the unfiltered letter view use an explicit unfiltered identity;
    /// a destination with no pills, or an empty list, reports absence. No
    /// pill index, group display name, or row position crosses.
    fn launch_selector(&self, state: &mbv_config::TuiLaunchState) -> Option<LaunchSelector> {
        // Only the persisted scope applies: every library with a painted
        // selector row restarts on Latest. Legacy letter/group/unfiltered
        // selectors in an old snapshot decode but resolve to the library
        // default (the Home precedent), and a pill-less owner (no selector
        // row at this position) is never forced into Latest mode.
        if !matches!(
            state.selector.as_ref(),
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Latest
            })
        ) {
            return None;
        }
        if self.selector_mode == super::EmbySelectorMode::None {
            return None;
        }
        (!self.latest_mode).then_some(LaunchSelector::EmbyLatest)
    }

    fn reanchor_launch_state(&mut self, _state: &mbv_config::TuiLaunchState) -> bool {
        if self.loading && self.items().is_empty() {
            return false;
        }
        // The shell applies the selector through App and pushes the resulting
        // content before this item-level re-anchor. Do not rewrite the
        // component-local selector here; that brief mirror could disagree
        // with the shell projection until the next sync pass.
        // A saved item never restores (spec: every restart lands on the
        // first row), so a legacy snapshot's item is ignored here too.
        self.carrier.select_first();
        true
    }

    /// The persisted pill scope, not the live pill: every library with a
    /// painted selector row restarts on Latest, and any other pill choice is
    /// session memory. The selected item is never recorded, so restoration
    /// always lands on the first row (a legacy snapshot's saved item decodes
    /// but is ignored).
    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let selector = (self.selector_mode != super::EmbySelectorMode::None).then_some(
            SelectorIdentity::Emby {
                key: EmbySelectorKey::Latest,
            },
        );
        (selector, None)
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

impl EmbyLibraryContent {
    /// The wheel's paging reach report (wheel-scrolls-viewport D4, task
    /// 4.1): the last painted selectable row of the retained frame resolved
    /// to an item index — the same index space this owner's
    /// `EmbyLibraryCursorIndex` reports use (structural rows never reach it).
    /// Nothing painted resolves to nothing: with no frame the wheel scrolls
    /// locally and reports no reach.
    fn wheel_reach_message(&self) -> Option<Msg> {
        Some(Msg::Shell(Box::new(ShellRequest::LibraryViewportReach {
            index: self.last_painted_reach_index()?,
        })))
    }

    fn last_painted_reach_index(&self) -> Option<usize> {
        let rect = self.carrier.current_content_rect()?;
        if rect.height == 0 || rect.width == 0 {
            return None;
        }
        let rows = self.carrier.rows();
        // wheel-scrolls-viewport 4.1 review P1: an empty list has no flow row
        // to clamp to, so the range below would slice `[0..=0]` out of
        // nothing and panic. Nothing painted resolves to nothing, an empty
        // list reports no reach.
        if rows.is_empty() {
            return None;
        }
        // Resolve over the viewport the frame actually painted, then take
        // the last selectable flow row the frame could have drawn.
        let viewport = self
            .carrier
            .wide()
            .resolve_viewport(usize::from(rect.height));
        let last_flow_row = (viewport.offset + viewport.height)
            .saturating_sub(1)
            .min(rows.len().saturating_sub(1));
        let target = rows[..=last_flow_row]
            .iter()
            .rev()
            .find_map(|row| row.selectable_target())?;
        self.items().iter().position(|item| &item.id == target)
    }

    fn on_list_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
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
        self.on_list_point_event(input, target)
    }

    fn on_list_point_event(
        &mut self,
        input: MediaListSurfaceInput,
        target: Option<String>,
    ) -> Option<Msg> {
        match input {
            MediaListSurfaceInput::Wheel { delta, .. } => {
                // wheel-scrolls-viewport D4, task 4.1: the wheel scrolls the
                // viewport freely inside the embedded control and never
                // moves the selection, so the shell's cursor-echo effects
                // (the resting `video_cursor` write and pagination from the
                // cursor) do not run. Pagination takes the post-scroll reach
                // report from the last painted selectable row instead.
                self.carrier
                    .delegate_operation(MediaListOperation::Scroll(delta));
                self.wheel_reach_message()
            }
            MediaListSurfaceInput::Click(_at)
            | MediaListSurfaceInput::ToggleClick(_at)
            | MediaListSurfaceInput::RangeClick(_at) => {
                let target = target?;
                self.carrier
                    .delegate_operation(input.into_operation(Some(target.clone()))?);
                let () = ();
                Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryRowClick {
                    target: Some(target),
                })))
            }
            MediaListSurfaceInput::DoubleClick(_at) => {
                let target = target?;
                self.carrier
                    .delegate_operation(MediaListOperation::Activate(target.clone()));
                Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryRowActivate {
                    target: Some(target),
                })))
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
                // Design D7: the menu targets the items this list paints,
                // resolved here rather than re-derived by the shell; a
                // target that resolves to nothing is dropped.
                let items = self.items_for_targets(&targets);
                (!items.is_empty()).then(|| {
                    Msg::Shell(Box::new(ShellRequest::RowContextMenu(
                        mbv_ui_model::context_menu::ContextMenuTargets::Emby(items),
                        Some((at.x, at.y)),
                    )))
                })
            }
            _ => None,
        }
    }
}
