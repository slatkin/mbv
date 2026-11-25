//! The Search control's contracts (user request 2026-11-19, the port of the
//! Search field's tests, which moved with the field from the Library Status
//! bar's suite): the field and its results dropdown, the
//! mount and focus facts, the collapsed form's icon and panel, and the panel
//! mount retracting when the control stops painting the field. The mount,
//! focus, and highlight facts are the store's (`window/library/search.rs`);
//! these tests own what the control paints from them. No test opens a window,
//! sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibrarySearchControl;
use crate::bridge::frame::library::{GuiSearchFrame, GuiSearchRowFrame};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The context handles the root hands the test: the Library store, the
/// conditional-mount toggle the unmount test drives, and the collapse signal
/// the collapse tests drive — the prop the top bar's measure would rewrite.
type FixtureHandles = (LibraryState, Signal<bool>, Signal<bool>);

/// The harness's props: the control's own props, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    collapsed: bool,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store over the mock core link and paints
/// the control, as the top bar composes it. The root writes the store's
/// collapse fact to the props' value, as the top bar's measure effect does,
/// so the control's own mount effect reads the fact the bar would have
/// written.
fn control_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        collapsed: props_collapsed,
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
    let state_for_effect = state.clone();
    provide_context(state.clone());
    let show = use_signal(|| true);
    // The collapse decision the top bar's measure would rewrite: the root
    // owns the signal, so the tests that drive a collapse flip write it.
    let collapsed = use_signal(move || props_collapsed);
    handle
        .borrow_mut()
        .replace((state.clone(), show, collapsed));
    // The store's collapse fact follows the signal, as the top bar's measure
    // effect writes it from the bar's measured width.
    use_effect(move || {
        state_for_effect.set_search_collapsed(collapsed.cloned());
    });
    if show.cloned() {
        rsx! {
            LibrarySearchControl { collapsed: collapsed.cloned() }
        }
    } else {
        rsx! {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    show: Signal<bool>,
    /// The collapse decision the top bar's measure would rewrite.
    collapsed: Signal<bool>,
    /// The mock core channel's receiver, held so the link's sends land: the
    /// field's own commands are the state layer's contracts
    /// (`window/library/search/tests.rs`).
    _receiver: mpsc::Receiver<CoreInput>,
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
    /// would, so the `Truncate` measures and the dropdown's anchor read
    /// laid-out boxes.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        self.poll_until_idle();
    }

    /// Carry a Search frame on the section, as the core's batch would, and
    /// resolve the dropdown's laid-out boxes. The frame rides its one shared
    /// handle, as the batch write builds it.
    fn set_search_frame(&mut self, frame: Option<GuiSearchFrame>) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || sections.search.set(frame.map(SectionRef::new)));
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
}

/// Build the document over the control's props and let its first render and
/// effects land.
fn built(collapsed: bool) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            control_app,
            FixtureProps {
                sender,
                collapsed,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                // The utilities the control tree sizes its boxes with, as the
                // status bar's harness carried them.
                blitz_dom::UserAgentStylesheet::parse(concat!(
                    ".w-full { width: 100%; }",
                    ".flex { display: flex; }",
                    ".flex-col { flex-direction: column; }",
                    ".flex-none { flex: none; }",
                    ".items-center { align-items: center; }",
                    ".justify-center { justify-content: center; }",
                    ".min-w-0 { min-width: 0; }",
                    ".overflow-hidden { overflow: hidden; }",
                    ".relative { position: relative; }",
                    ".absolute { position: absolute; }",
                    ".z-20 { z-index: 20; }",
                )),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let (state, show, collapsed) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        show,
        collapsed,
        _receiver: receiver,
    };
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
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

/// Contract (the search field's moved contract): a closed Search section
/// paints the field and no dropdown.
#[test]
fn a_closed_search_section_paints_the_field_and_no_dropdown() {
    let doc = built(false);

    assert!(
        doc.presents("[data-library-search-input]"),
        "the field paints"
    );
    assert!(
        !doc.presents("[data-library-search-results]"),
        "no dropdown paints"
    );
}

/// Contract (user request 2026-11-25, the clear button and no key hints): the
/// field paints no `Ctrl K` hint, an empty field paints no clear button, and
/// once the field holds text the clear button paints.
#[test]
fn the_clear_button_paints_only_while_the_field_holds_text() {
    let mut fixture = built(false);
    assert!(
        !fixture.presents("[data-library-search-hint]"),
        "the field paints no key hint"
    );
    assert!(
        !fixture.presents("[data-field-clear]"),
        "an empty field paints no clear button"
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.set_search_query("ba".to_owned()));

    assert!(
        fixture.presents("[data-field-clear]"),
        "the clear button paints"
    );
}

/// Contract (the moved contract): the dropdown paints the rows in the
/// list-row look, and a first landing highlights the first row.
#[test]
fn the_dropdown_paints_the_rows_with_the_first_row_highlighted() {
    let mut fixture = built(false);
    fixture.set_search_frame(Some(results()));

    // The row paints its title and its subtitle; the former markup's
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
            &blitz_dom::LocalName::from("data-library-search-row")
        ),
        [Some("r1".to_owned())]
    );
}

/// Contract (the spec's "The Search field SHALL search all libraries, show
/// the results above the field" — the dropdown paints while the frame carries
/// rows). Where it sits relative to the field is design presentation, not a
/// contract: no test asserts geometry (user decision 2026-11-19).
#[test]
fn the_results_dropdown_paints() {
    let mut fixture = built(false);
    fixture.set_search_frame(Some(results()));

    assert!(
        fixture.presents("[data-library-search-results]"),
        "the dropdown paints"
    );
    assert!(fixture.presents("[data-library-search]"), "the field stays");
}

/// Contract (the moved contract): the highlight survives a rerendered results
/// frame by row id, not object identity.
#[test]
fn the_highlight_survives_a_rerendered_results_frame_by_row_id() {
    let mut fixture = built(false);
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
            &blitz_dom::LocalName::from("data-library-search-row")
        ),
        [Some("r2".to_owned())]
    );
}

/// Contract (the moved contract): the highlight falls to the first row when a
/// new frame drops its id, and to none when the frame carries no rows.
#[test]
fn the_highlight_falls_to_the_first_row_when_a_new_frame_drops_its_id() {
    let mut fixture = built(false);
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

/// Contract (the moved contract): the dropdown paints the loading and failed
/// states while the query lands.
#[test]
fn the_dropdown_paints_the_loading_and_failed_states() {
    let mut fixture = built(false);
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

/// Contract (the review-of-row-12.1 finding P1's moved contract): unmounting
/// the control retracts the mount and focus facts, so an arrangement change
/// never leaves the keyboard armed on a field whose element nothing paints.
#[test]
fn unmounting_the_control_retracts_the_mount_and_focus_facts() {
    let mut fixture = built(false);
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

/// Contract (the moved contract): the field follows its focus fact into DOM
/// focus — the key router's `Ctrl+K` fact moves the DOM focus, and the
/// field's own blur event checks the fact back out (the blur direction is the
/// state layer's contract, `window/library/search/tests.rs`).
#[test]
fn the_field_follows_its_focus_fact_into_dom_focus() {
    let mut fixture = built(false);

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.focus_search_field());

    let dom = fixture.doc.inner();
    let focused = dom.get_focussed_node_id();
    let input = dom
        .query_selector("[data-library-search-input]")
        .expect("the selector is valid");
    assert_eq!(focused, input, "the field holds the DOM focus");
}

/// Contract (user request 2026-11-19): the collapsed form paints the icon and
/// no field, and the field's mount fact is down — a collapsed-and-closed
/// control arms no keyboard.
#[test]
fn the_collapsed_form_paints_the_icon_and_no_field() {
    let fixture = built(true);

    assert!(
        fixture.presents("[data-library-search-control]"),
        "the icon paints"
    );
    assert!(
        !fixture.presents("[data-library-search-input]"),
        "no field paints"
    );
    assert!(
        !fixture.state.search.peek().mounted,
        "the field is unmounted"
    );
}

/// Contract (user request 2026-11-19): the icon's press opens the panel that
/// carries the field, and the field mounts; the panel's second press closes
/// it and retracts the mount fact.
#[test]
fn the_icon_press_opens_and_closes_the_panel() {
    let mut fixture = built(true);

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

/// Contract (user request 2026-11-19): the collapse back to the inline form
/// closes the panel state — the panel is not the inline field's own — and the
/// inline field mounts again.
#[test]
fn the_inline_form_back_closes_the_panel_state() {
    let mut fixture = built(true);

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.toggle_search_panel());
    assert!(fixture.presents("[data-library-search-panel]"));

    let mut collapsed = fixture.collapsed;
    fixture.in_runtime(move || collapsed.set(false));
    assert!(
        fixture.presents("[data-library-search-input]"),
        "the inline field paints"
    );
    assert!(
        !fixture.presents("[data-library-search-panel]"),
        "the panel state is not the inline field's own"
    );
    assert!(fixture.state.search.peek().mounted, "the field mounts");
}
