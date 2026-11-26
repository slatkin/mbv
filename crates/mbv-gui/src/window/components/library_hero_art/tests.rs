//! Headless tests of the Hero's artwork box (D16): the contracts of
//! `LibraryHeroArt.test.ts` — a `loading` reference paints no img and the
//! loading fill, and the box's hooks name the art kind and the shape. The
//! two failed-load contracts (the fill paints in place of the image, and the
//! retry repaints the `img` with the same src) are the art painter's own
//! contracts (`window/components/art_image/tests.rs`), which the mounted
//! `ArtImage` holds — the box's wiring rides those tests. The store, the
//! timer router, the clock, and the scale factor travel in as the root's
//! contexts, so no test opens a window, sleeps, or reaches the core.

use std::sync::Arc;

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryHeroArt;
use crate::art::GuiArtRef;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::ArtEvents;

/// The props a test hands its document: the art to paint and the box's shape.
#[derive(Clone)]
struct FixtureProps {
    art: Option<GuiArtRef>,
    shape: crate::bridge::frame::library::GuiArtShapeFrame,
}

/// The root: provides the art painter's contexts and paints the box, as the
/// Heroes compose it.
fn art_app(props: FixtureProps) -> Element {
    let FixtureProps { art, shape } = props;
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    provide_context(events);
    provide_context(fired);
    provide_context(clock as Arc<dyn Clock>);
    let art = use_signal(move || art);
    let current = art.cloned();
    rsx! {
        LibraryHeroArt {
            shape,
            art: current,
            title: "Album",
            attr: "",
        }
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

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: &str) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(LocalName::from(attribute))
            .map(str::to_owned)
    }
}

/// Build the document over the box's art and shape and let the first render
/// land.
fn built(
    art: Option<GuiArtRef>,
    shape: crate::bridge::frame::library::GuiArtShapeFrame,
) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(art_app, FixtureProps { art, shape }),
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

/// Contract (`LibraryHeroArt.test.ts`): a `loading` reference paints no img
/// and the loading fill — it names no URL, and the box's placeholder text is
/// the fill's content.
#[test]
fn a_loading_reference_paints_no_img_and_the_loading_fill() {
    let fixture = built(
        Some(GuiArtRef::Loading),
        crate::bridge::frame::library::GuiArtShapeFrame::Square,
    );

    assert!(
        !fixture.presents("img"),
        "a loading reference paints no img"
    );
    assert_eq!(
        fixture.text_of("[data-library-hero-art]"),
        "[No art · the window's placeholder]".to_owned(),
        "the loading fill is the box's placeholder text"
    );
    assert_eq!(
        fixture.attr_of("[data-library-hero-art]", "data-library-hero-art"),
        Some("loading".to_owned()),
        "the box's art hook names the loading kind"
    );
    assert_eq!(
        fixture.attr_of("[data-library-hero-art]", "data-library-hero-art-shape"),
        Some("square".to_owned()),
        "the box's shape hook names the square arm"
    );
}
