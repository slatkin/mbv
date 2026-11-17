//! Headless tests of the Shows mode pills (D16; gui-collection-dropdown design
//! D8): the GUI Shows destination paints the series list alone, so the core
//! carries no pill and the mounted row paints none. The row itself goes away
//! with the collection-dropdown units. The core channel is a mock pair; no
//! test opens a window, sleeps, or reaches the core.

use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryShowsPills;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The harness's props: the mock core sender.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
}

/// The root: provides the Library store and paints the row, as the panel
/// composes them.
fn pills_app(props: FixtureProps) -> Element {
    let FixtureProps { sender } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let state = use_hook(move || {
        // The playhead position the interaction state reads: none — the
        // tests hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(sender),
            playhead_position,
            Arc::clone(&clock),
        )
    });
    provide_context(state);
    rsx! {
        LibraryShowsPills {}
    }
}

struct Fixture {
    doc: DioxusDocument,
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

    /// The painted pills' (key, pressed) pairs, in paint order.
    fn pressed(&self) -> Vec<(String, String)> {
        let dom = self.doc.inner();
        dom.query_selector_all("[data-library-pill]")
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                let node = dom.get_node(id).expect("the node is in the document");
                let key = node
                    .attr(LocalName::from("data-library-pill"))
                    .map(str::to_owned)?;
                let pressed = node
                    .attr(LocalName::from("aria-pressed"))
                    .map(str::to_owned)?;
                Some((key, pressed))
            })
            .collect()
    }
}

/// Build the document and let the first render and effects land.
fn built() -> Fixture {
    let (sender, _receiver) = mpsc::channel::<CoreInput>();
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(pills_app, FixtureProps { sender }),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let mut fixture = Fixture { doc };
    fixture.poll_until_idle();
    fixture
}

/// The GUI Shows destination carries no collections, so the mounted row paints
/// none (gui-collection-dropdown design D6, D8): the list paints the series
/// tree and no selector.
#[test]
fn no_shows_pill_paints() {
    let fixture = built();

    assert_eq!(fixture.pressed(), []);
}
