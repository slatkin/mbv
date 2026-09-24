//! Live-tick coverage for the mounted `LibraryPanel` (task 5.9). Proves, in
//! one real `Application::tick()` flow through the shell sync pass (ADR 0024,
//! design D15): framework focus follows the active library's migration state,
//! mouse eligibility follows the painted panel, painted pill clicks route
//! `SelectorPicked` to the active owner, the Wide split drag is owned by
//! the panel, an inactive owner
//! keeps its cursor/scroll across a tab change, and a library leaving the
//! catalog retires its owner. No destination converts here: the migrated
//! path is exercised through a test-owned fixture content owner, the minimal
//! owner the panel can host.

use std::cell::RefCell;
use std::rc::Rc;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow, Workspace,
    WorkspaceHeader,
};
use crate::app::components::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use crate::app::components::library_panel::{
    hero_content_emby, ArtworkShape, HeroArtwork, HeroContentData, HeroFacts, LibraryKey,
};
use crate::app::components::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaListSurfaceInput, MediaSemanticState,
};
use crate::app::components::msg::{Msg, TerminalObserverEvent};
use crate::app::components::{ComponentId, LibraryKind};
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};

// ── Fixture content owner ───────────────────────────────────────────────

#[derive(Default)]
struct FixtureLog {
    events: Vec<LibrarySlotEvent>,
    selections: Vec<Option<String>>,
}

struct FixtureOwner {
    carrier: MediaListCarrier<String>,
    workspace_carrier: MediaListCarrier<String>,
    log: Rc<RefCell<FixtureLog>>,
    workspace: bool,
    workspace_focused: bool,
}

impl FixtureOwner {
    fn new(log: Rc<RefCell<FixtureLog>>) -> Self {
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![row("alpha"), row("beta"), row("gamma")]);
        let mut workspace_carrier = MediaListCarrier::new();
        workspace_carrier.set_content(vec![row("track one"), row("track two")]);
        Self {
            carrier,
            workspace_carrier,
            log,
            workspace: false,
            workspace_focused: false,
        }
    }

    fn enable_workspace(&mut self) {
        self.workspace = true;
        self.workspace_carrier.select_index(1);
    }

    fn workspace_state(&self) -> (usize, usize, Option<String>) {
        (
            self.workspace_carrier.cursor(),
            self.workspace_carrier.scroll(),
            self.workspace_carrier.selected_target().cloned(),
        )
    }

    fn select_multiple_for_test(&mut self) {
        self.carrier.toggle_selection(&"alpha".to_string());
        self.carrier.toggle_selection(&"beta".to_string());
    }

    fn selected_targets(&self) -> Vec<String> {
        self.carrier.multi_selection().to_vec()
    }
}

fn row(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

impl LibraryContentOwner for FixtureOwner {
    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into(), "New".into()],
                markers: vec![],
                active: Some(0),
            }),
            list: ListSlot::Media(&mut self.carrier),
            hero: Some(HeroContent {
                facts: HeroFacts {
                    title: "Dune".into(),
                    meta_rows: vec!["2021".into()],
                    duration_row: None,
                    progress_row: None,
                    links: Vec::new(),
                    artwork: HeroArtwork {
                        shape: ArtworkShape::Landscape,
                        source: None,
                        decoration: None,
                        image: crate::app::components::library_panel::content::HeroImageState::None,
                    },
                },
                overview: None,
                credits: None,
                workspace: self.workspace.then_some(Workspace {
                    header: Some(WorkspaceHeader::Tracklist),
                    selector: None,
                    list: &mut self.workspace_carrier,
                    focused: self.workspace_focused,
                }),
            }),
        }
    }

    fn focus_hero_workspace(&mut self) -> bool {
        self.workspace_focused = self.workspace;
        self.workspace_focused
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        let selection = match event {
            LibrarySlotEvent::List(input) => {
                // The owner's typed translation "as today": resolve the
                // pointer target through its own carrier, then delegate.
                let target = match input {
                    MediaListSurfaceInput::Click(at)
                    | MediaListSurfaceInput::DoubleClick(at)
                    | MediaListSurfaceInput::ContextClick(at) => {
                        self.carrier.resolve_current_point(at).cloned()
                    }
                    _ => None,
                };
                if let Some(target) = &target {
                    self.carrier.select_target(target);
                }
                if let Some(operation) = input.into_operation(target) {
                    self.carrier.delegate_operation(operation);
                }
                self.carrier.selected_target().cloned()
            }
            _ => self.carrier.selected_target().cloned(),
        };
        let mut log = self.log.borrow_mut();
        log.events.push(event);
        log.selections.push(selection);
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn home_key() -> LibraryKey {
    LibraryKey::Home
}

fn movies_key() -> LibraryKey {
    LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-movies".into(),
        kind: LibraryKind::Movies,
    }
}

/// A Home tab whose owner has migrated: the fixture owner is pushed into the
/// mounted panel and one sync pass routes focus/eligibility to
/// `ComponentId::Library`. The base app's one Emby library is grouped Music
/// — still mounted as its old destination (task 8), so the fixture's
/// un-migrated branch is a real un-migrated library after task 6.1 moved
/// Movies into the panel.
fn migrated_home() -> (TickHarness, Rc<RefCell<FixtureLog>>) {
    let mut app = crate::app::render::make_music_group_app();
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.tab = crate::app::TabSelection::Home;
    let mut harness = TickHarness::new(app);
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    harness.model_mut().sync_library_panel();
    harness
        .model_mut()
        .push_library_owner(home_key(), Box::new(FixtureOwner::new(log.clone())));
    harness.model_mut().sync_mounted_surfaces();
    (harness, log)
}

fn migrated_home_with_workspace() -> (TickHarness, Rc<RefCell<FixtureLog>>) {
    let (mut harness, log) = migrated_home();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .enable_workspace();
    harness.model_mut().sync_mounted_surfaces();
    (harness, log)
}

fn draw_frame(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

/// Draw at the model's own terminal width — the breakpoint-transition draw.
fn draw_frame_sized(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let width = harness.model().app.terminal_width;
    let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

/// Draw at the model's own terminal width and height — for overlay cases
/// whose Workspace box needs more rows than the default 30-row backend
/// leaves the Library pane.
fn draw_frame_at_model_size(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

fn find_text(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
    for row in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf[(x, row)].symbol());
        }
        if let Some(offset) = line.find(needle) {
            return Some((offset as u16, row));
        }
    }
    None
}

/// The first painted occurrence of `needle` inside `area`'s rows, for
/// locating a browser row that another painted surface (a Wide Hero pane's
/// title) also shows.
fn find_text_in(
    buf: &ratatui::buffer::Buffer,
    needle: &str,
    area: ratatui::layout::Rect,
) -> Option<(u16, u16)> {
    for row in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            line.push_str(buf[(x, row)].symbol());
        }
        if let Some(offset) = line.find(needle) {
            return Some((area.left() + offset as u16, row));
        }
    }
    None
}

/// A left double-click at `(x, y)`: two press+step pairs, the gesture the
/// list slot's activation seam reads.
fn double_click(harness: &mut TickHarness, x: u16, y: u16) {
    for _ in 0..2 {
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }));
        let _ = harness.step();
    }
}

fn panel_of(harness: &TickHarness) -> Option<&crate::app::components::library_panel::LibraryPanel> {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
}

/// A Wide→Narrow resize drops the stale Wide geometry (ADR 0024): the old
/// gutter no longer arms the split drag, and a click inside the freshly
/// painted narrow list — outside the stale Wide list rect — still reaches the
/// active owner instead of being silently dropped.

// ── Library Hero overlay Workspace focus (bug-fix unit) ────────────────
//
// The overlay is the only non-Wide surface that focuses a Workspace: on
// open the destination's Workspace list must receive the destination's
// local keys (and the pointer's row resolution), keep them across ordinary
// refresh and Queue focus/return, and never let the covered browser list
// move. Exercised through the real TV and Music owners (their key routing
// is where the narrow fall-through lived), not the fixture owner, whose
// `on_key` is a no-op.

/// A narrow-geometry TV tab whose selected Series' season detail is cached:
/// the mounted panel hosts the real `TvContent` owner with a paintable
/// Workspace (`episode_count` episodes), tall enough for the overlay's
/// Workspace box.
fn migrated_tv_with_detail(episode_count: usize) -> TickHarness {
    let mut app = crate::app::render::make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.image_tags.thumb = "tag".into();
    }
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 80;
    app.terminal_height = 60;
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let episodes: Vec<_> = (1..=episode_count)
        .map(|index| {
            let mut episode = crate::app::tests::make_item(&format!("Episode {index}"), "Episode");
            episode.id = format!("episode-{index}");
            episode
        })
        .collect();
    app.series_detail_cache.insert(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), episodes)].into_iter().collect(),
        },
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_library_panel();
    harness.model_mut().sync_mounted_surfaces();
    harness
}

fn tv_owner_of(harness: &TickHarness) -> &crate::app::components::tv_content::TvContent {
    harness
        .model()
        .library_owner::<crate::app::components::tv_content::TvContent>(
            &harness.model().test_tv_owner_key(),
        )
        .expect("tv owner installed")
}

#[cfg(test)]
mod destination_selection;
#[cfg(test)]
mod hero;
#[cfg(test)]
mod panel_interaction;
#[cfg(test)]
mod panel_state;
#[cfg(test)]
mod workspace_geometry;
#[cfg(test)]
mod workspace_input;
