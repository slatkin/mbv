//! Headless tests of the Podcasts Hero (D16): the contracts of
//! `LibraryPodcastsHero.test.ts` — the artwork box takes the arm every Hero
//! shares, the show titles the header with the episode in the block, the
//! duration as a fact pill, and no Episodes band paints, and the actions send
//! the Podcasts commands with the frame's selected episode. The IPC of the
//! earlier tests is the mock core link here; the asserted
//! commands are the same neutral shapes. A headless document cannot fire a
//! DOM click, so the press contracts ride the handler functions the
//! component's own handlers call one line each. The core channel is a mock
//! pair; no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::LibraryPodcastsHero;
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiCollections, GuiDestinationList, GuiHeroFactLabelFrame,
    GuiHeroFactRowFrame, GuiHeroFactsFrame, GuiHeroPillFrame, GuiLibraryDetail, GuiLibraryList,
    GuiMetaKindFrame, GuiMetaSegmentFrame, GuiPodcastsDetail, GuiPodcastsHeroFrame,
    GuiPodcastsList,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::library::hero::hero_command;
use crate::window::library::lettered::{LetteredDestination, PodcastsList};
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The library index the Podcasts commands name.
const LIB_IDX: usize = 4;

/// One typed segment, as the pills and the row values carry their text.
fn segment(text: &str, kind: GuiMetaKindFrame) -> GuiMetaSegmentFrame {
    GuiMetaSegmentFrame {
        text: text.to_owned(),
        kind,
    }
}

/// One fact pill: its typed segments, no link.
fn pill(segments: Vec<GuiMetaSegmentFrame>) -> GuiHeroPillFrame {
    GuiHeroPillFrame {
        segments,
        link: None,
    }
}

/// One labelled fact row: the label word its enum names, one plain value.
fn row(label: GuiHeroFactLabelFrame, value: &str) -> GuiHeroFactRowFrame {
    GuiHeroFactRowFrame {
        label,
        value: vec![segment(value, GuiMetaKindFrame::Plain)],
    }
}

/// The selected episode's Hero facts: the show as the parent, the duration
/// pill, and the publish date row (design D12, row 10.1; design D1), with the
/// description the Overview section paints.
fn hero(art_shape: GuiArtShapeFrame) -> GuiPodcastsHeroFrame {
    GuiPodcastsHeroFrame {
        title: "Episode 12: The Long Road".to_owned(),
        parent: Some("Bits and Bites".to_owned()),
        facts: GuiHeroFactsFrame {
            pills: vec![pill(vec![segment("0 h 54 m", GuiMetaKindFrame::Time)])],
            rows: vec![row(GuiHeroFactLabelFrame::Published, "08 Oct")],
        },
        overview: Some("A conversation about long walks.".to_owned()),
        art_shape,
        art: None,
    }
}

/// The Podcasts detail part: the episode selected, with the Hero `facts`.
fn detail(facts: Option<GuiPodcastsHeroFrame>) -> GuiPodcastsDetail {
    GuiPodcastsDetail {
        selected: Some("show1:ep12".to_owned()),
        hero: facts,
    }
}

/// The Podcasts list part the destination's commands read the library index
/// from (design D12).
fn podcasts_library_list() -> GuiLibraryList {
    GuiLibraryList {
        refreshing: false,
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Podcasts(GuiPodcastsList {
            lib_idx: LIB_IDX,
            collections: GuiCollections {
                target: crate::gui_core::GuiCollectionTarget::Podcasts(LIB_IDX),
                all: true,
                entries: Vec::new(),
                noun: crate::gui_core::GuiCollectionNoun::Shows,
            },
            loading: false,
            failed: false,
            rows: Vec::new(),
            sort: None,
        }),
    }
}

/// The harness's props: the carried Podcasts content, the mock core sender,
/// and the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    detail: SectionRef<GuiLibraryDetail>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the Library store and the
/// Podcasts detail the presses read.
type FixtureHandles = (LibraryState, SectionRef<GuiLibraryDetail>);

/// The root: provides the Library store and paints the Hero, as the panel
/// composes it.
fn podcasts_hero_app(props: FixtureProps) -> Element {
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
        let mut sections = Sections::new();
        sections
            .library_list
            .set(Some(SectionRef::new(podcasts_library_list())));
        LibraryState::new(
            sections,
            CoreLink::new(sender),
            playhead_position,
            Arc::clone(&clock),
        )
    });
    provide_context(state.clone());
    handle.borrow_mut().replace((state, detail.clone()));
    rsx! {
        LibraryPodcastsHero { detail }
    }
}

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

    /// The style attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        self.attr_of(selector, "style").unwrap_or_default()
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

    /// The Library commands the core link received, in send order.
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
fn built(facts: Option<GuiPodcastsHeroFrame>) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            podcasts_hero_app,
            FixtureProps {
                sender,
                detail: SectionRef::new(GuiLibraryDetail::Podcasts(detail(facts))),
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

/// The Podcasts Hero mounted for the selected episode.
fn built_hero() -> Fixture {
    built(Some(hero(GuiArtShapeFrame::Square)))
}

/// Contract (`LibraryPodcastsHero.test.ts`, design D15): the artwork box takes
/// the arm every Hero shares — the square keeps the Music Hero's 300 px side,
/// the portrait the tall box, and the landscape fills the Hero content width
/// at 16:9.
#[rstest]
#[case::landscape(GuiArtShapeFrame::Landscape, "width: 100%", "aspect-ratio: 16 / 9")]
#[case::square(GuiArtShapeFrame::Square, "width: 300px", "height: 300px")]
#[case::portrait(GuiArtShapeFrame::Portrait, "width: 300px", "height: 450px")]
fn a_hero_places_its_artwork_box_at_the_shared_arm(
    #[case] shape: GuiArtShapeFrame,
    #[case] width: &str,
    #[case] height: &str,
) {
    let fixture = built(Some(hero(shape)));

    let style = fixture.style_of("[data-library-podcasts-hero-art] > div > div");
    assert!(
        style.contains(width) && style.contains(height),
        "the box holds the {width} by {height} arm, painted: {style}"
    );
}

/// Contract (`LibraryPodcastsHero.test.ts`, spec scenario "A Podcasts
/// episode's Hero"): the Hero paints the show as its title, the episode in
/// the block with its publish date row, the duration as the header band's
/// fact pill, and the description under its Overview section.
#[test]
fn the_hero_paints_the_show_as_its_title_the_episode_in_the_block_and_the_overview() {
    let fixture = built_hero();

    assert_eq!(
        fixture.text_of("[data-library-podcasts-hero-title]"),
        "Bits and Bites",
        "the show titles the header"
    );
    assert_eq!(
        fixture.all_texts("[data-library-podcasts-hero-pills] [data-library-podcasts-hero-pill]"),
        vec!["0 h 54 m".to_owned()],
        "the duration paints as the header band's fact pill"
    );
    assert_eq!(
        fixture.text_of("[data-library-podcasts-hero-block-name]"),
        "Episode 12: The Long Road",
        "the episode paints in the block"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-podcasts-hero-row]",
            "data-library-podcasts-hero-row"
        ),
        vec!["published".to_owned()],
        "the block paints the publish date row"
    );
    assert_eq!(
        fixture.all_texts("[data-library-podcasts-hero-row-value]"),
        vec!["08 Oct".to_owned()],
        "the row paints its value beside the label"
    );
    assert_eq!(
        fixture.text_of("p[data-library-podcasts-hero-overview]"),
        "A conversation about long walks.",
        "the description paints under its Overview section"
    );
}

/// Contract (`LibraryPodcastsHero.test.ts`): a Hero that paints none paints
/// nothing.
#[test]
fn a_hero_that_paints_none_paints_nothing() {
    let fixture = built(None);

    assert!(
        !fixture.presents("[data-library-podcasts-hero]"),
        "no Hero paints"
    );
}

/// Contract (spec scenario "A Podcasts episode's Hero", design D2): a
/// podcast Hero is episode information only — no Episodes band shows in the
/// Hero, and no Workspace or tracklist paints; the episodes stay in the
/// Library list.
#[test]
fn the_hero_paints_no_episodes_band_no_workspace_and_no_tracklist() {
    let fixture = built_hero();

    assert!(
        !fixture.presents("[data-library-podcasts-hero-list]"),
        "no Episodes band paints"
    );
    assert!(
        !fixture.presents("[data-library-shows-workspace]"),
        "no Workspace paints"
    );
    assert!(
        !fixture.presents("[data-library-tracklist]"),
        "no tracklist paints"
    );
}

/// Contract (`LibraryPodcastsHero.test.ts`): Play sends the play command for
/// the selected episode. The press contract rides the handler function the
/// component's own handler calls (a headless document cannot fire a DOM
/// click).
#[test]
fn play_sends_the_play_command_for_the_selected_episode() {
    let mut fixture = built_hero();

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<PodcastsList>(&state, &detail, PodcastsList::play);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::PodcastsPlay {
            lib_idx: LIB_IDX,
            item_id: "show1:ep12".to_owned(),
        }],
        "the play command names the frame's selected episode"
    );
}

/// Contract (`LibraryPodcastsHero.test.ts`): Add to queue sends the enqueue
/// command for the selected episode. The press contract rides the handler
/// function (a headless document cannot fire a DOM click).
#[test]
fn add_to_queue_sends_the_enqueue_command_for_the_selected_episode() {
    let mut fixture = built_hero();

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<PodcastsList>(&state, &detail, PodcastsList::queue);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::PodcastsQueue {
            lib_idx: LIB_IDX,
            item_id: "show1:ep12".to_owned(),
        }],
        "the enqueue command names the frame's selected episode"
    );
}
