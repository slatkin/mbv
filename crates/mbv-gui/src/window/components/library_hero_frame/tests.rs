//! Headless tests of the Hero frame (design D2): the one band order every
//! destination's Hero paints through. The frame paints the header band's
//! pills in their order, the block's labelled rows in theirs, the present
//! section headers — Overview, then Cast, then the list band — in theirs, no
//! playback progress anywhere, and the backdrop only when loaded art is carried
//! (spec scenario "The Hero's artwork makes its backdrop": a Hero whose artwork
//! has loaded shows the backdrop behind the header band, and a Hero with no
//! artwork shows no backdrop). The per-destination Hero suites keep their own
//! facts, actions, and rows. The art painter's contexts travel in as the
//! root's, and the pills' store rides a mock core link, so no test opens a
//! window, sleeps, or reaches the core.

use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName, NodeData};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{HeroBlock, HeroHeader, LibraryHeroFrame, LibraryHeroFrameProps};
use crate::art::{GuiArtRef, art_url};
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiHeroCastMemberFrame, GuiHeroFactLabelFrame, GuiHeroFactRowFrame,
    GuiHeroPillFrame, GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::ArtEvents;
use crate::window::library::hero::label_segments;
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The frame's props: a destination's attributes over the two bands' text,
/// with no pills, rows, sections, or backdrop carried.
fn props() -> LibraryHeroFrameProps {
    LibraryHeroFrameProps {
        attr: "abs",
        root: "data-library-abs-hero",
        header: HeroHeader {
            kicker: label_segments("audiobook"),
            title: "The Long Road Home".to_owned(),
            pills: Vec::new(),
        },
        header_action: None,
        block: HeroBlock {
            label: "The Long Road Home".to_owned(),
            eyebrow: None,
            name: None,
            name_episode: false,
            pills: Vec::new(),
            rows: Vec::new(),
        },
        art_shape: GuiArtShapeFrame::Square,
        art: None,
        art_title: "The Long Road Home".to_owned(),
        art_placeholder: None,
        actions: None,
        overview: None,
        cast: Vec::new(),
        list_title: None,
        children: None,
        backdrop: None,
    }
}

/// One typed segment, as the pills and the row values carry their text.
fn segment(text: &str) -> GuiMetaSegmentFrame {
    GuiMetaSegmentFrame {
        text: text.to_owned(),
        kind: GuiMetaKindFrame::Plain,
    }
}

/// One fact pill: its text, no link.
fn pill(text: &str) -> GuiHeroPillFrame {
    GuiHeroPillFrame {
        segments: vec![segment(text)],
        link: None,
    }
}

/// One link pill: its text and the URL its press opens.
fn link_pill(text: &str, url: &str) -> GuiHeroPillFrame {
    GuiHeroPillFrame {
        segments: vec![segment(text)],
        link: Some(url.to_owned()),
    }
}

/// One labelled fact row: the label word its enum names, one plain value.
fn row(label: GuiHeroFactLabelFrame, value: &str) -> GuiHeroFactRowFrame {
    GuiHeroFactRowFrame {
        label,
        value: vec![segment(value)],
    }
}

/// One cast member: the name, the role, the initials, and no photo.
fn cast_member(name: &str) -> GuiHeroCastMemberFrame {
    GuiHeroCastMemberFrame {
        name: name.to_owned(),
        role: "Director of Photography".to_owned(),
        initials: "AD".to_owned(),
        photo: None,
    }
}

/// The root: provides the art painter's contexts, the pills' Library store,
/// and paints the frame, as every destination's Hero composes it.
fn frame_app(props: LibraryHeroFrameProps) -> Element {
    provide_context(use_hook(ArtEvents::new));
    provide_context(use_hook(FiredTimers::new));
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    provide_context(Arc::clone(&clock));
    // The pills row's store: the link pill's press sends through the mock
    // core link, which no test reads.
    let (sender, _receiver) = mpsc::channel::<CoreInput>();
    let link = CoreLink::new(sender);
    let state = use_hook(move || {
        // The playhead position the interaction state reads: none — the
        // tests hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(Sections::new(), link, playhead_position, clock)
    });
    provide_context(state);
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

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The attribute of every node the selector names, in paint order.
    fn all_attrs(&self, selector: &str, attribute: &str) -> Vec<String> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .attr(LocalName::from(attribute))
                    .map(str::to_owned)
            })
            .collect()
    }

    /// The texts of every node the selector names, in paint order.
    fn all_texts(&self, selector: &str) -> Vec<String> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .map(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .text_content()
                    .trim()
                    .to_owned()
            })
            .collect()
    }
}

/// Build the document over the frame's props and let the first render land.
fn built(props: LibraryHeroFrameProps) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(frame_app, props),
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
    let fixture = built(props());

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

/// Contract (design D2): the header band paints its fact pills row, one pill
/// per fact, in the order the frame carries them — a link pill among them.
#[test]
fn the_frame_paints_the_header_pills_in_their_order() {
    let mut props = props();
    props.header.pills = vec![
        pill("1986"),
        pill("Sci-Fi"),
        link_pill("IMDb", "https://imdb.example/title"),
    ];
    let fixture = built(props);

    assert!(
        fixture.presents("[data-library-abs-hero-pills]"),
        "the header band's pills row paints"
    );
    assert_eq!(
        fixture.all_texts("[data-library-abs-hero-pill]"),
        vec!["1986".to_owned(), "Sci-Fi".to_owned(), "IMDb".to_owned()],
        "the pills paint in the carried order, link pill included"
    );
}

/// Contract (design D2): the detail block paints its labelled fact rows, one
/// row per fact, in the order the frame carries them, each row painting the
/// label word its enum names beside its value.
#[test]
fn the_frame_paints_the_block_fact_rows_in_their_order() {
    let mut props = props();
    props.block.rows = vec![
        row(GuiHeroFactLabelFrame::Director, "Ada Director"),
        row(GuiHeroFactLabelFrame::Released, "08 Oct 1986"),
    ];
    let fixture = built(props);

    assert!(
        fixture.presents("[data-library-abs-hero-rows]"),
        "the block's fact rows paint"
    );
    assert_eq!(
        fixture.all_attrs("[data-library-abs-hero-row]", "data-library-abs-hero-row"),
        vec!["director".to_owned(), "released".to_owned()],
        "the rows paint in the carried order, label wire per row"
    );
    assert_eq!(
        fixture.all_texts("[data-library-abs-hero-row-label]"),
        vec!["Director".to_owned(), "Released".to_owned()],
        "each row paints the label word its enum names"
    );
    assert_eq!(
        fixture.all_texts("[data-library-abs-hero-row-value]"),
        vec!["Ada Director".to_owned(), "08 Oct 1986".to_owned()],
        "each row paints its value beside the label"
    );
}

/// Contract (design D2): every present band paints under its titled section
/// header, in the frame's one order — the Overview, then the Cast, then the
/// destination's list band.
#[test]
fn the_frame_paints_the_section_headers_in_their_order() {
    let mut props = props();
    props.overview = Some("A long journey home.".to_owned());
    props.cast = vec![cast_member("Ada Director")];
    props.list_title = Some("Chapters".to_owned());
    props.children = Some(rsx! { div { "chapter rows" } });
    let fixture = built(props);

    assert!(
        fixture.presents("[data-library-abs-hero-overview]")
            && fixture.presents("[data-library-abs-hero-cast]")
            && fixture.presents("[data-library-abs-hero-list]"),
        "every present band paints under its section"
    );
    let text = fixture.text_of("[data-library-abs-hero]");
    let overview = text.find("Overview").expect("the Overview header paints");
    let cast = text.find("Cast").expect("the Cast header paints");
    let list = text
        .find("Chapters")
        .expect("the list band's header paints");
    assert!(
        overview < cast && cast < list,
        "the Overview, the Cast, and the list band paint in that order"
    );
}

/// Contract (design D2, the spec's "No Hero SHALL show playback progress"):
/// the frame paints no progress element — the former duration and progress
/// role rows paint nowhere, and no painted text holds a progress percent.
#[test]
fn the_frame_paints_no_progress_element() {
    let mut props = props();
    props.header.pills = vec![pill("9 h 51 m")];
    props.block.rows = vec![row(GuiHeroFactLabelFrame::Released, "08 Oct")];
    let fixture = built(props);

    assert!(
        !fixture.presents("[data-library-abs-hero-role]"),
        "no band paints the former duration or progress role row"
    );
    assert!(
        !fixture.text_of("[data-library-abs-hero]").contains('%'),
        "no painted text holds a progress percent"
    );
}

/// The one case table (design D4, the spec scenario "The Hero's artwork makes
/// its backdrop"): the backdrop paints only when the frame carries art with a source
/// — none paints without the prop or with a backdrop still loading.
#[rstest]
#[case::no_backdrop_prop(None, false)]
#[case::a_backdrop_without_a_source(Some(GuiArtRef::Loading), false)]
#[case::loaded_backdrop_art(Some(GuiArtRef::Image {
    src: art_url("backdrop/cover"),
}), true)]
fn the_backdrop_paints_only_when_the_frame_carries_loaded_art(
    #[case] backdrop: Option<GuiArtRef>,
    #[case] paints: bool,
) {
    let mut props = props();
    props.backdrop = backdrop;
    let fixture = built(props);

    assert_eq!(
        fixture.presents("[data-library-abs-hero-backdrop]"),
        paints,
        "the backdrop paints exactly when loaded art is carried"
    );
}
