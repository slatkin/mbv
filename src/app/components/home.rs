//! Interactive Component for the cross-Service Home destination.
//!
//! The component owns the selected section (pill) and section identity; the
//! embedded canonical controls (`WideMediaList` for hero-on-left Wide,
//! `InlineMediaBrowser` for inline Narrow) own the cursor and scroll over the
//! active section's projected rows. `render_home_content`
//! (`render/components/home.rs`) is the parent-owned hero + pill + chrome
//! painter and mounts the active control into the list area. Content is
//! mirrored from the shell; Home keyboard interpretation stays local. It emits
//! typed shell requests for effects that cross the Model boundary;
//! destination-independent chords are handled by the central router.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    InlineMediaBrowser, MediaListRow, MediaSemanticState, ViewportAnchor, WideMediaList,
};
use super::msg::{HomeHitRegion, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::HomeImagePaint;
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
    /// Canonical projection of the active section; the parent retains section
    /// identity. Both controls are fed the same active-section rows every
    /// `set_content`; only one is painted per breakpoint. They are the sole
    /// owner of cursor/scroll — the component keeps no mirror.
    canonical_list: WideMediaList<String>,
    inline_list: InlineMediaBrowser<String>,
    loading: bool,
    section: usize,
    /// Which canonical control the last `view()` painted (hero-on-left Wide vs
    /// inline Narrow). Drives the single `ViewportAnchor` handoff on a
    /// breakpoint transition and which control `cursor()` reads.
    wide: bool,
    focused: bool,
    /// Runtime terminal-capability flag (config-derived, not per-render
    /// content); set once by the shell after construction.
    use_nerd_fonts: bool,
    panel_area: Option<Rect>,
    hitmap: Vec<(Rect, usize)>,
    pill_targets: Vec<(Rect, usize)>,
    /// The current Continue Watching column target supplied by the shell's
    /// Model-owned `home_content` snapshot. It remains separate from the
    /// component's flat cursor, matching the legacy Home context-menu target.
    cw_item: Option<mbv_core::api::EmbyItem>,
    /// The cover image (if any) `view()` computed but could not paint
    /// itself (no `App`/image-cache authority); the shell takes it via
    /// `take_image_paint` right after `application.view()` returns and
    /// paints it using `App::paint_home_image`.
    image_paint: Option<HomeImagePaint>,
    /// The list area (`render_home_content`'s `left_area`) `view()` painted
    /// the rows into. Rebuilt every `view` like `hitmap`/`pill_targets`; this
    /// is Home's whole claim rect, so a click or wheel anywhere inside it is
    /// reported to the shell as a typed `HomeClick`/`HomeScroll` (the shell
    /// decides `App`'s cursor/focus/activation meaning). The component holds
    /// no double-click or scroll timing state — `App` owns that.
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
}

impl HomeComponent {
    pub fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            latest: Vec::new(),
            canonical_list: WideMediaList::new(),
            inline_list: InlineMediaBrowser::new(),
            loading: false,
            section: 0,
            wide: false,
            focused: false,
            use_nerd_fonts: false,
            panel_area: None,
            hitmap: Vec::new(),
            pill_targets: Vec::new(),
            cw_item: None,
            image_paint: None,
            list_area: Rect::default(),
            selected_item_rect: None,
            hero_area: None,
        }
    }

    pub(in crate::app) fn set_continue_watching_item(
        &mut self,
        item: Option<mbv_core::api::EmbyItem>,
    ) {
        self.cw_item = item;
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
    /// equals selectable index). Feeds both persistent controls; an ordinary
    /// refresh preserves the selected target through `ListCore::set_content`
    /// and locally clamps without any parent cursor/scroll input.
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
                semantic_state: MediaSemanticState::Ordinary,
            })
            .collect();
        self.canonical_list.set_content(rows.clone());
        self.inline_list.set_content(rows);
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    pub(in crate::app) fn set_use_nerd_fonts(&mut self, use_nerd_fonts: bool) {
        self.use_nerd_fonts = use_nerd_fonts;
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
            self.canonical_list.select_first();
            self.inline_list.select_first();
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
        let index = if self.wide {
            self.canonical_list.cursor()
        } else {
            self.inline_list.cursor()
        };
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

    /// Move the selection within the active section (clamped to its bounds) on
    /// both canonical controls in lockstep, so they stay cursor-aligned across
    /// a breakpoint transition. The keyboard navigation and the Model-boundary
    /// wheel scroll both use this (task 5.3d, Home wheel-scroll ownership) with
    /// the same delta semantics as keyboard Up/Down.
    pub(in crate::app) fn move_local_cursor(&mut self, delta: i64) {
        self.canonical_list.move_selection(delta);
        self.inline_list.move_selection(delta);
    }

    fn select_start(&mut self) {
        self.canonical_list.select_first();
        self.inline_list.select_first();
    }

    fn select_end(&mut self) {
        self.canonical_list.select_last();
        self.inline_list.select_last();
    }

    /// Move both controls' selection to the active-section row carrying flat
    /// index `flat` (a rebuilt hit-map target), when it is in the active
    /// section. Pre-#638 mouse compatibility: the bespoke `*HitRegion` path
    /// stays wired and reads control-exported geometry.
    fn select_flat(&mut self, flat: usize) {
        if let Some(index) = self.visible_indices().iter().position(|&idx| idx == flat) {
            self.canonical_list.select_index(index);
            self.inline_list.select_index(index);
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
        // the selection at its first row on both controls (no per-section
        // cursor cache).
        self.project_active_section();
        self.canonical_list.select_first();
        self.inline_list.select_first();
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

    /// Handle a keyboard event using TuiRealm key types. Home claims
    /// only its local navigation and typed effect requests; destination-
    /// independent chords are resolved by the central router.
    pub(in crate::app) fn handle_crossterm_key(&mut self, key: KeyEvent) -> Option<Msg> {
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
                self.move_local_cursor(-1);
                None
            }
            Key::Down => {
                self.move_local_cursor(1);
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
                self.move_local_cursor(-(self.page_size() as i64));
                None
            }
            Key::PageDown => {
                self.move_local_cursor(self.page_size() as i64);
                None
            }
            Key::Home => {
                self.select_start();
                None
            }
            Key::End => {
                self.select_end();
                None
            }
            Key::Char('.') => Some(Msg::Shell(ShellRequest::HomeContextMenu {
                home_cw_selected: self.section == 0,
                cw_item: self.cw_item.clone(),
            })),
            Key::Enter if ctrl => Some(Msg::Shell(ShellRequest::HomeEnqueue(self.cursor()))),
            Key::Enter => Some(Msg::Shell(ShellRequest::HomePlay(self.cursor()))),
            Key::Char('a') if ctrl => Some(Msg::Shell(ShellRequest::HomeEnqueue(self.cursor()))),
            Key::Char('w') if ctrl => Some(Msg::Shell(ShellRequest::HomeToggleWatched)),
            Key::Delete => Some(Msg::Shell(ShellRequest::HomeDelete(self.cursor()))),
            _ => None,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_crossterm_key(*key)
    }
    fn section_msg(&self, changed: bool) -> Option<Msg> {
        changed.then_some(Msg::Shell(ShellRequest::HomeSectionSelected(self.section)))
    }

    fn page_size(&self) -> usize {
        self.panel_area
            .map(|a| a.height as usize)
            .unwrap_or(1)
            .max(1)
    }

    /// Handle a TuiRealm mouse event. `None` means the event isn't Home's to
    /// handle (outside Home's own painted geometry — tab bar, queue panel,
    /// playback controls, the hero in two-column layout, ...); the caller
    /// falls through to the legacy mouse dispatch unchanged.
    ///
    /// The component owns *where* a Home click lands: it hit-tests against
    /// its own painted geometry (`list_area`, `hitmap`, `pill_targets`,
    /// rebuilt every `view`) and emits a typed `Msg::Shell` naming the region.
    /// It holds no double-click or scroll timing — the shell decides *when* a
    /// click counts against `App`'s own timing fields. Wheel scroll over the
    /// list area is claimed as `HomeScroll`: the shell moves the component's
    /// local cursor and, as a preserved pre-existing quirk, the independent
    /// Continue Watching column's cursor (`Model::handle_home_scroll` →
    /// `App::cw_move_cursor`), which this migration preserves rather than
    /// fixes (task 5.3d, Home wheel-scroll ownership).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let pos: ratatui::layout::Position = (mouse.column, mouse.row).into();
        match mouse.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                    -1
                } else {
                    1
                };
                if self.list_area.contains(pos) {
                    return Some(Msg::Shell(ShellRequest::HomeScroll { delta }));
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Section pills sit above the list area; claim them before
                // the row hit-test. `select_section` keeps the component's
                // own render state (section/cursor) authoritative; the shell
                // arm persists the selected source at the Model boundary.
                if let Some(section_idx) = self
                    .pill_targets
                    .iter()
                    .find(|(rect, _)| rect.contains(pos))
                    .map(|(_, section_idx)| *section_idx)
                {
                    self.select_section(section_idx);
                    return Some(Msg::Shell(ShellRequest::HomeClick {
                        region: HomeHitRegion::Pill(section_idx),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
                if self.list_area.contains(pos) {
                    // Local highlight only: the authoritative cursor set
                    // (row-map resolution, panel focus) stays in the shell.
                    // The `HomeClick` shell arm consumes the resolved cursor.
                    if let Some((_, flat_idx)) =
                        self.hitmap.iter().find(|(rect, _)| rect.contains(pos))
                    {
                        self.select_flat(*flat_idx);
                    }
                    return Some(Msg::Shell(ShellRequest::HomeClick {
                        region: HomeHitRegion::Row(self.cursor()),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            MouseEventKind::Down(MouseButton::Right) if self.list_area.contains(pos) => {
                // Resolve the row under the click before opening the menu;
                // a blank/gap click leaves the cursor unchanged (no hitmap
                // rect matches), so the context menu opens at the current
                // cursor. Matching the left-click/`BrowserComponent` row
                // treatment, resolving a row also moves the component-local
                // cursor so the emitted `ContextMenu` and the cursor agree
                // on the row under the click.
                if let Some((_, flat_idx)) = self.hitmap.iter().find(|(rect, _)| rect.contains(pos))
                {
                    self.select_flat(*flat_idx);
                }
                return Some(Msg::Shell(ShellRequest::HomeClick {
                    region: HomeHitRegion::ContextMenu(self.cursor()),
                    col: mouse.column,
                    row: mouse.row,
                }));
            }
            _ => {}
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn test_hitmap(&self) -> &[(Rect, usize)] {
        &self.hitmap
    }

    /// The active section's projected canonical rows (both controls hold the
    /// same vector).
    #[cfg(test)]
    pub(crate) fn test_active_rows(&self) -> &[MediaListRow<String>] {
        self.inline_list.rows()
    }

    /// The active control's resting scroll offset. `set_content` never seeds
    /// it, but the render pass persists the resolved scroll offset each frame
    /// (see `render_wide_media_list`). A `ViewportAnchor` handoff at a
    /// breakpoint transition can override it for discrete jumps.
    #[cfg(test)]
    pub(crate) fn test_active_scroll(&self) -> usize {
        if self.wide {
            self.canonical_list.scroll()
        } else {
            self.inline_list.scroll()
        }
    }
}

impl Default for HomeComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for HomeComponent {
    fn view(&mut self, f: &mut Frame, area: Rect) {
        // One `ViewportAnchor` handoff at a breakpoint transition: carry the
        // outgoing control's selected target and screen-row offset into the
        // incoming control (design.md D2). The cursors already track in
        // lockstep; the anchor keeps the offset continuous across the resize.
        let wide = crate::app::render::shared_hero_presentation(area).is_some();
        if wide != self.wide {
            let viewport_height = self.list_area.height.max(1) as usize;
            let anchor: Option<ViewportAnchor<String>> = if self.wide {
                self.canonical_list.viewport_anchor(viewport_height)
            } else {
                self.inline_list.viewport_anchor(viewport_height)
            };
            if let Some(anchor) = anchor {
                if wide {
                    self.canonical_list
                        .apply_viewport_anchor(&anchor, viewport_height);
                } else {
                    self.inline_list
                        .apply_viewport_anchor(&anchor, viewport_height);
                }
            }
            self.wide = wide;
        }

        let cursor = self.cursor();
        let result = crate::app::render::render_home_content(
            f,
            area,
            self.focused,
            &self.continue_items,
            &self.latest,
            self.section,
            cursor,
            &mut self.canonical_list,
            &self.inline_list,
            self.use_nerd_fonts,
        );
        self.section = result.resolved_section;
        self.hitmap = result.hitmap;
        self.pill_targets = result.pill_targets;
        self.list_area = result.left_area;
        self.selected_item_rect = result.selected_item_rect;
        self.image_paint = result.image_paint;
        self.hero_area = result.hero_area;
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for HomeComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
