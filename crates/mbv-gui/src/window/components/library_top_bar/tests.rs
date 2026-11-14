//! Headless tests of the Library top bar (D16): the contracts of
//! `LibraryTopBar.test.ts` the window's port keeps — the tab bar's paint and
//! current mark, the open-destination and Settings presses' commands, the
//! Search dropdown's paint states, the highlight's id survival over a
//! rerendered frame, and the mount and focus facts' lifecycle. The debounce
//! and the activate command's own contracts are
//! `window/library/search/tests.rs` (task 6.8): the component's typing and
//! row-press handlers call those state methods one line each, and a headless
//! document cannot fire a DOM input or click event, so those two press
//! contracts ride the state layer's tests. The core channel is a mock pair;
//! no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryTopBar, open_destination, open_settings};
use crate::bridge::frame::library::{GuiLibraryTabFrame, GuiSearchFrame, GuiSearchRowFrame};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The context handles the root hands the test: the Library store, the
/// conditional-mount toggle the unmount test drives, and the layout epoch
/// the re-measures read.
type FixtureHandles = (LibraryState, Signal<bool>, LayoutEpoch);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the bar tree
/// sizes its boxes with (the same rules the player panel's harness uses), so
/// the `Truncate` measures read laid-out widths.
const FIXTURE_CSS: &str = concat!(
    "[data-slot] { width: 100%; }",
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".justify-center { justify-content: center; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".shrink-0 { flex-shrink: 0; }",
    ".w-full { width: 100%; }",
    ".h-full { height: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
);

/// The harness's props: the top bar's own props plus the mock core sender
/// and the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    tabs: Vec<GuiLibraryTabFrame>,
    current_tab: Option<usize>,
    home_active: bool,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store over the mock core link and paints
/// the top bar with the section's carried Search facts, as the Library panel
/// composes it.
fn top_bar_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        tabs,
        current_tab,
        home_active,
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
    let show = use_signal(|| true);
    handle.borrow_mut().replace((state.clone(), show, epoch));
    let search = state.sections.search.cloned();
    if show.cloned() {
        rsx! {
            LibraryTopBar {
                tabs,
                current_tab,
                search,
                home_active,
            }
        }
    } else {
        rsx! {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    epoch: LayoutEpoch,
    show: Signal<bool>,
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
    /// would, so the `Truncate` measures read laid-out widths, and let the
    /// re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Carry a Search frame on the section, as the core's batch would, and
    /// resolve the dropdown's laid-out widths.
    fn set_search_frame(&mut self, frame: Option<GuiSearchFrame>) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || sections.search.set(frame));
        self.lay_out();
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
    fn attrs_of(&self, selector: &str, attribute: blitz_dom::LocalName) -> Vec<Option<String>> {
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
fn built(tabs: Vec<GuiLibraryTabFrame>, current_tab: Option<usize>, home_active: bool) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            top_bar_app,
            FixtureProps {
                sender,
                tabs,
                current_tab,
                home_active,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let (state, show, epoch) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        epoch,
        show,
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

/// One found row, as the frame carries it.
fn row(id: &str, title: &str, subtitle: &str) -> GuiSearchRowFrame {
    GuiSearchRowFrame {
        id: id.to_owned(),
        item_type: if id == "r2" { "Album" } else { "Artist" }.to_owned(),
        title: title.to_owned(),
        subtitle: subtitle.to_owned(),
    }
}

/// Two found rows: the dropdown's rows in the list-row look. Each call
/// builds fresh row values that share only ids: the highlight must never
/// depend on object identity.
fn results() -> GuiSearchFrame {
    GuiSearchFrame {
        query: "ba".to_owned(),
        loading: false,
        failed: false,
        results: vec![
            row("r1", "Beastie Boys", "Artist"),
            row("r2", "Licensed To Ill", "Beastie Boys"),
        ],
    }
}

/// Contract (`LibraryTopBar.test.ts`): the bar paints one tab per presented
/// destination and marks the current one with `aria-current`.
#[test]
fn paints_one_tab_per_presented_destination_and_marks_the_current_one() {
    let fixture = built(two_tabs(), Some(2), false);

    let labels: Vec<String> = fixture
        .attrs_of(
            "[data-library-tab]",
            blitz_dom::LocalName::from("data-library-tab"),
        )
        .into_iter()
        .map(|position| position.expect("the attribute paints"))
        .collect();
    assert_eq!(labels, ["1", "2"]);
    assert_eq!(fixture.text_of("[data-library-tab]"), "Music");
    assert_eq!(
        fixture.attrs_of("[data-library-tab]", local_name!("aria-current")),
        [None, Some("page".to_owned())]
    );
}

/// Contract (`LibraryTopBar.test.ts`): the bar paints the dim canvas fill,
/// the TUI BgDim slot.
#[test]
fn the_bar_paints_the_dim_canvas_fill() {
    let fixture = built(two_tabs(), Some(1), false);

    assert!(
        fixture
            .style_of("[data-library-region=\"topBar\"]")
            .contains("background: var(--canvas-dim)"),
        "the bar paints the dim fill"
    );
}

/// Contract (`LibraryTopBar.test.ts`): a press on a tab opens the
/// destination at its position — the open-destination command carries the
/// tab's own position.
#[test]
fn a_press_opens_the_destination_tab_at_its_position() {
    let mut fixture = built(two_tabs(), Some(1), false);

    let state = fixture.state.clone();
    fixture.in_runtime(move || open_destination(&state, 3));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::OpenDestination { position: 3 }]
    );
}

/// Contract (`LibraryTopBar.test.ts`): a press on the Home icon opens Home's
/// own position.
#[test]
fn a_press_on_the_home_icon_opens_home() {
    let mut fixture = built(two_tabs(), Some(1), false);

    let state = fixture.state.clone();
    fixture.in_runtime(move || open_destination(&state, 0));

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
    let fixture = built(two_tabs(), None, true);

    assert_eq!(
        fixture.attrs_of("[data-library-home]", local_name!("aria-current")),
        [Some("page".to_owned())]
    );
    assert_eq!(
        fixture.attrs_of("[data-library-tab]", local_name!("aria-current")),
        [None, None]
    );
}

/// Contract (`LibraryTopBar.test.ts`): a closed Search section paints the
/// field and no dropdown.
#[test]
fn a_closed_search_section_paints_the_field_and_no_dropdown() {
    let fixture = built(two_tabs(), Some(1), false);

    assert!(
        fixture.presents("[data-library-search-input]"),
        "the field paints"
    );
    assert!(
        !fixture.presents("[data-library-search-results]"),
        "no dropdown paints"
    );
}

/// Contract (`LibraryTopBar.test.ts`): the dropdown paints the rows in the
/// list-row look, and a first landing highlights the first row.
#[test]
fn the_dropdown_paints_the_rows_with_the_first_row_highlighted() {
    let mut fixture = built(two_tabs(), Some(1), false);
    fixture.set_search_frame(Some(results()));

    // The row paints its title and its subtitle; the Svelte markup's
    // inter-element space is formatting, so the joined text asserts both
    // parts.
    let first = fixture.text_of("[data-library-search-row=\"r1\"]");
    assert!(first.contains("Beastie Boys"), "{first}");
    assert!(first.contains("Artist"), "{first}");
    let second = fixture.text_of("[data-library-search-row=\"r2\"]");
    assert!(second.contains("Licensed To Ill"), "{second}");
    assert!(second.contains("Beastie Boys"), "{second}");
    assert_eq!(
        fixture.state.search.peek().highlight.clone(),
        Some("r1".to_owned())
    );
    assert_eq!(
        fixture.attrs_of(
            "[data-library-search-highlight]",
            blitz_dom::LocalName::from("data-library-search-row")
        ),
        [Some("r1".to_owned())]
    );
}

/// Contract (`LibraryTopBar.test.ts`): the highlight survives a rerendered
/// results frame by row id, not object identity.
#[test]
fn the_highlight_survives_a_rerendered_results_frame_by_row_id() {
    let mut fixture = built(two_tabs(), Some(1), false);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.set_search_highlight(Some("r2".to_owned())));
    // A new frame: fresh row values, same ids, same highlight.
    fixture.set_search_frame(Some(results()));

    assert_eq!(
        fixture.state.search.peek().highlight.clone(),
        Some("r2".to_owned())
    );
    assert_eq!(
        fixture.attrs_of(
            "[data-library-search-highlight]",
            blitz_dom::LocalName::from("data-library-search-row")
        ),
        [Some("r2".to_owned())]
    );
}

/// Contract (`LibraryTopBar.test.ts`): the highlight falls to the first row
/// when a new frame drops its id, and to none when the frame carries no
/// rows.
#[test]
fn the_highlight_falls_to_the_first_row_when_a_new_frame_drops_its_id() {
    let mut fixture = built(two_tabs(), Some(1), false);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.set_search_highlight(Some("r2".to_owned())));
    let dropped = GuiSearchFrame {
        results: vec![row("r1", "Beastie Boys", "Artist")],
        ..results()
    };
    fixture.set_search_frame(Some(dropped.clone()));
    assert_eq!(
        fixture.state.search.peek().highlight.clone(),
        Some("r1".to_owned())
    );

    fixture.set_search_frame(Some(GuiSearchFrame {
        results: Vec::new(),
        ..dropped
    }));
    assert_eq!(fixture.state.search.peek().highlight.clone(), None);
    assert!(
        !fixture.presents("[data-library-search-highlight]"),
        "no row paints the highlight"
    );
}

/// Contract (`LibraryTopBar.test.ts`): the dropdown paints the loading and
/// failed states while the query lands.
#[test]
fn the_dropdown_paints_the_loading_and_failed_states() {
    let mut fixture = built(two_tabs(), Some(1), false);
    fixture.set_search_frame(Some(GuiSearchFrame {
        loading: true,
        ..results()
    }));
    assert!(
        fixture.presents("[data-library-search-loading]"),
        "the loading state paints"
    );

    fixture.set_search_frame(Some(GuiSearchFrame {
        loading: false,
        failed: true,
        ..results()
    }));
    assert!(
        fixture.presents("[data-library-search-failed]"),
        "the failed state paints"
    );
}

/// Contract (`LibraryTopBar.test.ts`): a press on the Settings control opens
/// the screen — the `open` command, never the keyboard's `selectSection` —
/// and the control names itself for the pointer.
#[test]
fn a_press_on_the_settings_control_opens_the_screen() {
    let mut fixture = built(two_tabs(), Some(1), false);

    assert_eq!(
        fixture
            .attrs_of("[data-library-settings]", local_name!("aria-label"))
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

/// Contract (`LibraryTopBar.test.ts`, the review-of-row-12.1 finding P1):
/// unmounting the top bar retracts the mount and focus facts, so an
/// arrangement change never leaves the keyboard armed on a field whose
/// element nothing paints.
#[test]
fn unmounting_the_top_bar_retracts_the_mount_and_focus_facts() {
    let mut fixture = built(two_tabs(), Some(1), false);
    assert!(fixture.state.search.peek().mounted);

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.focus_search_field());
    assert!(fixture.state.search.peek().focused);

    let mut show = fixture.show;
    fixture.in_runtime(move || show.set(false));

    let held = fixture.state.search.peek().clone();
    assert!(!held.mounted);
    assert!(!held.focused);
}

/// Contract (`LibraryTopBar.test.ts`): the field follows its focus fact into
/// DOM focus — the key router's `Ctrl+K` fact moves the DOM focus, and the
/// field's own blur event checks the fact back out (the blur direction is
/// the state layer's contract, `window/library/search/tests.rs`).
#[test]
fn the_field_follows_its_focus_fact_into_dom_focus() {
    let mut fixture = built(two_tabs(), Some(1), false);

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.focus_search_field());

    let dom = fixture.doc.inner();
    let focused = dom.get_focussed_node_id();
    let input = dom
        .query_selector("[data-library-search-input]")
        .expect("the selector is valid");
    assert_eq!(focused, input, "the field holds the DOM focus");
}
