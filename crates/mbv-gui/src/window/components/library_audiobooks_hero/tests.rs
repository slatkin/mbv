//! Headless tests of the Audiobooks Hero (D16): the contracts of
//! `LibraryAudiobooksHero.test.ts` — the portrait cover-box, the title, the
//! duration fact pill, the author row, no progress element and no overview,
//! the chapter rows once they have landed, the chapter the predicted position
//! holds marked while the Hero's book plays, the loading line while the
//! detail has not landed, and the commands the chapter gestures and the
//! Hero's buttons send. The IPC of the earlier tests is the mock core
//! link here; the asserted commands are the same neutral shapes. A headless
//! document cannot fire a DOM click, so the press contracts ride the handler
//! functions the component's own handlers call one line each. The core
//! channel is a mock pair; no test opens a window, sleeps, or reaches the
//! core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryAudiobooksHero, play_chapter_row};
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiAudiobooksChapterRowFrame, GuiAudiobooksDetail, GuiAudiobooksHeroFrame,
    GuiAudiobooksList, GuiAudiobooksWorkspaceFrame, GuiDestinationList, GuiHeroFactLabelFrame,
    GuiHeroFactRowFrame, GuiHeroFactsFrame, GuiHeroPillFrame, GuiLibraryDetail, GuiLibraryList,
    GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::library::hero::hero_command;
use crate::window::library::lettered::{AudiobooksList, LetteredDestination};
use crate::window::library::state::LibraryState;
use crate::window::playhead::ticks_f64;
use crate::window::sections::{SectionRef, Sections};

/// The library index the Audiobooks commands name.
const LIB_IDX: usize = 4;

/// The ticks in one second, as the former test clock's `TEST_SECOND` is.
const SECOND: i64 = mbv_emby_model::TICKS_PER_SECOND;

/// A book's chapters on the merged timeline: the first spans 0–21:00, the
/// second 21:00–45:00 (design D5's `startTicks`/`endTicks`).
fn chapters() -> Vec<GuiAudiobooksChapterRowFrame> {
    vec![
        GuiAudiobooksChapterRowFrame {
            chapter: 1,
            title: "First Contact".to_owned(),
            duration: "21:00".to_owned(),
            start_ticks: 0,
            end_ticks: 21 * 60 * SECOND,
        },
        GuiAudiobooksChapterRowFrame {
            chapter: 2,
            title: "Fallen Angel".to_owned(),
            duration: "24:00".to_owned(),
            start_ticks: 21 * 60 * SECOND,
            end_ticks: 45 * 60 * SECOND,
        },
    ]
}

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

/// The selected book's Hero facts: the duration pill, the author row, and
/// the chapter Workspace (design D12, row 9.1; design D1).
fn hero(overrides: impl FnOnce(&mut GuiAudiobooksHeroFrame)) -> GuiAudiobooksHeroFrame {
    let mut facts = GuiAudiobooksHeroFrame {
        title: "The Caves of Steel".to_owned(),
        facts: GuiHeroFactsFrame {
            pills: vec![pill(vec![segment("9 h 51 m", GuiMetaKindFrame::Time)])],
            rows: vec![row(GuiHeroFactLabelFrame::Author, "Isaac Asimov")],
        },
        overview: None,
        art_shape: GuiArtShapeFrame::Portrait,
        art: None,
        workspace: Some(GuiAudiobooksWorkspaceFrame { rows: chapters() }),
    };
    overrides(&mut facts);
    facts
}

/// The Audiobooks detail part: the book selected, with `playing` the playing
/// book and `loading` the Workspace's load fact.
fn detail(
    facts: Option<GuiAudiobooksHeroFrame>,
    loading: bool,
    playing: Option<&str>,
) -> GuiAudiobooksDetail {
    GuiAudiobooksDetail {
        workspace_loading: loading,
        selected: Some("book1".to_owned()),
        playing_book_id: playing.map(str::to_owned),
        hero: facts,
    }
}

/// The Audiobooks list part the destination's commands read the library index
/// from (design D12).
fn audiobooks_library_list() -> GuiLibraryList {
    GuiLibraryList {
        refreshing: false,
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Audiobooks(GuiAudiobooksList {
            lib_idx: LIB_IDX,
            loading: false,
            failed: false,
            rows: Vec::new(),
            sort: None,
        }),
    }
}

/// The harness's props: the carried Audiobooks content, the playhead position
/// the mark reads, the mock core sender, and the handle the test reads the
/// store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    detail: SectionRef<GuiLibraryDetail>,
    position_ticks: f64,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the Library store and the
/// Audiobooks detail the presses read.
type FixtureHandles = (LibraryState, SectionRef<GuiLibraryDetail>);

/// The root: provides the Library store and paints the Hero, as the panel
/// composes it.
fn abs_hero_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        detail,
        position_ticks,
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
        // The playhead position the interaction state reads: the test's fixed
        // position, stamped before the build (design D5).
        let playhead_position = Memo::new(move || position_ticks);
        let mut sections = Sections::new();
        sections
            .library_list
            .set(Some(SectionRef::new(audiobooks_library_list())));
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
        LibraryAudiobooksHero { lib_idx: LIB_IDX, detail }
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
/// effects land. `position_ticks` is the playhead's predicted position the
/// chapter mark reads (design D5).
fn built(facts: Option<GuiAudiobooksHeroFrame>, loading: bool, playing: Option<&str>) -> Fixture {
    built_at(facts, loading, playing, 0.0)
}

/// [`built`] at a playhead position other than the start.
fn built_at(
    facts: Option<GuiAudiobooksHeroFrame>,
    loading: bool,
    playing: Option<&str>,
    position_ticks: f64,
) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            abs_hero_app,
            FixtureProps {
                sender,
                detail: SectionRef::new(GuiLibraryDetail::Audiobooks(detail(
                    facts, loading, playing,
                ))),
                position_ticks,
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

/// The Audiobooks Hero mounted for the selected book.
fn built_book() -> Fixture {
    built(Some(hero(|_| ())), false, None)
}

/// The playhead stamped mid-chapter-two, the book the frame selects, so the
/// Workspace marks it (design D5).
fn chapter_two_position() -> f64 {
    ticks_f64(30 * 60 * SECOND)
}

/// Contract (`LibraryAudiobooksHero.test.ts`): the cover art is the book
/// artwork policy's portrait — the Hero places its artwork box at the
/// portrait arm's size.
#[test]
fn the_portrait_hero_places_its_artwork_box_at_the_portrait_size() {
    let fixture = built_book();

    let style = fixture.style_of("[data-library-abs-hero-art] > div > div");
    assert!(
        style.contains("width: 300px") && style.contains("height: 450px"),
        "the portrait box holds 300 px by 450 px, painted: {style}"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`, design D2): the Hero paints
/// its title, the book's duration as the header band's fact pill, and the
/// book's author as the block's labelled row.
#[test]
fn the_hero_paints_its_title_its_duration_pill_and_its_author_row() {
    let fixture = built_book();

    assert_eq!(
        fixture.text_of("[data-library-abs-hero-title]"),
        "The Caves of Steel",
        "the title paints"
    );
    assert_eq!(
        fixture.all_texts("[data-library-abs-hero-pills] [data-library-abs-hero-pill]"),
        vec!["9 h 51 m".to_owned()],
        "the duration paints as the header band's fact pill"
    );
    assert_eq!(
        fixture.all_attrs("[data-library-abs-hero-row]", "data-library-abs-hero-row"),
        vec!["author".to_owned()],
        "the block paints the author row"
    );
    assert_eq!(
        fixture.all_texts("[data-library-abs-hero-row-value]"),
        vec!["Isaac Asimov".to_owned()],
        "the row paints its value beside the label"
    );
}

/// Contract (spec scenario "A partly played item shows no progress"): a book
/// played past its half shows no progress element — no progress bar, no
/// resume position, and no remaining time; Play still resumes from the stored
/// position through the queue.
#[test]
fn a_partly_played_item_shows_no_progress() {
    let fixture = built_at(
        Some(hero(|_| ())),
        false,
        Some("book1"),
        chapter_two_position(),
    );

    assert!(
        !fixture.presents("[data-library-abs-hero-role]"),
        "no duration or progress role row paints"
    );
    assert!(
        !fixture.text_of("[data-library-abs-hero]").contains('%'),
        "no painted text holds a progress percent"
    );
}

/// Contract (gui-hero-design-polish D2): the list band paints under the
/// Chapters section header.
#[test]
fn the_list_band_paints_under_the_chapters_section() {
    let fixture = built_book();

    assert_eq!(
        fixture.text_of("section[data-library-abs-hero-list] span"),
        "Chapters",
        "the list band's section header names Chapters"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`): the book's facts carry no
/// overview — the Hero keeps the TUI's facts.
#[test]
fn the_hero_paints_no_overview() {
    let fixture = built_book();

    assert!(
        !fixture.presents("[data-library-abs-hero-overview]"),
        "no overview paints"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`): the Workspace paints the
/// chapter rows once the chapters have landed.
#[test]
fn the_workspace_paints_the_chapter_rows_once_the_chapters_have_landed() {
    let fixture = built_book();

    assert_eq!(
        fixture.all_attrs("[data-library-abs-chapter]", "data-library-abs-chapter"),
        vec!["1".to_owned(), "2".to_owned()],
        "the chapters paint in the book's order"
    );
    assert_eq!(
        fixture.text_of("[data-library-abs-chapter=\"2\"] [data-library-abs-chapter-name]"),
        "Fallen Angel",
        "the chapter's title paints"
    );
    assert_eq!(
        fixture.text_of("[data-library-abs-chapter=\"2\"] [data-library-abs-chapter-duration]"),
        "24:00",
        "the chapter's duration paints"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`, design D5): the chapter whose
/// span holds the predicted position carries the tracklist's play glyph,
/// while the Hero's book is the playing book — a highlight marks focus only,
/// so the row keeps its own fill.
#[test]
fn the_workspace_marks_the_chapter_the_predicted_position_holds() {
    let fixture = built_at(
        Some(hero(|_| ())),
        false,
        Some("book1"),
        chapter_two_position(),
    );

    assert_eq!(
        fixture.attr_of(
            "[data-library-abs-chapter=\"2\"]",
            "data-library-abs-chapter-playing"
        ),
        Some(String::new()),
        "the marked chapter carries the playing mark"
    );
    assert!(
        fixture.presents("[data-library-abs-chapter=\"2\"] [data-library-abs-chapter-glyph]"),
        "the marked chapter paints its play glyph"
    );
    assert!(
        fixture
            .attr_of(
                "[data-library-abs-chapter=\"1\"]",
                "data-library-abs-chapter-playing"
            )
            .is_none(),
        "no other chapter carries it"
    );
    assert!(
        !fixture.presents("[data-library-abs-chapter=\"1\"] [data-library-abs-chapter-glyph]"),
        "no other chapter paints a glyph"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`, design D5): a different playing
/// book marks no chapter.
#[test]
fn a_different_playing_book_marks_no_chapter() {
    let fixture = built_at(
        Some(hero(|_| ())),
        false,
        Some("book2"),
        chapter_two_position(),
    );

    assert!(
        fixture
            .attr_of(
                "[data-library-abs-chapter=\"2\"]",
                "data-library-abs-chapter-playing"
            )
            .is_none(),
        "no chapter carries the playing mark"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`): a book whose detail has not
/// landed paints the loading line instead of rows.
#[test]
fn a_book_whose_detail_has_not_landed_paints_the_loading_line() {
    let fixture = built(Some(hero(|facts| facts.workspace = None)), true, None);

    assert!(
        fixture
            .text_of("[data-library-abs-chapters-loading]")
            .contains("Loading chapters"),
        "the loading line paints"
    );
    assert!(
        !fixture.presents("[data-library-abs-chapter]"),
        "no chapter row paints"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`): a Hero that paints none paints
/// nothing.
#[test]
fn a_hero_that_paints_none_paints_nothing() {
    let fixture = built(None, false, None);

    assert!(
        !fixture.presents("[data-library-abs-hero]"),
        "no Hero paints"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`): a double-click on a chapter
/// and a hover on its play control send its play-chapter command — the two
/// gestures ride the same play path the component's handlers call
/// (`play_chapter_row`, one line each). A headless document fires neither
/// gesture.
#[test]
fn a_chapter_s_play_gesture_sends_its_play_chapter_command() {
    let mut fixture = built_book();

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || play_chapter_row(&state, LIB_IDX, &detail, "2"));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::AbsPlayChapter {
            lib_idx: LIB_IDX,
            book_id: "book1".to_owned(),
            chapter: 2,
        }],
        "the play-chapter command names the played chapter"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`): Play sends the play command
/// for the selected book. The press contract rides the handler function the
/// component's own handler calls (a headless document cannot fire a DOM
/// click).
#[test]
fn play_sends_the_play_command_for_the_selected_book() {
    let mut fixture = built_book();

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<AudiobooksList>(&state, &detail, AudiobooksList::play);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::AbsPlay {
            lib_idx: LIB_IDX,
            item_id: "book1".to_owned(),
        }],
        "the play command names the selected book"
    );
}

/// Contract (`LibraryAudiobooksHero.test.ts`): Add to queue sends the enqueue
/// command for the selected book. The press contract rides the handler
/// function (a headless document cannot fire a DOM click).
#[test]
fn add_to_queue_sends_the_enqueue_command_for_the_selected_book() {
    let mut fixture = built_book();

    let detail = fixture.detail.clone();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        hero_command::<AudiobooksList>(&state, &detail, AudiobooksList::queue);
    });

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::AbsQueue {
            lib_idx: LIB_IDX,
            item_id: "book1".to_owned(),
        }],
        "the enqueue command names the selected book"
    );
}
