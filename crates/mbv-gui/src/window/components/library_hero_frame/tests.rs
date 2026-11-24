//! Headless tests of the Hero frame (design D10): the one band order every
//! destination's Hero paints through. The frame's kicker, title, and artwork
//! paint top to bottom as one look, whatever destination supplies them; the
//! per-destination Hero suites keep their own facts, actions, and rows. The
//! art painter's contexts travel in as the root's, so no test opens a window,
//! sleeps, or reaches the core.

use std::sync::Arc;

use blitz_dom::{Document as _, NodeData};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{HeroBlock, HeroHeader, LibraryHeroFrame, LibraryHeroFrameProps};
use crate::bridge::frame::library::GuiArtShapeFrame;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::library::hero::label_segments;

/// The frame's props: a destination's attributes over the two bands' text,
/// with no art carried.
fn props() -> LibraryHeroFrameProps {
    LibraryHeroFrameProps {
        attr: "abs",
        root: "data-library-abs-hero",
        header: HeroHeader {
            kicker: label_segments("audiobook"),
            title: "The Long Road Home".to_owned(),
            facts: Vec::new(),
        },
        header_action: None,
        block: HeroBlock {
            label: "The Long Road Home".to_owned(),
            eyebrow: None,
            name: None,
            name_episode: false,
            facts: None,
            meta: Vec::new(),
            overview: None,
        },
        art_shape: GuiArtShapeFrame::Square,
        art: None,
        art_title: "The Long Road Home".to_owned(),
        art_placeholder: None,
        actions: None,
        children: None,
    }
}

/// The root: provides the art painter's contexts and paints the frame, as
/// every destination's Hero composes it.
fn frame_app(props: LibraryHeroFrameProps) -> Element {
    provide_context(use_hook(ArtEvents::new));
    provide_context(use_hook(FiredTimers::new));
    provide_context(Arc::new(RecordingClock::new()) as Arc<dyn Clock>);
    provide_context(ScaleFactor(Signal::new(1.0)));
    rsx! {
        LibraryHeroFrame { ..props }
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

    /// The tag of the first node the selector names.
    fn tag_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        match &dom.get_node(id).expect("the node is in the document").data {
            NodeData::Element(data) => data.name.local.to_string(),
            _ => panic!("{selector} is an element"),
        }
    }
}

/// Build the document over the frame's props and let the first render land.
fn built() -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(frame_app, props()),
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

/// Contract (the frame's one look, `LibraryHeroFrame.test.ts`): the frame
/// paints its header band above the artwork — the kicker over the title, both
/// before the art box.
#[test]
fn the_frame_paints_its_header_band_above_the_artwork() {
    let fixture = built();

    assert_eq!(
        fixture.text_of("[data-library-abs-hero-kicker]"),
        "audiobook",
        "the kicker is the header band's label"
    );
    assert_eq!(
        fixture.tag_of("[data-library-abs-hero-title]"),
        "h1",
        "the title paints in the header band's h1"
    );
    let text = fixture.text_of("[data-library-abs-hero]");
    let header = text.find("audiobook").expect("the header band paints");
    let art = text.find("[No art").expect("the art box paints");
    assert!(header < art, "the header band paints above the artwork");
}
