//! Headless tests of the plain Emby-library Hero (gui-hero-design-polish
//! design D2, D3, D4): the contracts of the movie case of the spec scenario
//! "A movie's Hero" — the header band paints the kind word's kicker, the
//! movie's title, and the fact pills in the frame's order with the `IMDb` link
//! as a link pill; the detail block paints the labelled rows in the frame's
//! order and no runtime row; the cast grid paints the members in the frame's
//! order, a member with no photo shows the member's initials; no `IMDb` header
//! action paints; the Hero's art makes its backdrop through the shared
//! art-to-backdrop mapping; a Hero without facts paints nothing; no Workspace
//! and no tracklist paint; and the actions send the Emby commands with the
//! frame's selected row. The IPC of the earlier tests is the mock core link
//! here; the asserted commands are the same neutral shapes. A headless
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
use rstest::rstest;

use super::LibraryEmbyHero;
use crate::art::{GuiArtRef, art_url};
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiDestinationList, GuiEmbyDetail, GuiEmbyHeroFrame, GuiEmbyList,
    GuiHeroCastMemberFrame, GuiHeroFactLabelFrame, GuiHeroFactRowFrame, GuiHeroFactsFrame,
    GuiHeroPillFrame, GuiLibraryDetail, GuiLibraryList, GuiMetaKindFrame, GuiMetaSegmentFrame,
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

/// One link pill: its typed segments and the URL its press opens.
fn link_pill(segments: Vec<GuiMetaSegmentFrame>, url: &str) -> GuiHeroPillFrame {
    GuiHeroPillFrame {
        segments,
        link: Some(url.to_owned()),
    }
}

/// One labelled fact row: the label word its enum names, one plain value.
fn row(label: GuiHeroFactLabelFrame, value: &str) -> GuiHeroFactRowFrame {
    GuiHeroFactRowFrame {
        label,
        value: vec![segment(value, GuiMetaKindFrame::Plain)],
    }
}

/// The selected row's Hero facts: the movie case of the spec scenario
/// "A movie's Hero" — pills for the year, the genre, the runtime, and the
/// `IMDb` link; rows for the director, the release date, and the studio, and
/// no runtime row.
fn movie_hero(art_shape: GuiArtShapeFrame) -> GuiEmbyHeroFrame {
    GuiEmbyHeroFrame {
        kind: "movie".to_owned(),
        title: "Heat".to_owned(),
        facts: GuiHeroFactsFrame {
            pills: vec![
                pill(vec![segment("1995", GuiMetaKindFrame::Date)]),
                pill(vec![segment("Crime", GuiMetaKindFrame::Plain)]),
                pill(vec![segment("2 h 50 m", GuiMetaKindFrame::Time)]),
                link_pill(
                    vec![segment("IMDb", GuiMetaKindFrame::Plain)],
                    "https://www.imdb.com/title/tt0113277/",
                ),
            ],
            rows: vec![
                row(GuiHeroFactLabelFrame::Director, "Michael Mann"),
                row(GuiHeroFactLabelFrame::Released, "08 Oct 1995"),
                row(GuiHeroFactLabelFrame::Studio, "20th Century Fox"),
            ],
        },
        overview: Some("A crew plans a bank job.".to_owned()),
        art_shape,
        art: None,
        cast: Vec::new(),
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
        refreshing: false,
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

/// Contract (`LibraryEmbyHero.test.ts`, invariant 30): the header's big title
/// is the movie itself (a movie is its own largest unit), and the kicker is
/// the item's kind word in upper case.
#[test]
fn the_header_paints_the_item_kind_s_kicker_and_the_movie_s_title() {
    let fixture = built(Some(movie_hero(GuiArtShapeFrame::Landscape)));

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

/// Contract (spec scenario "A movie's Hero", gui-hero-design-polish D2): the
/// header band paints the movie's fact pills in the frame's order — the year,
/// each genre, the runtime, and the `IMDb` link as the one link pill.
#[test]
fn the_header_paints_the_movie_s_pills_in_the_frame_s_order() {
    let fixture = built(Some(movie_hero(GuiArtShapeFrame::Landscape)));

    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-pill]"),
        vec![
            "1995".to_owned(),
            "Crime".to_owned(),
            "2 h 50 m".to_owned(),
            "IMDb".to_owned(),
        ],
        "the pills paint in the frame's order, the IMDb link among them"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-emby-hero-pill]",
            "data-library-emby-hero-pill"
        ),
        vec![
            "fact".to_owned(),
            "fact".to_owned(),
            "fact".to_owned(),
            "link".to_owned(),
        ],
        "the IMDb pill paints as the one link pill"
    );
}

/// Contract (spec scenario "A movie's Hero", gui-hero-design-polish D2): the
/// detail block paints the movie's labelled rows in the frame's order — the
/// director, the release date, and the studio — and no runtime row: the
/// runtime is a header pill, so no row paints a duration.
#[test]
fn the_block_paints_the_movie_s_labelled_rows_in_the_frame_s_order() {
    let fixture = built(Some(movie_hero(GuiArtShapeFrame::Landscape)));

    assert_eq!(
        fixture.all_attrs("[data-library-emby-hero-row]", "data-library-emby-hero-row"),
        vec![
            "director".to_owned(),
            "released".to_owned(),
            "studio".to_owned()
        ],
        "the rows paint in the frame's order, the label wire per row"
    );
    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-row-label]"),
        vec![
            "Director".to_owned(),
            "Released".to_owned(),
            "Studio".to_owned()
        ],
        "each row paints the label word its enum names"
    );
    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-row-value]"),
        vec![
            "Michael Mann".to_owned(),
            "08 Oct 1995".to_owned(),
            "20th Century Fox".to_owned(),
        ],
        "each row paints its value beside the label"
    );
}

/// Contract (spec scenario "A movie's Hero", the cast member with no photo,
/// gui-hero-design-polish D3): the cast grid paints the members in the
/// frame's order — the people that are not directors — with each member's
/// name and role.
#[test]
fn the_cast_grid_paints_the_members_in_the_frame_s_order() {
    let mut hero = movie_hero(GuiArtShapeFrame::Landscape);
    hero.cast = vec![
        GuiHeroCastMemberFrame {
            name: "Al Pacino".to_owned(),
            role: "Lt. Vincent Hanna".to_owned(),
            initials: "AP".to_owned(),
            photo: None,
        },
        GuiHeroCastMemberFrame {
            name: "Robert De Niro".to_owned(),
            role: "Neil McCauley".to_owned(),
            initials: "RD".to_owned(),
            photo: Some(GuiArtRef::Image {
                src: art_url("card/photo"),
            }),
        },
    ];
    let fixture = built(Some(hero));

    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-cast-name]"),
        vec!["Al Pacino".to_owned(), "Robert De Niro".to_owned()],
        "the names paint in the frame's order"
    );
    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-cast-role]"),
        vec!["Lt. Vincent Hanna".to_owned(), "Neil McCauley".to_owned()],
        "the roles paint beside their names"
    );
}

/// Contract (spec scenario "A cast member with no photo", gui-hero-design-polish
/// D3): the member the Service holds no photo for shows the member's initials
/// where a photo would show, and the member with a photo paints the photo.
#[test]
fn a_cast_member_with_no_photo_shows_the_member_s_initials() {
    let mut hero = movie_hero(GuiArtShapeFrame::Landscape);
    hero.cast = vec![
        GuiHeroCastMemberFrame {
            name: "Al Pacino".to_owned(),
            role: "Lt. Vincent Hanna".to_owned(),
            initials: "AP".to_owned(),
            photo: None,
        },
        GuiHeroCastMemberFrame {
            name: "Robert De Niro".to_owned(),
            role: "Neil McCauley".to_owned(),
            initials: "RD".to_owned(),
            photo: Some(GuiArtRef::Image {
                src: art_url("card/photo"),
            }),
        },
    ];
    let fixture = built(Some(hero));

    assert_eq!(
        fixture.all_texts("[data-library-emby-hero-cast-initials]"),
        vec!["AP".to_owned()],
        "the member with no photo shows the member's initials"
    );
    let photos = fixture.all_attrs("[data-library-emby-hero-cast-photo] img", "src");
    assert_eq!(
        photos,
        vec![art_url("card/photo")],
        "the member with a photo paints the photo"
    );
}

/// Contract (gui-hero-design-polish D2, the deleted `IMDb` header action): no
/// `IMDb` button paints in the header band — the `IMDb` link is the link pill,
/// and no other header action paints.
#[test]
fn no_imdb_header_action_paints() {
    let fixture = built(Some(movie_hero(GuiArtShapeFrame::Landscape)));

    assert!(
        !fixture.presents("[data-library-emby-imdb]"),
        "no IMDb header action paints"
    );
    assert!(
        fixture.presents("[data-library-emby-hero-pill]"),
        "the IMDb link paints as a fact pill instead"
    );
}

/// The one case table (gui-hero-design-polish D4, spec scenario "The Hero's
/// artwork makes its backdrop"): the backdrop paints through the shared art-to-backdrop
/// mapping exactly when the Hero carries art with a card source — none paints
/// without art, with the bundled placeholder, or with art still loading.
#[rstest]
#[case::no_art(None, false)]
#[case::placeholder_art(Some(GuiArtRef::Placeholder {
    src: art_url("placeholder"),
}), false)]
#[case::loading_art(Some(GuiArtRef::Loading), false)]
#[case::card_art(Some(GuiArtRef::Image {
    src: art_url("card/backdrop"),
}), true)]
fn the_hero_s_art_makes_its_backdrop(#[case] art: Option<GuiArtRef>, #[case] paints: bool) {
    let mut hero = movie_hero(GuiArtShapeFrame::Landscape);
    hero.art = art;
    let fixture = built(Some(hero));

    assert_eq!(
        fixture.presents("[data-library-emby-hero-backdrop]"),
        paints,
        "the backdrop paints exactly when the Hero carries card art"
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
    let fixture = built(Some(movie_hero(GuiArtShapeFrame::Landscape)));

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
    let mut fixture = built(Some(movie_hero(GuiArtShapeFrame::Landscape)));

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

/// Contract (`LibraryEmbyHero.test.ts`): Add to queue sends the enqueue
/// command for the selected row. The press contract rides the handler
/// function (a headless document cannot fire a DOM click).
#[test]
fn add_to_queue_sends_the_enqueue_command_for_the_selected_row() {
    let mut fixture = built(Some(movie_hero(GuiArtShapeFrame::Landscape)));

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
