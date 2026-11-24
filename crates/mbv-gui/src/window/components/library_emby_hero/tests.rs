//! Headless tests of the plain Emby-library Hero (D16): the contracts of
//! `LibraryEmbyHero.test.ts` — the artwork box reserves the arm the frame
//! declares, the header paints the kind word's kicker and the movie's title,
//! the credits table and the `IMDb` button paint as the TUI's hero shows them,
//! a press on the `IMDb` button sends the open-url command with the url the
//! frame carries (the `IMDb` meta row mapped out of the header's facts), a
//! Hero without facts paints nothing, no Workspace and no tracklist paint,
//! and the actions send the Emby commands with the frame's selected row. The
//! The IPC of the earlier tests is the mock core link here; the asserted
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

use super::{LibraryEmbyHero, open_imdb};
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiDestinationList, GuiEmbyDetail, GuiEmbyHeroCreditFrame, GuiEmbyHeroFrame,
    GuiEmbyList, GuiLibraryDetail, GuiLibraryList, GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::library::hero::hero_command;
use crate::window::library::lettered::{EmbyList, LetteredDestination};
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The library index the Emby commands name.
const LIB_IDX: usize = 3;

/// The selected row's Hero facts: two meta rows, with the runtime and the
/// resume roles marked, and the overview text (design D12, row 8.1).
fn hero(art_shape: GuiArtShapeFrame) -> GuiEmbyHeroFrame {
    GuiEmbyHeroFrame {
        kind: "movie".to_owned(),
        title: "Heat".to_owned(),
        meta_rows: vec![
            vec![GuiMetaSegmentFrame {
                text: "2 h 50 m".to_owned(),
                kind: GuiMetaKindFrame::Time,
            }],
            vec![GuiMetaSegmentFrame {
                text: "1995 · Crime".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            }],
        ],
        duration_row: Some(0),
        progress_row: Some(1),
        overview: Some("A crew plans a bank job.".to_owned()),
        art_shape,
        art: None,
        credits: Vec::new(),
        imdb_url: None,
    }
}

/// The Emby detail part: the row selected, with the Hero `facts`.
fn detail(facts: Option<GuiEmbyHeroFrame>) -> GuiEmbyDetail {
    GuiEmbyDetail {
        selected: Some("movie1".to_owned()),
        hero: facts,
    }
}

/// The Emby list part the destination's commands read the library index
/// from (design D12).
fn emby_library_list() -> GuiLibraryList {
    GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Emby(GuiEmbyList {
            lib_idx: LIB_IDX,
            library_name: "Movies".to_owned(),
            collections: None,
            collection_names: Vec::new(),
            feed_group: false,
            loading: false,
            failed: false,
            rows: Vec::new(),
            sort: None,
        }),
    }
}

/// The harness's props: the carried Emby content, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    detail: SectionRef<GuiLibraryDetail>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the Library store and the
/// Emby detail the presses read.
type FixtureHandles = (LibraryState, SectionRef<GuiLibraryDetail>);

/// The root: provides the Library store and paints the Hero, as the panel
/// composes it.
fn emby_hero_app(props: FixtureProps) -> Element {
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
            .set(Some(SectionRef::new(emby_library_list())));
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
        LibraryEmbyHero { detail }
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
fn built(facts: Option<GuiEmbyHeroFrame>) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            emby_hero_app,
            FixtureProps {
                sender,
                detail: SectionRef::new(GuiLibraryDetail::Emby(detail(facts))),
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

/// Contract (`LibraryEmbyHero.test.ts`, design D15): the artwork box reserves
/// the arm the frame declares — a portrait or square cover keeps the Music
/// Hero's 300 px box at the aspect's height.
#[rstest]
#[case::square(GuiArtShapeFrame::Square, "300px")]
#[case::portrait(GuiArtShapeFrame::Portrait, "450px")]
fn a_cover_hero_places_its_artwork_box_at_300_px_by_the_aspect_s_height(
    #[case] shape: GuiArtShapeFrame,
    #[case] height: &str,
) {
    let fixture = built(Some(hero(shape)));

    let style = fixture.style_of("[data-library-emby-hero-art]");
    assert!(
        style.contains("width: 300px") && style.contains(&format!("height: {height}")),
        "the box holds 300 px by {height}, painted: {style}"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`, design D15): a landscape still fills
/// the content width at 16:9 instead of the 300 px box, as the TUI's hero
/// does.
#[test]
fn a_landscape_hero_fills_the_content_width_at_16_9_instead_of_the_300_px_box() {
    let fixture = built(Some(hero(GuiArtShapeFrame::Landscape)));

    let style = fixture.style_of("[data-library-emby-hero-art]");
    assert!(
        style.contains("width: 100%")
            && style.contains("aspect-ratio: 16 / 9")
            && style.contains("height: auto"),
        "the landscape still fills the content width at 16:9, painted: {style}"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`, invariant 30): the header's big title
/// is the movie itself (a movie is its own largest unit), and the kicker is
/// the item's kind word in upper case.
#[test]
fn the_header_paints_the_item_kind_s_kicker_and_the_movie_s_title() {
    let fixture = built(Some(hero(GuiArtShapeFrame::Landscape)));

    assert_eq!(
        fixture.text_of("[data-library-emby-hero-kicker]"),
        "MOVIE",
        "the kicker is the kind word in upper case"
    );
    assert_eq!(
        fixture.text_of("[data-library-emby-hero-title]"),
        "Heat",
        "the title is the movie itself"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`, design D17, Q7): the Hero paints its
/// credits table as the TUI's hero shows it — the director first, the name at
/// the left and the role at the right.
#[test]
fn the_hero_paints_its_credits_table() {
    let mut facts = hero(GuiArtShapeFrame::Landscape);
    facts.credits = vec![
        GuiEmbyHeroCreditFrame {
            name: "Michael Mann".to_owned(),
            role: "Director".to_owned(),
        },
        GuiEmbyHeroCreditFrame {
            name: "Al Pacino".to_owned(),
            role: "Lt. Vincent Hanna".to_owned(),
        },
    ];
    let fixture = built(Some(facts));

    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-credit-name]"),
        vec!["Michael Mann".to_owned(), "Al Pacino".to_owned()],
        "the names paint in the frame's order"
    );
    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-credit-role]"),
        vec!["Director".to_owned(), "Lt. Vincent Hanna".to_owned()],
        "the roles paint beside their names"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`, design D17, Q7): the `IMDb` button
/// paints under the url the frame carries, and a press sends the open-url
/// command with it. The `IMDb` meta row rides as the header's trailing
/// action, mapped out of the facts the layout joins (invariant 30: the piece
/// is never bent) — the press contract rides the handler function the
/// component's own handler calls (a headless document cannot fire a DOM
/// click).
#[test]
fn the_imdb_button_paints_and_a_press_sends_the_open_url_command() {
    let mut facts = hero(GuiArtShapeFrame::Landscape);
    facts.progress_row = None;
    facts.meta_rows.push(vec![GuiMetaSegmentFrame {
        text: "IMDb".to_owned(),
        kind: GuiMetaKindFrame::Plain,
    }]);
    facts.imdb_url = Some("https://www.imdb.com/title/tt0113277/".to_owned());
    let mut fixture = built(Some(facts));

    assert_eq!(
        fixture.text_of("[data-library-emby-imdb]"),
        "IMDb",
        "the IMDb button paints"
    );
    assert_eq!(
        fixture.attr_of("[data-library-emby-imdb]", "data-library-emby-imdb"),
        Some("https://www.imdb.com/title/tt0113277/".to_owned()),
        "the button carries the url the frame holds"
    );
    assert_eq!(
        fixture.text_of("[data-library-emby-hero-facts]"),
        "1995 · Crime",
        "the IMDb meta row maps out of the header's facts"
    );

    let url = "https://www.imdb.com/title/tt0113277/".to_owned();
    let state = fixture.state.clone();
    fixture.in_runtime(move || open_imdb(&state, &url));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::OpenUrl {
            url: "https://www.imdb.com/title/tt0113277/".to_owned(),
        }],
        "the press sends the open-url command with the frame's url"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`): a Hero that paints none paints
/// nothing.
#[test]
fn a_hero_that_paints_none_paints_nothing() {
    let fixture = built(None);

    assert!(
        !fixture.presents("[data-library-emby-hero]"),
        "no Hero paints"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`): a plain Emby library carries no
/// Workspace and no tracklist — a folder or a boxset plays through the
/// folder-play rule, so the Hero is one block.
#[test]
fn the_hero_paints_no_workspace_and_no_tracklist() {
    let fixture = built(Some(hero(GuiArtShapeFrame::Landscape)));

    assert!(
        !fixture.presents("[data-library-shows-workspace]"),
        "no Workspace paints"
    );
    assert!(
        !fixture.presents("[data-library-tracklist]"),
        "no tracklist paints"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`): Play sends the play command for the
/// selected row. The press contract rides the handler function the
/// component's own handler calls (a headless document cannot fire a DOM
/// click).
#[test]
fn play_sends_the_play_command_for_the_selected_row() {
    let mut fixture = built(Some(hero(GuiArtShapeFrame::Landscape)));

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<EmbyList>(&state, &detail, EmbyList::play);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::EmbyPlay {
            lib_idx: LIB_IDX,
            item_id: "movie1".to_owned(),
        }],
        "the play command names the frame's selected row"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`): Play next sends the play-next
/// command for the selected row. The press contract rides the handler
/// function (a headless document cannot fire a DOM click).
#[test]
fn play_next_sends_the_play_next_command_for_the_selected_row() {
    let mut fixture = built(Some(hero(GuiArtShapeFrame::Landscape)));

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<EmbyList>(&state, &detail, EmbyList::play_next);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::EmbyPlayNext {
            lib_idx: LIB_IDX,
            item_id: "movie1".to_owned(),
        }],
        "the play-next command names the frame's selected row"
    );
}

/// Contract (`LibraryEmbyHero.test.ts`): Add to queue sends the enqueue
/// command for the selected row. The press contract rides the handler
/// function (a headless document cannot fire a DOM click).
#[test]
fn add_to_queue_sends_the_enqueue_command_for_the_selected_row() {
    let mut fixture = built(Some(hero(GuiArtShapeFrame::Landscape)));

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<EmbyList>(&state, &detail, EmbyList::queue);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::EmbyQueue {
            lib_idx: LIB_IDX,
            item_id: "movie1".to_owned(),
        }],
        "the enqueue command names the frame's selected row"
    );
}
