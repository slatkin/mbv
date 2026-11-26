//! Headless tests of the Feeds Hero (D16): the contracts of
//! `LibraryFeedsHero.test.ts` — the placeholder artwork box takes the arm
//! every Hero shares (a feed entry has no artwork source), the feed titles
//! the header with the entry in the block, the duration as a fact pill and
//! the publish date row, no overview paints, no Workspace and no tracklist
//! paint, the actions send the
//! Feeds commands for the frame's selected entry with no library index, and the
//! list band holds the Hero's feed's entries alone. The IPC of the earlier
//! tests is the mock core link here;
//! the asserted commands are the same neutral shapes. A headless document
//! cannot fire a DOM click, so the press contracts ride the handler functions
//! the component's own handlers call one line each. The core channel is a
//! mock pair; no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::LibraryFeedsHero;
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiCollections, GuiDestinationList, GuiEntryRowFrame, GuiFeedsDetail,
    GuiFeedsHeroFrame, GuiFeedsList, GuiHeroFactLabelFrame, GuiHeroFactRowFrame, GuiHeroFactsFrame,
    GuiHeroPillFrame, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList, GuiMetaKindFrame,
    GuiMetaSegmentFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::library::hero::hero_command;
use crate::window::library::lettered::{FeedsList, LetteredDestination};
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

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

/// The selected entry's Hero facts: the feed as the parent, the duration
/// pill, and the publish date row (design D12, row 11.1; design D1). A feed
/// entry has no artwork source and no overview, so neither rides here.
fn hero(art_shape: GuiArtShapeFrame) -> GuiFeedsHeroFrame {
    GuiFeedsHeroFrame {
        title: "The Long Road Home".to_owned(),
        parent: Some("Bits and Bites".to_owned()),
        facts: GuiHeroFactsFrame {
            pills: vec![pill(vec![segment("0 h 54 m", GuiMetaKindFrame::Time)])],
            rows: vec![row(GuiHeroFactLabelFrame::Published, "08 Oct")],
        },
        art_shape,
    }
}

/// The carried Feeds rows: one merged entry, `extra` the test's own rows.
fn rows(extra: Vec<GuiEntryRowFrame>) -> Vec<GuiEntryRowFrame> {
    let mut rows = vec![GuiEntryRowFrame {
        sort_letter: None,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: "sub1:guid12".to_owned(),
        title: "The Long Road Home".to_owned(),
        subtitle: Some("Bits and Bites".to_owned()),
        date: Some("08 Oct".to_owned()),
        state: GuiHomeRowStateFrame::Ordinary,
    }];
    rows.extend(extra);
    rows
}

/// The Feeds detail part with the hero picked.
fn detail(facts: Option<GuiFeedsHeroFrame>, playing: Option<&str>) -> GuiFeedsDetail {
    GuiFeedsDetail {
        playing_entry_id: playing.map(str::to_owned),
        selected: Some("sub1:guid12".to_owned()),
        hero: facts,
    }
}

/// The harness's props: the carried Feeds content, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    list: SectionRef<GuiLibraryList>,
    detail: SectionRef<GuiLibraryDetail>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the Library store and the
/// detail the presses read.
type FixtureHandles = (LibraryState, SectionRef<GuiLibraryDetail>);

/// The root: provides the Library store and paints the Hero, as the panel
/// composes it.
fn feeds_hero_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        list,
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
    handle.borrow_mut().replace((state, detail.clone()));
    rsx! {
        LibraryFeedsHero { list, detail }
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
fn built(facts: Option<GuiFeedsHeroFrame>, rows: Vec<GuiEntryRowFrame>) -> Fixture {
    built_playing(facts, rows, None)
}

/// Build the document with the playing entry id the detail carries.
fn built_playing(
    facts: Option<GuiFeedsHeroFrame>,
    rows: Vec<GuiEntryRowFrame>,
    playing: Option<&str>,
) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            feeds_hero_app,
            FixtureProps {
                sender,
                list: SectionRef::new(GuiLibraryList {
                    refreshing: false,
                    tabs: Vec::new(),
                    current_tab: Some(2),
                    destination: GuiDestinationList::Feeds(GuiFeedsList {
                        sort: None,
                        collections: GuiCollections {
                            target: crate::gui_core::GuiCollectionTarget::Feeds,
                            all: true,
                            entries: Vec::new(),
                            noun: crate::gui_core::GuiCollectionNoun::Subscriptions,
                        },
                        loading: false,
                        failed: false,
                        platform: None,
                        rows,
                    }),
                }),
                detail: SectionRef::new(GuiLibraryDetail::Feeds(detail(facts, playing))),
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

/// The Feeds Hero mounted for the selected entry.
fn built_hero() -> Fixture {
    built(Some(hero(GuiArtShapeFrame::Square)), rows(Vec::new()))
}

/// Contract (`LibraryFeedsHero.test.ts`, design D15, row 11.1): a feed entry
/// has no artwork source, so the box paints the placeholder in every arm the
/// shared artwork component reserves — the square keeps the 300 px side, and
/// the landscape fills the Hero content width at 16:9.
#[rstest]
#[case::square(GuiArtShapeFrame::Square, "width: 300px", "height: 300px")]
#[case::landscape(GuiArtShapeFrame::Landscape, "width: 100%", "aspect-ratio: 16 / 9")]
fn a_hero_places_its_placeholder_artwork_box_at_the_shared_arm(
    #[case] shape: GuiArtShapeFrame,
    #[case] width: &str,
    #[case] height: &str,
) {
    let fixture = built(Some(hero(shape)), rows(Vec::new()));

    let style = fixture.style_of("[data-library-feeds-hero-art] > div > div");
    assert!(
        style.contains(width) && style.contains(height),
        "the box holds the {width} by {height} arm, painted: {style}"
    );
    assert!(
        fixture
            .text_of("[data-library-feeds-hero-art]")
            .contains("the window's placeholder"),
        "the placeholder paints, for the entry has no artwork source"
    );
}

/// Contract (`LibraryFeedsHero.test.ts`): the Hero paints the feed as its
/// title, the entry in the block with its publish date row, and the duration
/// as the header band's fact pill.
#[test]
fn the_hero_paints_the_feed_as_its_title_the_entry_in_the_block_and_the_duration_pill() {
    let fixture = built_hero();

    assert_eq!(
        fixture.text_of("[data-library-feeds-hero-title]"),
        "Bits and Bites",
        "the feed titles the header"
    );
    assert_eq!(
        fixture.text_of("[data-library-feeds-hero-block-name]"),
        "The Long Road Home",
        "the entry paints in the block"
    );
    assert_eq!(
        fixture.all_texts("[data-library-feeds-hero-pills] [data-library-feeds-hero-pill]"),
        vec!["0 h 54 m".to_owned()],
        "the duration paints as the header band's fact pill"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-feeds-hero-row]",
            "data-library-feeds-hero-row"
        ),
        vec!["published".to_owned()],
        "the block paints the publish date row"
    );
    assert_eq!(
        fixture.all_texts("[data-library-feeds-hero-row-value]"),
        vec!["08 Oct".to_owned()],
        "the row paints its value beside the label"
    );
}

/// Contract (`LibraryFeedsHero.test.ts`): a feed entry carries no overview,
/// so no overview paragraph paints (design D12, row 11.1).
#[test]
fn the_hero_paints_no_overview_paragraph() {
    let fixture = built_hero();

    assert!(
        !fixture.presents("[data-library-feeds-hero-overview]"),
        "no overview paints"
    );
}

/// Contract (`LibraryFeedsHero.test.ts`): a Hero that paints none paints
/// nothing.
#[test]
fn a_hero_that_paints_none_paints_nothing() {
    let fixture = built(None, rows(Vec::new()));

    assert!(
        !fixture.presents("[data-library-feeds-hero]"),
        "no Hero paints"
    );
}

/// Contract (`LibraryFeedsHero.test.ts`): a Feeds Hero is entry information
/// only — no Workspace and no tracklist paints (design D12, row 11.1).
#[test]
fn the_hero_paints_no_workspace_and_no_tracklist() {
    let fixture = built_hero();

    assert!(
        !fixture.presents("[data-library-shows-workspace]"),
        "no Workspace paints"
    );
    assert!(
        !fixture.presents("[data-library-tracklist]"),
        "no tracklist paints"
    );
}

/// Contract (`LibraryFeedsHero.test.ts`, design D2, row 11.1): Play sends the
/// play command for the selected entry, named by the row id the frame
/// painted — a Client holds one feed tab, so no library index rides it. The
/// press contract rides the handler function the component's own handler
/// calls (a headless document cannot fire a DOM click).
#[test]
fn play_sends_the_play_command_for_the_selected_entry() {
    let mut fixture = built_hero();

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<FeedsList>(&state, &detail, FeedsList::play);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::FeedsPlay {
            item_id: "sub1:guid12".to_owned(),
        }],
        "the play command names the frame's selected entry"
    );
}

/// Contract (`LibraryFeedsHero.test.ts`): Add to queue sends the enqueue
/// command for the selected entry. The press contract rides the handler
/// function (a headless document cannot fire a DOM click).
#[test]
fn add_to_queue_sends_the_enqueue_command_for_the_selected_entry() {
    let mut fixture = built_hero();

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<FeedsList>(&state, &detail, FeedsList::queue);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::FeedsQueue {
            item_id: "sub1:guid12".to_owned(),
        }],
        "the enqueue command names the frame's selected entry"
    );
}

/// Contract (`LibraryFeedsHero.test.ts`, invariant 30): a feed is a
/// collection — the list band paints under the Entries section and holds the
/// feed's entries alone, and no entry carries a cursor: the Workspace keeps
/// none (`gui-pointer-target-interaction` D6).
#[test]
fn the_list_band_holds_the_feed_entries_alone() {
    let extra = vec![
        GuiEntryRowFrame {
            sort_letter: None,
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "sub1:guid11".to_owned(),
            title: "Earlier Walk".to_owned(),
            subtitle: Some("Bits and Bites".to_owned()),
            date: Some("07 Oct".to_owned()),
            state: GuiHomeRowStateFrame::Ordinary,
        },
        GuiEntryRowFrame {
            sort_letter: None,
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "sub2:guid20".to_owned(),
            title: "Other Feed".to_owned(),
            subtitle: Some("Elsewhere".to_owned()),
            date: Some("06 Oct".to_owned()),
            state: GuiHomeRowStateFrame::Ordinary,
        },
    ];
    let fixture = built(Some(hero(GuiArtShapeFrame::Square)), rows(extra));

    assert_eq!(
        fixture.text_of("section[data-library-feeds-hero-list] span"),
        "Entries",
        "the list band's section header names Entries"
    );
    assert_eq!(
        fixture.all_attrs("[data-library-feeds-entry]", "data-library-feeds-entry"),
        vec!["sub1:guid12".to_owned(), "sub1:guid11".to_owned()],
        "the list band holds the Hero's feed's entries alone"
    );
    assert!(
        !fixture.presents("[data-library-feeds-entry-cursor]"),
        "no entry carries a cursor"
    );
    assert_eq!(
        fixture.attr_of("[data-pointer-row]", "data-pointer-row"),
        Some("sub1:guid12".to_owned()),
        "the first entry carries the pointer marker"
    );
}

/// Contract (gui-hero-design-polish design D6, row 8.2): the entry row whose
/// id the playing entry carries takes the playing mark, and an idle playhead
/// marks no row.
#[rstest]
#[case::the_playing_entry(Some("sub1:guid11"), vec!["sub1:guid11"])]
#[case::an_idle_playhead(None, Vec::new())]
fn the_playing_entry_marks_its_own_entry_row(
    #[case] playing: Option<&str>,
    #[case] expected: Vec<&str>,
) {
    let earlier = GuiEntryRowFrame {
        sort_letter: None,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: "sub1:guid11".to_owned(),
        title: "The Earlier Road".to_owned(),
        subtitle: Some("Bits and Bites".to_owned()),
        date: Some("01 Oct".to_owned()),
        state: GuiHomeRowStateFrame::Ordinary,
    };
    let fixture = built_playing(
        Some(hero(GuiArtShapeFrame::Square)),
        rows(vec![earlier]),
        playing,
    );

    assert_eq!(
        fixture.all_attrs(
            "[data-library-feeds-entry-playing]",
            "data-library-feeds-entry"
        ),
        expected,
        "only the playing entry's row carries the playing mark"
    );
}
