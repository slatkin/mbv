//! Library-panel boundary adapter for the Movies/HomeVideos/Generic Emby content
//! owner: per-frame panel content, slot events, and launch-state identities.
use tuirealm::event::{Key, KeyEvent};

use super::EmbyLibraryContent;
use crate::app::components::inline_search::{InlineSearch, InlineSearchHost};
use crate::app::components::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow,
};
use crate::app::components::library_panel::hero::hero_content_emby;
use crate::app::components::library_panel::owner::{
    LaunchSelector, LibraryContentOwner, LibrarySlotEvent,
};
use crate::app::components::library_panel::HeroContentData;
use crate::app::components::media_list::{MediaListOperation, MediaListSurfaceInput, RowIntent};
use crate::app::components::msg::{LeafKeyResult, Msg, ShellRequest};
use crate::app::render::LetterFilter;
use mbv_core::config::{EmbyLetterBucket, EmbySelectorKey, LibraryItemIdentity, SelectorIdentity};

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
            let pills: Vec<String> = std::iter::once("Latest".to_string())
                .chain(std::iter::once("All".to_string()))
                .chain(
                    self.feed_groups
                        .iter()
                        .map(|s| crate::app::ui_util::trunc_str(s, 12)),
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
        } else if self.show_letter_pills {
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
                    self.letter_filter
                        .as_ref()
                        .map(|f| f.index + 1)
                        .unwrap_or(1)
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
            LibrarySlotEvent::SelectorPicked(index) => self.pick_selector(index),
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
                        // (`shell/emby_library.rs::handle_emby_library_request`).
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
                            crate::app::state::types::context_menu::ContextMenuTargets::Browser(
                                targets,
                            ),
                            Some((at.x, at.y)),
                        )))
                    }
                    _ => None,
                }
            }
            // The Browser owner has no Workspace and no hero-pane input of
            // its own.
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::HeroPane(_) => None,
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

    /// Bounded read-only launch-state identities (task 2.1): the current
    /// main-Selector pill as a stable key — the closed letter bucket's fixed
    /// value, or the selected feed/home-video group's folder content ID —
    /// plus the shared carrier's stable item target. The "All" group pill
    /// and the unfiltered letter view use an explicit unfiltered identity;
    /// a destination with no pills, or an empty list, reports absence. No
    /// pill index, group display name, or row position crosses.
    fn launch_selector(&self, state: &mbv_core::config::TuiLaunchState) -> Option<LaunchSelector> {
        if matches!(
            state.selector.as_ref(),
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Latest
            })
        ) {
            return (!self.latest_mode).then_some(LaunchSelector::EmbyLatest);
        }
        if self.group_pills {
            let target = match state.selector.as_ref() {
                Some(SelectorIdentity::Emby {
                    key: EmbySelectorKey::Group(id),
                }) => self
                    .feed_group_ids
                    .iter()
                    .position(|candidate| candidate == id)
                    .map(|index| index + 1)
                    .unwrap_or(0),
                _ => 0,
            };
            return (self.latest_mode || self.feed_group_cursor != target)
                .then_some(LaunchSelector::Emby { index: target });
        }
        if self.show_letter_pills {
            let current = self.letter_filter.as_ref().map(|filter| filter.index);
            return match state.selector.as_ref() {
                Some(SelectorIdentity::Emby {
                    key: EmbySelectorKey::Letter(bucket),
                }) => {
                    let target = bucket.to_index();
                    (self.latest_mode || current != Some(target))
                        .then_some(LaunchSelector::Emby { index: target })
                }
                // No letter pill is represented by an index. The shell uses
                // this out-of-band value for the distinct clear intent.
                _ if current.is_some() || self.latest_mode => {
                    Some(LaunchSelector::Emby { index: usize::MAX })
                }
                _ => None,
            };
        }
        None
    }

    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        if self.loading && self.items().is_empty() {
            return false;
        }
        // The shell applies the selector through App and pushes the resulting
        // content before this item-level re-anchor. Do not rewrite the
        // component-local selector here; that brief mirror could disagree
        // with the shell projection until the next sync pass.
        let selected = match state.item.as_ref() {
            Some(LibraryItemIdentity::Emby { id }) => self.carrier.select_target(id),
            _ => false,
        };
        if !selected {
            self.carrier.select_first();
        }
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let selector = if self.latest_mode {
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Latest,
            })
        } else if self.group_pills {
            if self.feed_group_cursor == 0 {
                Some(SelectorIdentity::Emby {
                    key: EmbySelectorKey::Unfiltered,
                })
            } else {
                self.feed_group_cursor.checked_sub(1).and_then(|group| {
                    self.feed_group_ids
                        .get(group)
                        .cloned()
                        .map(|id| SelectorIdentity::Emby {
                            key: EmbySelectorKey::Group(id),
                        })
                })
            }
        } else if self.show_letter_pills {
            Some(SelectorIdentity::Emby {
                key: self
                    .letter_filter
                    .as_ref()
                    .map(|filter| {
                        EmbyLetterBucket::from_index(filter.index)
                            .expect("LetterFilter index comes from LETTER_FILTER_BUCKETS")
                    })
                    .map(EmbySelectorKey::Letter)
                    .unwrap_or(EmbySelectorKey::Unfiltered),
            })
        } else {
            None
        };
        let item = self
            .carrier
            .selected_target()
            .cloned()
            .map(|id| LibraryItemIdentity::Emby { id });
        (selector, item)
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
