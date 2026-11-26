//! Headless tests of the Home Hero (D16): the contracts of
//! `LibraryHomeHero.test.ts` — the Hero paints the title, the fact pills and
//! rows, and the overview, and no list band; the header takes the parent
//! as its title; the artwork box reserves the arm the frame declares; and the
//! actions send
//! the Home commands for the frame's selected row. The IPC of the
//! the tests is the mock core link here; the asserted commands are the
//! same neutral shapes. A headless document cannot fire a DOM click, so the
//! press contracts ride the handler function the component's own handlers
//! call one line each. The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryHomeHero;
use super::press;
use crate::art::{GuiArtRef, art_url};
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiHeroFactLabelFrame, GuiHeroFactRowFrame, GuiHeroFactsFrame,
    GuiHeroPillFrame, GuiHomeDetail, GuiHomeHeroFrame, GuiHomeRowIdFrame, GuiLibraryDetail,
    GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::FiredTimers;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The selected row's identity: the Hero's actions name it.
fn selected() -> GuiHomeRowIdFrame {
    GuiHomeRowIdFrame::Continue("e1".to_owned())
}

/// The selected row's Hero facts: two meta rows, with the duration and the
/// resume roles marked, and the overview text (design D12).
fn hero(art_shape: GuiArtShapeFrame) -> GuiHomeHeroFrame {
    GuiHomeHeroFrame {
        title: "Better Call Saul".to_owned(),
        parent: None,
        facts: GuiHeroFactsFrame {
            pills: vec![GuiHeroPillFrame {
                segments: vec![GuiMetaSegmentFrame {
                    text: "2015".to_owned(),
                    kind: GuiMetaKindFrame::Date,
                }],
                link: None,
            }],
            rows: vec![GuiHeroFactRowFrame {
                label: GuiHeroFactLabelFrame::Released,
                value: vec![GuiMetaSegmentFrame {
                    text: "February 8, 2015".to_owned(),
                    kind: GuiMetaKindFrame::Date,
                }],
            }],
        },
        overview: Some("Jimmy turns to paying work.".to_owned()),
        art_shape,
        art: None,
        cast: Vec::new(),
    }
}

/// The Home detail part with the hero picked.
fn home(facts: GuiHomeHeroFrame) -> GuiHomeDetail {
    GuiHomeDetail {
        selected: Some(selected()),
        hero: Some(facts),
    }
}

/// The harness's props: the carried Home content, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    detail: SectionRef<GuiLibraryDetail>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and paints the Home Hero, as the
/// panel composes it.
fn home_hero_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        detail,
        handle,
    } = props;
    // The art painter's contexts (the art painter consumes them), over the
    // clock the store holds. The stores hook once, as the art painter's own
    // harness holds them.
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    provide_context(events);
    provide_context(fired);
    provide_context(ScaleFactor(Signal::new(1.0)));
    provide_context(Arc::clone(&clock));
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
    provide_context(state.clone());
    // The handle's copy is the test's read side; the props carry their own.
    handle.borrow_mut().replace((state, detail.clone()));
    rsx! {
        LibraryHomeHero { detail }
    }
}

/// The context handles the root hands the test: the Library store and the
/// Home detail the presses read.
type FixtureHandles = (LibraryState, SectionRef<GuiLibraryDetail>);

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    detail: SectionRef<GuiLibraryDetail>,
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
    /// their subscribers, and let the re-renders land.
    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
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

    /// The style attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(LocalName::from("style"))
            .unwrap_or_default()
            .to_owned()
    }

    /// The Home commands the core link received, in send order.
    fn library_commands(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the carried content and let the first render and
/// effects land.
fn built(facts: GuiHomeHeroFrame) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            home_hero_app,
            FixtureProps {
                sender,
                detail: SectionRef::new(GuiLibraryDetail::Home(home(facts))),
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let (state, detail) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        detail,
        receiver,
    };
    fixture.poll_until_idle();
    fixture
}

/// Contract (`LibraryHomeHero.test.ts`): the Hero paints the title, the fact
/// pills and labelled rows, and the overview, and no list band.
#[test]
fn the_hero_paints_the_title_the_facts_and_the_overview_and_no_list_band() {
    let fixture = built(hero(GuiArtShapeFrame::Landscape));

    assert_eq!(
        fixture.text_of("[data-library-home-hero-title]"),
        "Better Call Saul",
        "the title paints"
    );
    assert!(
        fixture.presents("[data-library-home-hero-pill]"),
        "the fact pills paint"
    );
    assert!(
        fixture.presents("[data-library-home-hero-row]"),
        "the labelled fact rows paint"
    );
    assert_eq!(
        fixture.text_of("p[data-library-home-hero-overview]"),
        "Jimmy turns to paying work.",
        "the overview paints"
    );
    assert!(
        !fixture.presents("[data-library-home-hero-list]"),
        "no list band paints"
    );
}

/// Contract (the flat episode scenario, gui-hero-design-polish spec): an
/// episode whose payload names its series titles the header with the series,
/// and the episode paints in the block's name line.
#[test]
fn a_flat_episode_titles_the_header_with_its_series_and_names_the_episode() {
    let mut frame = hero(GuiArtShapeFrame::Landscape);
    frame.title = "Ozymandias".to_owned();
    frame.parent = Some("Breaking Bad".to_owned());
    let fixture = built(frame);

    assert_eq!(
        fixture.text_of("[data-library-home-hero-title]"),
        "Breaking Bad",
        "the series titles the header"
    );
    assert_eq!(
        fixture.text_of("[data-library-home-hero-block-name]"),
        "Ozymandias",
        "the episode paints in the block's name line"
    );
}

/// Contract (the Home feed-entry scenario, gui-hero-design-polish spec): a
/// feed entry whose payload names its feed titles the header with the feed,
/// and the entry paints in the block's name line.
#[test]
fn a_feed_entry_titles_the_header_with_its_feed_and_names_the_entry() {
    let mut frame = hero(GuiArtShapeFrame::Landscape);
    frame.title = "Entry title".to_owned();
    frame.parent = Some("Example Feed".to_owned());
    let fixture = built(frame);

    assert_eq!(
        fixture.text_of("[data-library-home-hero-title]"),
        "Example Feed",
        "the feed titles the header"
    );
    assert_eq!(
        fixture.text_of("[data-library-home-hero-block-name]"),
        "Entry title",
        "the entry paints in the block's name line"
    );
}

/// Contract (design D4): the Hero's card art makes its glow, and no art paints
/// none.
#[test]
fn the_hero_s_card_art_makes_its_glow() {
    let mut frame = hero(GuiArtShapeFrame::Landscape);
    frame.art = Some(GuiArtRef::Image {
        src: art_url("card/backdrop"),
    });
    let fixture = built(frame);

    assert!(
        fixture.presents("[data-library-home-hero-glow]"),
        "the glow paints from the card art"
    );
}

/// Contract (`LibraryHomeHero.test.ts`, design D15): the artwork box reserves
/// the arm the frame declares — a square or portrait cover keeps the Music
/// Hero's 300 px box at the aspect's height.
#[test]
fn a_square_or_portrait_hero_places_its_artwork_box_at_300_px_wide() {
    let square = built(hero(GuiArtShapeFrame::Square));
    let style = square.style_of("[data-library-home-hero-art] > div > div");
    assert!(
        style.contains("width: 300px") && style.contains("height: 300px"),
        "the square box holds 300 px on a side, painted: {style}"
    );

    let portrait = built(hero(GuiArtShapeFrame::Portrait));
    let style = portrait.style_of("[data-library-home-hero-art] > div > div");
    assert!(
        style.contains("width: 300px") && style.contains("height: 450px"),
        "the portrait box holds 300 px by 450 px, painted: {style}"
    );
}

/// Contract (`LibraryHomeHero.test.ts`, design D15): a landscape still fills
/// the Hero content width at 16:9 instead of the 300 px box, as the TUI's
/// hero does.
#[test]
fn a_landscape_hero_fills_the_content_width_at_16_9_instead_of_the_300_px_box() {
    let fixture = built(hero(GuiArtShapeFrame::Landscape));

    let style = fixture.style_of("[data-library-home-hero-art] > div > div");
    assert!(
        style.contains("width: 100%") && style.contains("aspect-ratio: 16 / 9"),
        "the landscape still fills the content width at 16:9, painted: {style}"
    );
    assert!(
        !style.contains("height: 300px") && !style.contains("height: 450px"),
        "the landscape still keeps no square box's height, painted: {style}"
    );
}

/// Contract (`LibraryHomeHero.test.ts`): Play sends the play command for the
/// selected row. The press that sends it is the handler function the
/// component's own handler calls one line each; the press contract rides it
/// (a headless document cannot fire a DOM click).
#[test]
fn play_sends_the_play_command_for_the_selected_row() {
    let mut fixture = built(hero(GuiArtShapeFrame::Landscape));

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || press(&state, &detail, LibraryState::play_home_row));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::HomePlay {
            row: crate::gui_core::GuiHomeRowId::Continue("e1".to_owned()),
        }],
        "the play command names the frame's selected row"
    );
}

/// Contract (`LibraryHomeHero.test.ts`): Add to queue sends the enqueue
/// command for the selected row. The press contract rides the handler
/// function (a headless document cannot fire a DOM click).
#[test]
fn add_to_queue_sends_the_enqueue_command_for_the_selected_row() {
    let mut fixture = built(hero(GuiArtShapeFrame::Landscape));

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || press(&state, &detail, LibraryState::queue_home_row));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::HomeQueue {
            row: crate::gui_core::GuiHomeRowId::Continue("e1".to_owned()),
        }],
        "the enqueue command names the frame's selected row"
    );
}
