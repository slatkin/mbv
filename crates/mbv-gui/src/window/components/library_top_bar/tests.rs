//! Headless tests of the window's global header (D16): the contracts of
//! `LibraryTopBar.test.ts` the window's port keeps — the tab bar's paint and
//! current mark, the open-destination and Settings presses' commands, the
//! Library pointer zone the header carries (design D7 of
//! `move-playback-header-into-queue-footer`), and the destination press the
//! root reads for the Mini arrangement (design D8). The Search field and its
//! dropdown moved to the Library Status bar with their contracts
//! (`library_status_bar/tests.rs`). The core channel is a mock pair; no test
//! opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryTopBar, LibraryTopBarProps, open_destination, open_playlists, open_settings};
use crate::bridge::frame::library::GuiLibraryTabFrame;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The context handles the root hands the test: the Library store and the
/// layout epoch the re-measures read.
type FixtureHandles = (LibraryState, LayoutEpoch);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the bar tree
/// sizes its boxes with.
const FIXTURE_CSS: &str = concat!(
    ".flex { display: flex; }",
    ".items-center { align-items: center; }",
    ".justify-center { justify-content: center; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".overflow-hidden { overflow: hidden; }",
);

/// The harness's props: the bar's own props plus the mock core sender and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    props: LibraryTopBarProps,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store over the mock core link and paints
/// the header, as the window's root composes it above the panel row.
fn top_bar_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        props,
        handle,
    } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let state = use_hook(move || {
        // The playhead position the Search state reads: none — the tests
        // hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(sender),
            playhead_position,
            Arc::clone(&clock),
        )
    });
    provide_context(state.clone());
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    handle.borrow_mut().replace((state, epoch));
    rsx! {
        LibraryTopBar { ..props }
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    epoch: LayoutEpoch,
    receiver: mpsc::Receiver<CoreInput>,
}

impl Fixture {
    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Run `f` inside the virtual dom's runtime, so the signal writes mark
    /// their subscribers.
    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, and let the re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The trimmed text of the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
            .trim()
            .to_owned()
    }

    /// The attribute of every node the selector names, in paint order.
    fn attrs_of(&self, selector: &str, attribute: &blitz_dom::LocalName) -> Vec<Option<String>> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .map(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .attr(attribute.clone())
                    .map(str::to_owned)
            })
            .collect()
    }

    /// The `style` attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .expect("the node paints");
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(local_name!("style"))
            .map(str::to_owned)
            .unwrap_or_default()
    }

    /// The Library commands the core link received, in send order.
    fn library_commands(&self) -> Vec<mbv::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the bar's props and let its first render and
/// effects land.
fn built(props: LibraryTopBarProps) -> Fixture {
    built_at(1200, props)
}

/// Build the document over the bar's props at `width` logical pixels wide,
/// so a test can drive the Search control's collapse decision with the
/// viewport the bar would really measure.
fn built_at(width: u32, props: LibraryTopBarProps) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            top_bar_app,
            FixtureProps {
                sender,
                props,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(width, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let (state, epoch) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        epoch,
        receiver,
    };
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
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

/// The header's props over the two tabs, with `current` active.
fn props(
    tabs: Vec<GuiLibraryTabFrame>,
    current_tab: Option<usize>,
    home_active: bool,
) -> LibraryTopBarProps {
    LibraryTopBarProps {
        tabs,
        current_tab,
        home_active,
        playlists_active: false,
        on_destination_opened: None,
        search: None,
    }
}

/// Contract (`LibraryTopBar.test.ts`): the bar paints one tab per presented
/// destination and marks the current one with `aria-current`.
#[test]
fn paints_one_tab_per_presented_destination_and_marks_the_current_one() {
    let fixture = built(props(two_tabs(), Some(2), false));

    let labels: Vec<String> = fixture
        .attrs_of(
            "[data-library-tab]",
            &blitz_dom::LocalName::from("data-library-tab"),
        )
        .into_iter()
        .map(|position| position.expect("the attribute paints"))
        .collect();
    assert_eq!(labels, ["1", "2"]);
    assert_eq!(fixture.text_of("[data-library-tab]"), "Music");
    assert_eq!(
        fixture.attrs_of("[data-library-tab]", &local_name!("aria-current")),
        [None, Some("page".to_owned())]
    );
}

/// Contract (`LibraryTopBar.test.ts`): the bar paints the dim canvas fill,
/// the TUI `BgDim` slot.
#[test]
fn the_bar_paints_the_dim_canvas_fill() {
    let fixture = built(props(two_tabs(), Some(1), false));

    assert!(
        fixture
            .style_of("[data-library-region=\"topBar\"]")
            .contains("background: var(--canvas-dim)"),
        "the bar paints the dim fill"
    );
}

/// Contract (design D7 of `move-playback-header-into-queue-footer`): the
/// header carries the Library pointer zone, so a key pressed over it routes
/// as the Library's — the bar now sits outside the Library column, which
/// carries the zone for the panel's own boxes.
#[test]
fn the_bar_carries_the_library_pointer_zone() {
    let fixture = built(props(two_tabs(), Some(1), false));

    assert_eq!(
        fixture.attrs_of(
            "[data-library-region=\"topBar\"]",
            &blitz_dom::LocalName::from("data-pointer-zone"),
        ),
        [Some("library".to_owned())]
    );
}

/// Contract (`LibraryTopBar.test.ts`): a press on a tab opens the
/// destination at its position — the open-destination command carries the
/// tab's own position.
#[test]
fn a_press_opens_the_destination_tab_at_its_position() {
    let mut fixture = built(props(two_tabs(), Some(1), false));

    let state = fixture.state.clone();
    fixture.in_runtime(move || open_destination(&state, 3, None));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::OpenDestination { position: 3 }]
    );
}

/// Contract (`LibraryTopBar.test.ts`): a press on the Home icon opens Home's
/// own position.
#[test]
fn a_press_on_the_home_icon_opens_home() {
    let mut fixture = built(props(two_tabs(), Some(1), false));

    let state = fixture.state.clone();
    fixture.in_runtime(move || open_destination(&state, 0, None));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::OpenDestination { position: 0 }]
    );
}

/// Contract (`LibraryTopBar.test.ts`): the Home icon is current only while
/// the Home content is present — the tabs keep marking their destination.
#[test]
fn the_home_icon_is_current_only_while_the_home_content_is_present() {
    // A realistic frame (design D12): the destinations are exclusive, so the
    // frame's `currentTab` stays null while the Home content is present.
    let fixture = built(props(two_tabs(), None, true));

    assert_eq!(
        fixture.attrs_of("[data-library-home]", &local_name!("aria-current")),
        [Some("page".to_owned())]
    );
    assert_eq!(
        fixture.attrs_of("[data-library-tab]", &local_name!("aria-current")),
        [None, None]
    );
}

/// Contract (`LibraryTopBar.test.ts`): a press on the Settings control opens
/// the screen — the `open` command, never the keyboard's `selectSection` —
/// and the control names itself for the pointer.
#[test]
fn a_press_on_the_settings_control_opens_the_screen() {
    let mut fixture = built(props(two_tabs(), Some(1), false));

    assert_eq!(
        fixture
            .attrs_of("[data-library-settings]", &local_name!("aria-label"))
            .first()
            .and_then(Clone::clone)
            .as_deref(),
        Some("Settings")
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || open_settings(&state));

    let settings_commands: Vec<CoreInput> = fixture.receiver.try_iter().collect();
    assert_eq!(settings_commands.len(), 1);
    assert!(
        matches!(
            settings_commands.first(),
            Some(CoreInput::Settings(mbv::GuiSettingsCommand::Open))
        ),
        "the Settings control sends the open command"
    );
}

/// Contract (user request 2026-11-19): the Playlists control sits beside the
/// Home icon and its press sends the `playlists_open` toggle — the screen
/// opens at Playlists and closes while it shows Playlists. The route the
/// settings sidebar carried before it.
#[test]
fn a_press_on_the_playlists_control_toggles_the_screen_at_playlists() {
    let mut fixture = built(props(two_tabs(), Some(1), false));

    assert!(
        fixture.presents("[data-library-playlists]"),
        "the Playlists control paints beside Home"
    );
    assert_eq!(
        fixture
            .attrs_of("[data-library-playlists]", &local_name!("aria-label"))
            .first()
            .and_then(Clone::clone)
            .as_deref(),
        Some("Playlists")
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || open_playlists(&state));

    let commands: Vec<CoreInput> = fixture.receiver.try_iter().collect();
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(
            commands.first(),
            Some(CoreInput::Playlists(mbv::GuiPlaylistsCommand::Toggle))
        ),
        "the Playlists control sends the toggle command"
    );
}

/// Contract (user request 2026-11-19): the Search control paints the field
/// inline while the bar is wide enough, and paints no collapsed icon.
#[test]
fn a_wide_bar_paints_the_search_field_inline() {
    let fixture = built(props(two_tabs(), Some(1), false));

    assert!(
        fixture.presents("[data-library-search-input]"),
        "the field paints inline"
    );
    assert_eq!(
        fixture.text_of("[data-library-search-hint]"),
        "Ctrl K",
        "the chord's hint paints inside the field"
    );
    assert!(
        !fixture.presents("[data-library-search-control]"),
        "no collapsed icon paints"
    );
}

/// Contract (user request 2026-11-19): a bar too narrow for the inline field
/// paints the Search icon and no field; the icon's press opens the panel that
/// carries the field, and a second press closes it.
#[test]
fn a_narrow_bar_collapses_the_search_control_to_its_icon() {
    let mut fixture = built_at(400, props(two_tabs(), Some(1), false));

    assert!(
        fixture.presents("[data-library-search-control]"),
        "the icon paints"
    );
    assert!(
        !fixture.presents("[data-library-search-input]"),
        "no inline field paints"
    );
    assert!(
        !fixture.state.search.peek().mounted,
        "the field is unmounted"
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.toggle_search_panel());
    assert!(
        fixture.presents("[data-library-search-panel]"),
        "the panel paints"
    );
    assert!(
        fixture.presents("[data-library-search-input]"),
        "the panel carries the field"
    );
    assert!(fixture.state.search.peek().mounted, "the field mounts");

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.toggle_search_panel());
    assert!(
        !fixture.presents("[data-library-search-panel]"),
        "the panel closes"
    );
    assert!(!fixture.state.search.peek().mounted, "the field unmounts");
}
