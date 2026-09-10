//! Interactive Component for the cross-Service Home destination.
//!
//! The component owns the selected section (pill) and section identity; one
//! shared canonical `MediaList` owner of the active section's rows moves
//! between the `WideMediaList` (Wide hero Wide) and `InlineMediaBrowser`
//! (inline Narrow) Presentations — it owns the cursor and scroll, and the
//! component keeps no cursor mirror. `render_home_content`
//! (`render/components/home.rs`) is the parent-owned hero + pill + chrome
//! painter and mounts the active carrier into the list area. Content is
//! projected from the shell; Home keyboard interpretation stays local. It emits
//! typed shell requests for effects that cross the Model boundary;
//! destination-independent chords are handled by the central router.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, RowIntent,
    RowLocalInput, RowLocalOutcome,
};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{Msg, ShellRequest, TerminalObserverEvent};
use super::user_event::UserEvent;
use crate::app::render::{HomeCarrier, HomeImagePaint};
use crate::app::types_playback::HomeLatestSource;
use crate::app::ui_util::fmt_duration_short;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::QueueItem;

/// The resume-percentage badge legacy Home rows drew next to the title
/// (`TEXT_METADATA`, rendered as canonical `trailing`). Only for in-progress,
/// unfinished items with a non-zero rounded percentage — legacy Home rows show
/// no played marker and no active recolouring, so `semantic_state` stays
/// `Ordinary` for every Home row.
fn home_progress_badge(item: &QueueItem) -> Option<String> {
    let (position, runtime) = (item.playback_position_ticks(), item.runtime_ticks());
    (position > 0 && !item.played() && runtime > 0)
        .then(|| (position as i128 * 100 / runtime as i128) as u16)
        .filter(|pct| *pct > 0)
        .map(|pct| format!("{pct}%"))
}

/// The Interactive Component for the Home destination.
pub struct HomeComponent {
    continue_items: Vec<QueueItem>,
    latest: Vec<(String, HomeLatestSource, Vec<QueueItem>)>,
    /// The one shared canonical owner of the active section's rows, carried by
    /// exactly one of the persistent presentations (design.md D1). The owner
    /// holds rows, cursor, scroll, and selected target; a breakpoint change
    /// moves the same owner between carriers.
    carrier: MediaListCarrier<String>,
    loading: bool,
    section: usize,
    /// Which presentation the last `view()` painted (Wide hero Wide vs inline
    /// Narrow). Derived from the painted breakpoint; a transition moves the
    /// shared owner between carriers.
    wide: bool,
    focused: bool,
    /// Runtime terminal-capability flag (config-derived, not per-render
    /// content); set once by the shell after construction.
    use_nerd_fonts: bool,
    images_enabled: bool,
    pill_targets: Vec<(Rect, usize)>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle.
    mouse_gestures: MouseGestureState,
    /// Section-pill rects as last-push-wins rectangles (design.md D6),
    /// repopulated in `view()` from `pill_targets`.
    pill_regions: HitRegions<usize>,
    /// The cover image (if any) `view()` computed but could not paint
    /// itself (no `App`/image-cache authority); the shell takes it via
    /// `take_image_paint` right after `application.view()` returns and
    /// paints it using `App::paint_home_image`.
    image_paint: Option<HomeImagePaint>,
    /// The list area (`render_home_content`'s `left_area`) `view()` painted
    /// the rows into. Rebuilt every `view` like `pill_targets`; this
    /// is Home's whole claim rect, so a click or wheel anywhere inside it is
    /// recognized by the private `MouseGestureState`. The double-click window
    /// and wheel throttle live in `mouse_gestures`.
    list_area: Rect,
    /// The selected row's painted rect (`render_home_content`'s
    /// `selected_item_rect`), retained for the shell to anchor the Home
    /// context menu against what the component actually painted rather than
    /// the legacy `AppLayout` copy (task 5.3d, Home menu-placement geometry).
    /// `None` when this render produced no selection rect, matching the
    /// legacy copy's own optionality.
    selected_item_rect: Option<Rect>,
    /// The hero panel `render_home_content` painted this `view` (its
    /// `hero_area`), retained so the single painter's own geometry is
    /// observable to characterization tests without any layout mirror (task
    /// 5.3d, Home legacy underpaint removal). `None` when this render
    /// painted no hero (too short, or no hero item).
    hero_area: Option<Rect>,
    /// Session-only Wide hero list-pane width override (per-draw shell push,
    /// `None` = default ratio). Forwarded into the shared split; never stored
    /// clamped.
    list_pane_width: Option<u16>,
}

impl HomeComponent {
    pub fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            latest: Vec::new(),
            carrier: MediaListCarrier::new(Presentation::Inline),
            loading: false,
            section: 0,
            wide: false,
            focused: false,
            use_nerd_fonts: false,
            images_enabled: true,
            pill_targets: Vec::new(),
            mouse_gestures: MouseGestureState::new(),
            pill_regions: HitRegions::new(),
            image_paint: None,
            list_area: Rect::default(),
            selected_item_rect: None,
            hero_area: None,
            list_pane_width: None,
        }
    }

    /// Replace the shell-owned content snapshot. Section/cursor clamp to
    /// the new content (this is the async section clamp; the component is
    /// the sole owner of the numeric section).
    pub(in crate::app) fn set_content(
        &mut self,
        continue_items: Vec<QueueItem>,
        latest: Vec<(String, HomeLatestSource, Vec<QueueItem>)>,
        loading: bool,
    ) {
        self.continue_items = continue_items;
        self.latest = latest;
        self.loading = loading;
        self.clamp_section();
        self.project_active_section();
    }

    /// Project only the active Home section's items as canonical `Item` rows
    /// (Home has no `Heading`/`Spacer` vocabulary, so structural-row index
    /// equals selectable index). Feeds the shared owner wherever it currently
    /// resides; an ordinary refresh preserves the selected target through
    /// `MediaList::set_content` and locally clamps without any parent
    /// cursor/scroll input.
    fn project_active_section(&mut self) {
        self.ensure_carrier();
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
            .map(|item| MediaListRow::Item {
                primary: item.display_name(),
                // Stable per-item identity (Emby id / feed guid / ABS episode
                // id) — the same id the queue/shell treat as canonical — so an
                // ordinary refresh retains the selection by identity, not by a
                // title that can collide across episodes.
                target: item.id().to_owned(),
                trailing: home_progress_badge(item),
                duration: item
                    .duration()
                    .map(|ticks| fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64)),
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
            })
            .collect();
        self.carrier.set_content(rows);
    }

    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn set_use_nerd_fonts(&mut self, use_nerd_fonts: bool) {
        self.use_nerd_fonts = use_nerd_fonts;
    }

    pub(in crate::app) fn set_images_enabled(&mut self, images_enabled: bool) {
        self.images_enabled = images_enabled;
    }

    /// Records the session-only Wide hero list-pane width override for the
    /// next `view()`. Pushed each frame by `render_home_component`; it is a
    /// layout fact, not content, so it never enters the event-scoped
    /// `set_content` projection.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.list_pane_width = list_pane_width;
    }

    /// Takes the cover image (if any) `view()` computed but could not
    /// paint itself. The shell calls this right after `application.view()`
    /// returns and paints it via `App::paint_home_image`.
    pub(in crate::app) fn take_image_paint(
        &mut self,
    ) -> Option<crate::app::render::HomeImagePaint> {
        self.image_paint.take()
    }

    /// Restore a persisted pill selection once a section matching `source`
    /// exists, mirroring the `home_section_pending` restore the shell applies
    /// on `push_home_content`. Returns `true` once restored (the shell clears the
    /// pending marker afterward).
    pub(in crate::app) fn restore_section(&mut self, source: &HomeLatestSource) -> bool {
        if let Some(idx) = self.latest.iter().position(|(_, s, _)| s == source) {
            self.section = idx + 1;
            self.clamp_section();
            self.project_active_section();
            self.delegate_row_local_input(RowLocalInput::First, None);
            true
        } else {
            false
        }
    }

    /// The flat cursor (Continue Watching + every latest section) the shell's
    /// `home_flat_target` resolves. Derived from the active canonical control's
    /// selectable index over the active section's rows; the component keeps no
    /// cursor of its own.
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
    /// here keeps the off-by-one rule in the component (the sole numeric
    /// section owner); the shell persists this identity, never the index
    /// (task 5.3d).
    pub(in crate::app) fn source_for_section(&self, section: usize) -> Option<HomeLatestSource> {
        if section == 0 {
            return None;
        }
        self.latest
            .get(section - 1)
            .map(|(_, source, _)| source.clone())
    }

    /// Home's whole painted panel rect (`list_area`) and its selected-row
    /// rect, for the shell to place the context menu over what this component
    /// actually painted rather than the legacy `AppLayout` copies (task 5.3d,
    /// Home menu-placement geometry). `selected_item_rect` is `None` when this
    /// render produced no selection rect.
    pub(in crate::app) fn menu_placement_geometry(&self) -> (Rect, Option<Rect>) {
        (self.list_area, self.selected_item_rect)
    }

    /// The hero panel `view()` painted this render (the single painter's own
    /// geometry, for characterization), `None` when it painted none. Not a
    /// layout mirror — the component owns every Home `view`-painted rect.
    pub(in crate::app) fn hero_area(&self) -> Option<Rect> {
        self.hero_area
    }

    #[cfg(test)]
    pub(crate) fn test_pill_targets(&self) -> &[(Rect, usize)] {
        &self.pill_targets
    }

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

    /// The one seam through which Home offers an already-normalized row-local
    /// key or pointer gesture to the shared owner carrying its active section.
    /// The owner applies the local state transition and returns the closed
    /// provider-neutral outcome; Home translates external row intents into its
    /// typed Msgs (design.md D3).
    fn delegate_row_local_input(
        &mut self,
        input: RowLocalInput,
        pointer_target: Option<String>,
    ) -> RowLocalOutcome<String> {
        self.ensure_carrier();
        self.carrier.delegate(input, pointer_target)
    }

    /// Home's typed request target for a stable item identity the shared owner
    /// resolved, or the owner's current selection for a local effect. The
    /// identity is never re-derived from a cursor minus a section start index.
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
        self.delegate_row_local_input(RowLocalInput::First, None);
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

    /// The typed effect target for Home's current selection (the shared
    /// owner's stable target, never a cursor-minus-section-index lookup).
    fn row_target(&self) -> super::msg::HomeRowTarget {
        self.home_row_target(self.carrier.selected_target().cloned())
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !self.focused {
            return None;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        match key.code {
            Key::Up => {
                self.delegate_row_local_input(RowLocalInput::Move(-1), None);
                None
            }
            Key::Down => {
                self.delegate_row_local_input(RowLocalInput::Move(1), None);
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
                self.delegate_row_local_input(RowLocalInput::Page(-1), None);
                None
            }
            Key::PageDown => {
                self.delegate_row_local_input(RowLocalInput::Page(1), None);
                None
            }
            Key::Home => {
                self.delegate_row_local_input(RowLocalInput::First, None);
                None
            }
            Key::End => {
                self.delegate_row_local_input(RowLocalInput::Last, None);
                None
            }
            Key::Char('.') if self.section == 0 => {
                let target = match self.delegate_row_local_input(RowLocalInput::Context, None) {
                    RowLocalOutcome::External(RowIntent::Context(target)) => {
                        self.home_row_target(Some(target))
                    }
                    _ => self.row_target(),
                };
                Some(Msg::Shell(ShellRequest::HomeContextMenu {
                    home_cw_selected: true,
                    target,
                }))
            }
            Key::Char('.') => None,
            Key::Enter if ctrl => Some(Msg::Shell(ShellRequest::HomeEnqueue(self.row_target()))),
            Key::Enter => match self.delegate_row_local_input(RowLocalInput::Activate, None) {
                RowLocalOutcome::External(RowIntent::Activate(target)) => Some(Msg::Shell(
                    ShellRequest::HomePlay(self.home_row_target(Some(target))),
                )),
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

    fn section_msg(&self, changed: bool) -> Option<Msg> {
        changed.then_some(Msg::Shell(ShellRequest::HomeSectionSelected(self.section)))
    }

    /// Handle a TuiRealm mouse event. `None` means the event isn't Home's to
    /// handle (outside Home's own painted geometry — tab bar, queue panel,
    /// playback controls, the hero in two-column layout, ...); the caller
    /// falls through to the legacy mouse dispatch unchanged.
    ///
    /// Gesture recognition (click / double-click / right-click / wheel) comes
    /// from the private `MouseGestureState` (ADR 0024, design.md D3). Row
    /// identity comes from the embedded control's `resolve_point`
    /// (design.md D6); section pills from `pill_regions`. The component emits
    /// a semantic `Msg` with a resolved target — never raw coordinates —
    /// except the context-menu anchor (design.md D4). Wheel movement mutates
    /// the active canonical control locally; only the Continue Watching cursor
    /// authority crosses the boundary.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Home does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                // Home's hero is destination-painted chrome outside the
                // carrier's list area, so the destination keeps claiming it
                // here (design Non-Goals: chrome stays with the parent).
                let claimed = self.carrier.claims_current_point(at)
                    || self.hero_area.is_some_and(|hero| hero.contains(at));
                if !claimed {
                    return None;
                }
                self.delegate_row_local_input(RowLocalInput::Wheel { at, delta }, None);
                // Return a framework-visible claim after mutating local state;
                // the shell resolves effects from the component's selected
                // stable target rather than an App-wide cursor mirror.
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::Click(at) => {
                if let Some(&section_idx) = self.pill_regions.resolve(at) {
                    self.select_section(section_idx);
                    return Some(Msg::Shell(ShellRequest::HomePillClick {
                        target: section_idx,
                    }));
                }
                if !self.claim_row(at) {
                    return None;
                }
                Some(Msg::Shell(ShellRequest::HomeRowClick {
                    target: self.row_target(),
                }))
            }
            MouseGesture::DoubleClick(at) => {
                if let Some(&section_idx) = self.pill_regions.resolve(at) {
                    self.select_section(section_idx);
                    return Some(Msg::Shell(ShellRequest::HomePillClick {
                        target: section_idx,
                    }));
                }
                if !self.claim_row(at) {
                    return None;
                }
                Some(Msg::Shell(ShellRequest::HomeRowActivate {
                    target: self.row_target(),
                }))
            }
            MouseGesture::RightClick(at) => {
                if !self.claim_row(at) {
                    return None;
                }
                Some(Msg::Shell(ShellRequest::HomeRowContextMenu {
                    target: self.row_target(),
                    anchor: (mouse.column, mouse.row),
                }))
            }
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => None,
        }
    }

    /// If `at` lands in the Home list area, move the selection to the row
    /// under it (a blank/gap click leaves it unchanged, matching the legacy
    /// hit map) and return `true`.
    fn claim_row(&mut self, at: Position) -> bool {
        if !self.list_area.contains(at) {
            return false;
        }
        if let Some(id) = self.resolve_row_id(at) {
            self.delegate_row_local_input(RowLocalInput::Click(at), Some(id));
        }
        true
    }

    /// The stable item id under `point`, resolved by the embedded canonical
    /// control that painted the active list (design.md D6). The inline hero
    /// covers the selected item, so a hero click carries the current
    /// selection.
    fn resolve_row_id(&self, point: Position) -> Option<String> {
        self.carrier.resolve_current_point(point).cloned()
    }

    /// Test seam: reset the private gesture recognizer so a synchronous test
    /// loop can drive successive wheel/click events without the
    /// throttle/double-click window collapsing them.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }

    /// The visible row rectangles paired with their flat index, reproduced
    /// from the active control's exported row geometry (the same mapping the
    /// deleted parent hit map used). The selected inline detail block is
    /// appended, mirroring the legacy hit map.
    #[cfg(test)]
    pub(crate) fn test_hitmap(&self) -> Vec<(Rect, usize)> {
        use super::media_list::RowGeometry;
        fn rows(g: &RowGeometry<String>, area: Rect, flat: &[usize]) -> Vec<(Rect, usize)> {
            let offset = g.offset();
            g.visible_rows(area)
                .into_iter()
                .enumerate()
                .filter_map(|(i, rect)| Some((rect, *flat.get(g.source_row(offset + i)?)?)))
                .collect()
        }
        let flat = self.visible_indices();
        if self.carrier.active() == Presentation::Wide {
            rows(
                &self
                    .carrier
                    .wide()
                    .row_geometry(self.list_area.height as usize),
                self.list_area,
                &flat,
            )
        } else {
            let detail_rows = self.hero_area.map_or(0, |hero| hero.height as usize);
            let mut map = rows(
                &self
                    .carrier
                    .inline()
                    .row_geometry(self.list_area.height as usize, detail_rows),
                self.list_area,
                &flat,
            );
            if let Some(hero) = self.hero_area {
                map.push((hero, self.cursor()));
            }
            map
        }
    }

    /// The active section's projected canonical rows (the carrier holds the
    /// active vector).
    #[cfg(test)]
    pub(crate) fn test_active_rows(&self) -> &[MediaListRow<String>] {
        self.carrier.rows()
    }

    /// The active carrier's resting scroll offset. `set_content` never seeds
    /// it, but the render pass persists the resolved scroll offset each frame
    /// (see `render_wide_media_list`). A `ViewportAnchor` handoff at a
    /// breakpoint transition can override it for discrete jumps.
    #[cfg(test)]
    pub(crate) fn test_active_scroll(&self) -> usize {
        self.carrier.scroll()
    }

    /// The presentation the painted breakpoint currently selects (design.md
    /// D2).
    fn active_presentation(&self) -> Presentation {
        if self.wide {
            Presentation::Wide
        } else {
            Presentation::Inline
        }
    }

    /// Move the shared owner into the active presentation when they diverge.
    /// A responsive change reads the same owner and preserves only the
    /// outgoing selected-row viewport offset (design.md D1); no cursor, scroll,
    /// or selection is copied between presentations.
    fn ensure_carrier(&mut self) {
        let target = self.active_presentation();
        let viewport_height = self.list_area.height.max(1) as usize;
        self.carrier.ensure_presentation(target, viewport_height);
    }
}

impl Default for HomeComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for HomeComponent {
    fn view(&mut self, f: &mut Frame, area: Rect) {
        // One shared owner per logical row flow (design.md D1): a breakpoint
        // change reconfigures the same owner and preserves only the outgoing
        // selected-row viewport offset — the owner is never copied between
        // presentations.
        let wide = crate::app::render::wide_hero_fits(area);
        self.wide = wide;
        self.ensure_carrier();

        let cursor = self.cursor();
        let control = if self.carrier.active() == Presentation::Wide {
            HomeCarrier::Wide(self.carrier.wide_mut())
        } else {
            HomeCarrier::Inline(self.carrier.inline_mut())
        };
        let result = crate::app::render::render_home_content(
            f,
            area,
            self.focused,
            &self.continue_items,
            &self.latest,
            self.section,
            cursor,
            control,
            self.use_nerd_fonts,
            self.images_enabled,
            self.list_pane_width,
        );
        self.pill_targets = result.pill_targets;
        self.list_area = result.left_area;
        self.selected_item_rect = result.selected_item_rect;
        self.image_paint = result.image_paint;
        self.hero_area = result.hero_area;

        // Adopt the section-pill rects the composer just painted into the
        // irregular-chrome registry (design.md D6).
        self.pill_regions.clear();
        for (rect, target) in &self.pill_targets {
            self.pill_regions.push(*rect, *target);
        }
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for HomeComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        // Keep the shared owner in the presentation the painted breakpoint
        // currently selects before any row-local input touches it (design.md
        // D1).
        match ev {
            Event::Keyboard(key) => {
                self.ensure_carrier();
                self.handle_key(key)
            }
            Event::Mouse(mouse) => {
                self.ensure_carrier();
                self.handle_mouse(mouse)
            }
            _ => None,
        }
    }
}
