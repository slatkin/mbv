use crate::library_panel::content::{HeroContent, LibraryPanelContent, ListSlot, SelectorRow};
use crate::library_panel::{LibraryContentOwner, LibraryPanel, LibrarySlotEvent};
use crate::media_list::{MediaListCarrier, MediaListSurfaceInput};
use mbv_render::components::media_list::{MediaKind, MediaListRow, MediaSemanticState};
use mbv_ui_model::library::LibraryKey;
use mbv_ui_msg::{Msg, UserEvent};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use std::cell::RefCell;
use std::rc::Rc;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyModifiers, MouseEvent, MouseEventKind};

mod mini_view;
mod overlay;
mod split_drag;
mod surface;

fn item(target: &str) -> MediaListRow<String> {
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

/// What a fixture owner did: every slot event and every post-event list
/// selection, shared with the test through an `Rc` (the panel owns the
/// owner, so the test reads the log, not the component).
#[derive(Default)]
struct FixtureLog {
    events: Vec<LibrarySlotEvent>,
    selections: Vec<Option<String>>,
}

/// A test-owned content owner: the minimal migrated owner the panel can
/// host — a carrier-backed list slot plus a policy-shaped hero. It
/// translates slot events exactly as a destination will ("as today":
/// typed point resolution through its own carrier), proving the
/// infrastructure without converting any destination.
struct FixtureOwner {
    carrier: MediaListCarrier<String>,
    log: Rc<RefCell<FixtureLog>>,
    link: bool,
    overview: Option<String>,
    /// The owner's own prose-viewport offset, moved only by its
    /// `hero_scroll` (design D7: no `ViewportAnchor` for document
    /// viewports).
    hero_scroll: usize,
}

impl FixtureOwner {
    fn new(log: Rc<RefCell<FixtureLog>>) -> Self {
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
        Self {
            carrier,
            log,
            link: false,
            overview: None,
            hero_scroll: 0,
        }
    }

    fn with_link(mut self) -> Self {
        self.link = true;
        self
    }

    /// Give the fixture's hero an overview text; with one long enough to
    /// wrap beyond the overview box's viewport, the box paints and can
    /// scroll.
    fn with_overview(mut self, overview: impl Into<String>) -> Self {
        self.overview = Some(overview.into());
        self
    }

    /// The fixture's content-preserving reset: return the list to its first
    /// row without replacing its rows.
    fn reset_presentation(&mut self) {
        self.carrier.reset_presentation();
    }
}

impl LibraryContentOwner for FixtureOwner {
    fn hero_scroll_offset(&self) -> usize {
        self.hero_scroll
    }

    // The owner owns its own offset; the panel only delivers the recognized
    // delta and the box's own clamp (same shape as the real destinations).
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

    fn reset_presentation(&mut self) {
        self.reset_presentation();
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
                facts: crate::library_panel::HeroFacts {
                    title: "Dune".into(),
                    meta_rows: if self.link {
                        vec!["2021".into(), "IMDb".into()]
                    } else {
                        vec!["2021".into()]
                    },
                    links: if self.link {
                        vec![crate::library_panel::HeroLink {
                            name: "IMDb".into(),
                            url: "https://imdb.test/dune".into(),
                        }]
                    } else {
                        Vec::new()
                    },
                    duration_row: None,
                    progress_row: None,
                    artwork: crate::library_panel::HeroArtwork {
                        shape: crate::library_panel::ArtworkShape::Landscape,
                        source: None,
                        decoration: None,
                        image: mbv_render::components::tv_wide::HeroImageState::None,
                    },
                },
                overview: self.overview.clone(),
                overview_title: None,
                credits: None,
                workspace: None,
            }),
        }
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        let selection = match event {
            LibrarySlotEvent::List(input) => {
                // Typed translation "as today": the owner resolves the
                // pointer target through its own carrier, then delegates.
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
        // A consumed pointer gesture after a list mutation reports the
        // framework's claim marker (ADR 0024).
        Some(Msg::TerminalEvent(
            mbv_ui_msg::TerminalObserverEvent::MouseClaimed,
        ))
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn draw_panel(panel: &mut LibraryPanel) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    let area = Rect::new(0, 0, 120, 30);
    terminal.draw(|f| Component::view(panel, f, area)).unwrap();
    terminal.backend().buffer().clone()
}

fn line_text(buf: &ratatui::buffer::Buffer, row: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, row)].symbol())
        .collect::<String>()
}

/// The fixture owner installed for `key`, downcast for state reads/writes.
fn fixture_owner_mut<'a>(panel: &'a mut LibraryPanel, key: &LibraryKey) -> &'a mut FixtureOwner {
    panel
        .owner_mut(key)
        .and_then(|owner| owner.as_any_mut().downcast_mut::<FixtureOwner>())
        .expect("fixture owner installed")
}

/// The fixture owner's current list selection, for reset assertions.
fn fixture_selection(panel: &LibraryPanel, key: &LibraryKey) -> Option<String> {
    panel
        .owner(key)
        .and_then(|owner| owner.as_any().downcast_ref::<FixtureOwner>())
        .and_then(|owner| owner.carrier.selected_target().cloned())
}
