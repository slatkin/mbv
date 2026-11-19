//! The global header's window contracts (design D7, D8 of
//! `move-playback-header-into-queue-footer`): the root mounts the top bar above
//! the panel row, across the full window width and over both columns; the
//! Settings screen shows no header; and a press on a destination tab while the
//! Mini arrangement shows the Queue makes that destination active and paints
//! the Library in the Queue column's place. The fixture root composes the
//! window's own column the way `App` does — the header above the panel row —
//! and reads every derived fact through the root's own helpers
//! ([`queue_column_shows`], [`library_column_shows`], [`header_facts`],
//! [`destination_pressed`]). The panel row's two columns are stand-ins: this
//! suite owns which of them paints, not what they paint. No test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::channel;

use blitz_dom::Document as _;
use blitz_traits::events::{
    BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, Point, PointerCoords,
    PointerDetails, UiEvent,
};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::super::{
    MiniView, ROOT_STYLE, destination_pressed, header_facts, library_column_shows,
    queue_column_shows,
};
use crate::bridge::frame::library::{
    GuiDestinationList, GuiHomeDetail, GuiHomeList, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryTabFrame,
};
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::components::library_top_bar::LibraryTopBar;
use crate::window::input::f32_from_f64;
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The viewport the fixture paints at: a Wide window.
const WIDTH: u32 = 1200;
const HEIGHT: u32 = 900;

/// The screen the fixture paints: the two panels behind the global header, or
/// the Settings screen in their place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Panels,
    Settings,
}

/// The fixture root's props: the screen, the arrangement, and the cell the
/// root hands its section store back through.
#[derive(Clone)]
struct RootProps {
    screen: Screen,
    arrangement: GuiArrangement,
    handle: Rc<RefCell<Option<Sections>>>,
}

/// The root over the window's real stylesheet: the window's one column — the
/// header above the panel row — with the Settings screen in the panel row's
/// place while its section is present.
fn root(props: RootProps) -> Element {
    let RootProps {
        screen,
        arrangement,
        handle,
    } = props;
    let (sender, _core) = channel::<CoreInput>();
    let link = CoreLink::new(sender);
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    provide_context(use_hook(LayoutEpoch::new));
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let library_sections = sections.clone();
    let library_link = CoreLink::clone(&link);
    let library = use_hook(move || {
        LibraryState::new(
            library_sections,
            library_link,
            Memo::new(|| 0.0),
            Arc::clone(&clock),
        )
    });
    provide_context(library);
    handle.borrow_mut().replace(sections.clone());

    let settings_open = screen == Screen::Settings;
    // The Mini view and the panel row's two columns (design D8), through the
    // root's own pick.
    let mini_view = use_signal(MiniView::default);
    let mini = mini_view.cloned();
    let panel_shows = true;
    let queue_visible = queue_column_shows(panel_shows, arrangement, mini);
    let library_visible = library_column_shows(panel_shows, arrangement, mini);
    // The header's facts and its press (design D7, D8).
    let header = header_facts(&sections);
    let header_current_tab = header.current_tab;
    let header_home_active = header.home_active;
    let mut opened = mini_view;
    let on_destination_opened = Callback::new(move |pressed: usize| {
        destination_pressed(
            arrangement,
            &mut opened,
            pressed,
            header_current_tab,
            header_home_active,
        );
    });

    let stylesheet = ROOT_STYLE.as_str();
    rsx! {
        style { "{stylesheet}" }
        main { class: "h-dvh w-full overflow-hidden",
            div { class: "flex h-full min-h-0 w-full min-w-0 flex-col",
                if !settings_open {
                    LibraryTopBar {
                        tabs: header.tabs.clone(),
                        current_tab: header.current_tab,
                        home_active: header.home_active,
                        on_destination_opened: Some(on_destination_opened),
                    }
                }
                div {
                    class: "relative flex min-h-0 flex-1 w-full min-w-0 overflow-hidden",
                    "data-panel-row": "",
                    if settings_open {
                        div { class: "flex flex-1", "data-settings-screen": "" }
                    } else {
                        if library_visible {
                            div { class: "flex flex-1", "data-library-column": "" }
                        }
                        if queue_visible {
                            div { class: "flex flex-1", "data-queue-column": "" }
                        }
                    }
                }
            }
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
}

impl Fixture {
    /// Build the screen's document over a `WIDTH` × `HEIGHT` viewport and the
    /// window's real stylesheet.
    fn built(screen: Screen, arrangement: GuiArrangement) -> Self {
        let handle = Rc::new(RefCell::new(None));
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                root,
                RootProps {
                    screen,
                    arrangement,
                    handle: Rc::clone(&handle),
                },
            ),
            DocumentConfig {
                viewport: Some(Viewport::new(WIDTH, HEIGHT, 1.0, ColorScheme::Light)),
                ua_stylesheets: vec![blitz_dom::UserAgentStylesheet::default_sheet()],
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        let sections = handle
            .borrow_mut()
            .take()
            .expect("the root handed its store over");
        let mut fixture = Self { doc, sections };
        fixture.settle();
        fixture
    }

    /// Poll the document until its effects have run, then resolve its style and
    /// layout, as the renderer's frame does before it paints.
    fn settle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                break;
            }
        }
        self.doc.inner.borrow_mut().resolve(0.0);
    }

    /// Write the Library list section's tabs with the first tab active, as the
    /// core's batch would.
    fn carry_tabs(&mut self, tabs: Vec<GuiLibraryTabFrame>) {
        let current_tab = tabs.first().map(|tab| tab.position);
        self.carry_tabs_at(tabs, current_tab);
    }

    /// Write the Library list section's tabs with `current_tab` active, as the
    /// core's batch carries the destination an open made active.
    fn carry_tabs_at(&mut self, tabs: Vec<GuiLibraryTabFrame>, current_tab: Option<usize>) {
        let mut list = self.sections.library_list;
        self.doc.vdom.in_runtime(move || {
            list.set(Some(SectionRef::new(
                crate::bridge::frame::library::GuiLibraryList {
                    tabs,
                    current_tab,
                    destination: crate::bridge::frame::library::GuiDestinationList::Music(
                        crate::bridge::frame::library::GuiMusicList {
                            lib_idx: 0,
                            collections: crate::bridge::frame::library::GuiCollections {
                                target: mbv::GuiCollectionTarget::Music(0),
                                all: true,
                                entries: Vec::new(),
                                noun: mbv::GuiCollectionNoun::Groups,
                                order: None,
                            },
                            loading: false,
                            failure: None,
                            artists: Vec::new(),
                        },
                    ),
                },
            )));
        });
        self.settle();
    }

    /// Write the Home destination's sections, as the core's batch would: the
    /// Home content is present, so the header's Home icon is current and no
    /// tab is.
    fn carry_home(&mut self) {
        let mut list = self.sections.library_list;
        let mut detail = self.sections.library_detail;
        self.doc.vdom.in_runtime(move || {
            list.set(Some(SectionRef::new(GuiLibraryList {
                tabs: two_tabs(),
                current_tab: None,
                destination: GuiDestinationList::Home(GuiHomeList {
                    loading: false,
                    rows: Vec::new(),
                }),
            })));
            detail.set(Some(SectionRef::new(GuiLibraryDetail::Home(
                GuiHomeDetail {
                    selected: None,
                    hero: None,
                },
            ))));
        });
        self.settle();
    }

    /// Write the Settings section's frame, as the core's batch would.
    fn carry_settings(&mut self) {
        let mut settings = self.sections.settings;
        self.doc.vdom.in_runtime(move || {
            settings.set(Some(SectionRef::new(GuiSettingsFrame {
                section: GuiSettingsSection::Services,
                sections: vec![GuiSettingsSection::Services],
                content: GuiSettingsContent::Rows { rows: Vec::new() },
            })));
        });
        self.settle();
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The laid-out client rect of the first node the selector names.
    fn rect_of(&self, selector: &str) -> blitz_dom::BoundingRect {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_client_bounding_rect(id)
            .expect("the node is laid out")
    }

    /// Press the painted control the selector names: the press lands on its
    /// centre.
    fn press(&mut self, selector: &str) {
        let rect = self.rect_of(selector);
        let (x, y) = (
            f32_from_f64(rect.x + rect.width / 2.0),
            f32_from_f64(rect.y + rect.height / 2.0),
        );
        self.click(x, y);
    }

    /// Press and release at `x`/`y`: the release dispatches the DOM `click`,
    /// so the painted control's own handler runs.
    fn click(&mut self, x: f32, y: f32) {
        self.doc.handle_ui_event(pointer(true, x, y));
        self.doc.handle_ui_event(pointer(false, x, y));
        self.settle();
    }
}

/// One pointer press or release at `x`/`y` in the document's client
/// coordinates, as the shell builds them from a winit pointer button.
fn pointer(down: bool, x: f32, y: f32) -> UiEvent {
    let coords = PointerCoords {
        page_x: x,
        page_y: y,
        screen_x: x,
        screen_y: y,
        client_x: x,
        client_y: y,
    };
    let event = BlitzPointerEvent {
        id: BlitzPointerId::Mouse,
        is_primary: true,
        coords,
        button: MouseEventButton::Main,
        buttons: MouseEventButtons::Primary,
        mods: Modifiers::default(),
        details: PointerDetails::default(),
        element: Point::default(),
        active_pointers: Arc::default(),
    };
    if down {
        UiEvent::PointerDown(event)
    } else {
        UiEvent::PointerUp(event)
    }
}

/// Two presented destination tabs, as the frame carries them.
fn two_tabs() -> Vec<GuiLibraryTabFrame> {
    vec![
        GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        },
        GuiLibraryTabFrame {
            position: 2,
            label: "Shows".to_owned(),
        },
    ]
}

/// Contract (spec "The first GUI slice presents the player panel of the design
/// canvas", the scenario "The header spans both columns"): the header paints
/// and both columns paint. How wide the header is and where the columns sit
/// under it are design presentation, not a contract: no test asserts geometry
/// (a full-window-width assert broke on a presentation-only change; user
/// decision 2026-11-19).
#[test]
fn the_header_paints_with_both_columns() {
    let mut fixture = Fixture::built(Screen::Panels, GuiArrangement::Wide);
    fixture.carry_tabs(two_tabs());

    assert!(
        fixture.presents("[data-library-region=\"topBar\"]"),
        "the header paints"
    );
    assert!(
        fixture.presents("[data-library-column]") && fixture.presents("[data-queue-column]"),
        "both columns paint"
    );
}

/// Contract (spec "The first GUI slice presents the player panel of the design
/// canvas", the scenario "The Settings screen shows"): no global header paints
/// while the Settings screen shows.
#[test]
fn the_settings_screen_paints_no_header() {
    let mut fixture = Fixture::built(Screen::Settings, GuiArrangement::Wide);
    fixture.carry_settings();

    assert!(
        fixture.presents("[data-settings-screen]"),
        "the Settings screen paints"
    );
    assert!(
        !fixture.presents("[data-library-region=\"topBar\"]"),
        "no header paints above it"
    );
}

/// Contract (spec "The top bar shows the destination tabs and Search", the
/// scenario "A tab is pressed in Mini while the Queue shows"): the press makes
/// the destination active and paints the Library in the Queue column's place.
#[test]
fn a_tab_press_in_mini_shows_the_library_in_place_of_the_queue() {
    let mut fixture = Fixture::built(Screen::Panels, GuiArrangement::Mini);
    fixture.carry_tabs(two_tabs());
    assert!(
        fixture.presents("[data-queue-column]") && !fixture.presents("[data-library-column]"),
        "Mini starts on the Queue column"
    );

    fixture.press("[data-library-tab=\"2\"]");

    assert!(
        fixture.presents("[data-library-column]"),
        "the Library paints in the Queue column's place"
    );
    assert!(
        !fixture.presents("[data-queue-column]"),
        "the Queue column gives its place up"
    );
}

/// Contract (design D8): in Mini, the active destination's own tab toggles the
/// view back to the Queue, so a destination press never traps the window on
/// the Library.
#[test]
fn the_active_destinations_tab_in_mini_returns_to_the_queue() {
    let mut fixture = Fixture::built(Screen::Panels, GuiArrangement::Mini);
    fixture.carry_tabs_at(two_tabs(), Some(2));
    // The setup press shows the Library, as the scenario above does.
    fixture.press("[data-library-tab=\"2\"]");
    assert!(
        fixture.presents("[data-library-column]"),
        "the Library shows"
    );

    fixture.press("[data-library-tab=\"2\"]");

    assert!(
        fixture.presents("[data-queue-column]"),
        "the active destination's tab puts the Queue back"
    );
    assert!(
        !fixture.presents("[data-library-column]"),
        "the Library gives its place up"
    );
}

/// Contract (design D8): in Mini, a press on a destination that is not the
/// active one keeps the Library showing, so only the active tab toggles.
#[test]
fn a_press_on_another_destination_keeps_the_library_in_mini() {
    let mut fixture = Fixture::built(Screen::Panels, GuiArrangement::Mini);
    fixture.carry_tabs_at(two_tabs(), Some(1));
    fixture.press("[data-library-tab=\"2\"]");
    // The core answers the open: Shows is the active tab now.
    fixture.carry_tabs_at(two_tabs(), Some(2));

    fixture.press("[data-library-tab=\"1\"]");

    assert!(
        fixture.presents("[data-library-column]"),
        "a different destination's tab keeps the Library"
    );
    assert!(
        !fixture.presents("[data-queue-column]"),
        "the Queue stays hidden"
    );
}

/// Contract (design D8): in Mini, the Home icon is the Queue/Library toggle
/// while the Home content is active.
#[test]
fn the_home_icon_in_mini_returns_to_the_queue_while_home_is_active() {
    let mut fixture = Fixture::built(Screen::Panels, GuiArrangement::Mini);
    fixture.carry_home();
    // The setup press shows the Home content in the Library's place.
    fixture.press("[data-library-home]");
    assert!(
        fixture.presents("[data-library-column]"),
        "the Library shows"
    );

    fixture.press("[data-library-home]");

    assert!(
        fixture.presents("[data-queue-column]"),
        "the Home icon puts the Queue back"
    );
    assert!(
        !fixture.presents("[data-library-column]"),
        "the Library gives its place up"
    );
}
