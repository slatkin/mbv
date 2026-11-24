//! Headless tests of the Shows Hero (D16): the contracts of
//! `LibraryShowsHero.test.ts` — the artwork box per art shape, the series in
//! the header band and the selected season's detail beside its poster, the
//! season dropdown naming the core's selection, the selected season's
//! episode rows with their played state, the Latest mode's Hero with no
//! Workspace, and the commands the season press, the episode gestures, and
//! the Hero's buttons send. The IPC of the earlier tests is the mock
//! core link here; the asserted commands are the same neutral shapes. A
//! headless document cannot fire a DOM click, so the press contracts ride the
//! handler functions the component's own handlers call one line each. The
//! core channel is a mock pair; no test opens a window, sleeps, or reaches
//! the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{
    LibraryShowsHero, ShowsAction, act_on, action_target, play_episode, queue_episode,
    select_season,
};
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiHomeRowStateFrame, GuiLibraryDetail, GuiMetaKindFrame,
    GuiMetaSegmentFrame, GuiShowsDetail, GuiShowsEpisodeRowFrame, GuiShowsHeroFrame,
    GuiShowsSeasonPillFrame, GuiShowsWorkspaceFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::library::parts::library_shows_detail;
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The library index the Shows commands name.
const LIB_IDX: usize = 2;

/// The selected season's episodes: one played, one untouched.
fn episodes() -> Vec<GuiShowsEpisodeRowFrame> {
    vec![
        GuiShowsEpisodeRowFrame {
            episode_id: "ep1".to_owned(),
            number: 1,
            name: "Pilot".to_owned(),
            duration: "58:00".to_owned(),
            state: GuiHomeRowStateFrame::Played,
        },
        GuiShowsEpisodeRowFrame {
            episode_id: "ep2".to_owned(),
            number: 2,
            name: "Cat in the Bag".to_owned(),
            duration: "47:00".to_owned(),
            state: GuiHomeRowStateFrame::Ordinary,
        },
    ]
}

/// The series Hero's Workspace: two seasons, the first selected.
fn workspace() -> GuiShowsWorkspaceFrame {
    GuiShowsWorkspaceFrame {
        seasons: vec![
            GuiShowsSeasonPillFrame {
                season_id: "s1".to_owned(),
                name: "Season 1".to_owned(),
                selected: true,
            },
            GuiShowsSeasonPillFrame {
                season_id: "s2".to_owned(),
                name: "Season 2".to_owned(),
                selected: false,
            },
        ],
        selected_season: Some("s1".to_owned()),
        episodes: episodes(),
    }
}

/// The selected row's Hero facts: the series' facts with its Workspace.
fn hero(overrides: impl FnOnce(&mut GuiShowsHeroFrame)) -> GuiShowsHeroFrame {
    let mut facts = GuiShowsHeroFrame {
        title: "Better Call Saul".to_owned(),
        parent: None,
        meta_rows: vec![
            vec![GuiMetaSegmentFrame {
                text: "TV Series".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            }],
            vec![GuiMetaSegmentFrame {
                text: "5 seasons".to_owned(),
                kind: GuiMetaKindFrame::Count,
            }],
        ],
        duration_row: None,
        progress_row: None,
        overview: Some("A lawyer turns to the wrong clients.".to_owned()),
        art_shape: GuiArtShapeFrame::Landscape,
        art: None,
        workspace: Some(workspace()),
    };
    overrides(&mut facts);
    facts
}

/// The Shows detail part: the series row selected, with the Hero `facts`.
fn detail(facts: Option<GuiShowsHeroFrame>) -> GuiShowsDetail {
    GuiShowsDetail {
        workspace_loading: false,
        selected: Some("series1".to_owned()),
        hero: facts,
    }
}

/// The harness's props: the carried Shows content, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    detail: SectionRef<GuiLibraryDetail>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the Library store and the
/// Shows detail the presses read.
type FixtureHandles = (LibraryState, SectionRef<GuiLibraryDetail>);

/// The root: provides the Library store and paints the Hero, as the panel
/// composes it.
fn shows_hero_app(props: FixtureProps) -> Element {
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
    handle.borrow_mut().replace((state, detail.clone()));
    rsx! {
        LibraryShowsHero { lib_idx: LIB_IDX, detail }
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
fn built(facts: Option<GuiShowsHeroFrame>) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            shows_hero_app,
            FixtureProps {
                sender,
                detail: SectionRef::new(GuiLibraryDetail::Shows(detail(facts))),
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

/// The Shows Hero mounted for the selected series.
fn built_series() -> Fixture {
    built(Some(hero(|_| ())))
}

/// Contract (`LibraryShowsHero.test.ts`, design D14): the artwork box
/// reserves the arm the frame declares — a square or portrait cover keeps the
/// canvas's 300 px box beside the text.
#[rstest]
#[case::square(GuiArtShapeFrame::Square, "300px")]
#[case::portrait(GuiArtShapeFrame::Portrait, "450px")]
fn a_cover_hero_places_its_artwork_box_at_300_px_wide(
    #[case] shape: GuiArtShapeFrame,
    #[case] height: &str,
) {
    let fixture = built(Some(hero(|facts| facts.art_shape = shape)));

    let style = fixture.style_of("[data-library-shows-hero-art]");
    assert!(
        style.contains("width: 300px") && style.contains(&format!("height: {height}")),
        "the box holds 300 px by {height}, painted: {style}"
    );
}

/// Regression (2026-11-19, the TV Hero was not responsive): a poster (portrait
/// or square) Hero wraps its text below the art when the Hero is too narrow
/// for both, as a landscape Hero does. The detail block takes `flex-wrap` and
/// the text a 320 px basis for every art shape.
#[rstest]
#[case::square(GuiArtShapeFrame::Square)]
#[case::portrait(GuiArtShapeFrame::Portrait)]
#[case::landscape(GuiArtShapeFrame::Landscape)]
fn the_detail_block_wraps_its_text_below_the_art_for_every_shape(#[case] shape: GuiArtShapeFrame) {
    let fixture = built(Some(hero(|facts| facts.art_shape = shape)));

    let class = fixture
        .attr_of("[data-library-shows-hero-block]", "class")
        .unwrap_or_default();
    assert!(
        class.contains("flex-wrap"),
        "the block wraps, class: {class}"
    );
    let text = fixture.style_of("[data-library-shows-hero-block] > div:last-child");
    assert!(
        text.contains("flex-basis: 320px"),
        "the text holds a 320 px basis, painted: {text}"
    );
}

/// Contract (`LibraryShowsHero.test.ts`, design D14): the landscape arm is
/// the TUI's — the still fills the Hero content width at 16:9 above the
/// facts.
#[test]
fn a_landscape_hero_fills_the_hero_content_width_at_16_9() {
    let fixture = built_series();

    let style = fixture.style_of("[data-library-shows-hero-art]");
    assert!(
        style.contains("width: 100%")
            && style.contains("aspect-ratio: 16 / 9")
            && style.contains("height: auto"),
        "the landscape still fills the content width at 16:9, painted: {style}"
    );
}

/// Contract (`LibraryShowsHero.test.ts`): the Hero paints the series in the
/// header band and the series' detail beside its poster — the selected season
/// where Music paints the album, and the series' overview under it.
#[test]
fn the_hero_paints_the_series_in_the_header_and_the_season_s_detail_beside_its_poster() {
    let fixture = built_series();

    assert_eq!(
        fixture.text_of("[data-library-shows-hero-kicker]"),
        "series",
        "the kicker is the series label"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-hero-title]"),
        "Better Call Saul",
        "the title is the series"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-hero-facts]"),
        "TV Series · 5 seasons",
        "the header's facts join the series' meta rows"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-hero-block-name]"),
        "Season 1",
        "the block's name is the selected season"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-hero-block-facts]"),
        "2 episodes",
        "the block's facts are the selected season's episode count"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-hero-overview]"),
        "A lawyer turns to the wrong clients.",
        "the overview paints under the block"
    );
}

/// Contract (user request 2026-11-20): the Workspace paints one season
/// dropdown, not a pill per season. Its trigger names the core's selected
/// season, and the closed popover paints no season row.
#[test]
fn the_workspace_paints_one_season_dropdown_naming_the_core_selection() {
    let fixture = built_series();

    assert_eq!(
        fixture.all_texts("[data-library-shows-seasons-trigger]"),
        vec!["Season 1".to_owned()],
        "the trigger names the core's selected season"
    );
    assert!(
        fixture.all_texts("[data-library-shows-season]").is_empty(),
        "the closed dropdown paints no season row"
    );
}

/// Contract (`LibraryShowsHero.test.ts`): the Workspace paints the selected
/// season's episodes with their number, name, duration, and played state.
#[test]
fn the_workspace_paints_the_selected_season_s_episodes() {
    let fixture = built_series();

    assert_eq!(
        fixture.all_attrs("[data-library-shows-episode]", "data-library-shows-episode"),
        vec!["ep1".to_owned(), "ep2".to_owned()],
        "the episodes paint in their fetch order"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-episode=\"ep2\"] [data-library-shows-episode-number]"),
        "2",
        "the episode's number paints"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-episode=\"ep2\"] [data-library-shows-episode-name]"),
        "Cat in the Bag",
        "the episode's name paints"
    );
    assert_eq!(
        fixture
            .text_of("[data-library-shows-episode=\"ep2\"] [data-library-shows-episode-duration]"),
        "47:00",
        "the episode's duration paints"
    );
    assert_eq!(
        fixture.attr_of(
            "[data-library-shows-episode=\"ep1\"]",
            "data-library-shows-episode-state"
        ),
        Some("played".to_owned()),
        "the played episode carries the played state"
    );
}

/// Contract (`LibraryShowsHero.test.ts`): the Latest mode's Hero paints its
/// facts and no Workspace.
#[test]
fn the_latest_mode_s_hero_paints_its_facts_and_no_workspace() {
    let fixture = built(Some(hero(|facts| facts.workspace = None)));

    assert_eq!(
        fixture.text_of("[data-library-shows-hero-kicker]"),
        "episode",
        "the Latest Hero's kicker is the episode label"
    );
    assert_eq!(
        fixture.text_of("[data-library-shows-hero-title]"),
        "Better Call Saul",
        "the Latest Hero's title is the episode's facts' title"
    );
    assert!(
        !fixture.presents("[data-library-shows-workspace]"),
        "no Workspace paints"
    );
}

/// Contract (`LibraryShowsHero.test.ts`): a Hero that paints none paints
/// nothing.
#[test]
fn a_hero_that_paints_none_paints_nothing() {
    let fixture = built(None);

    assert!(
        !fixture.presents("[data-library-shows-hero]"),
        "no Hero paints"
    );
}

/// Contract (`LibraryShowsHero.test.ts`): a season press sends the
/// select-season command. The press contract rides the handler function the
/// component's own handler calls (a headless document cannot fire a DOM
/// click).
#[test]
fn a_season_press_sends_the_select_season_command() {
    let mut fixture = built_series();

    let state = fixture.state.clone();
    fixture.in_runtime(move || select_season(&state, LIB_IDX, "s2"));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::ShowsSelectSeason {
            lib_idx: LIB_IDX,
            season_id: "s2".to_owned(),
        }],
        "the select-season command names the pressed season"
    );
}

/// Contract (`LibraryShowsHero.test.ts`): a double-click on an episode sends
/// its play command. The press contract rides the handler function the
/// component's own handler calls (a headless document cannot fire a DOM
/// double-click).
#[test]
fn a_double_click_on_an_episode_sends_its_play_command() {
    let mut fixture = built_series();

    let state = fixture.state.clone();
    fixture.in_runtime(move || play_episode(&state, LIB_IDX, "ep2"));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::ShowsPlay {
            lib_idx: LIB_IDX,
            item_id: "ep2".to_owned(),
        }],
        "the play command names the played episode"
    );
}

/// Contract (`LibraryShowsHero.test.ts`): a hovered row adds its episode to
/// the queue. The press contract rides the handler function the row's action
/// closure calls (a headless document cannot fire a hover).
#[test]
fn a_hovered_row_adds_its_episode_to_the_queue() {
    let mut fixture = built_series();

    let state = fixture.state.clone();
    fixture.in_runtime(move || queue_episode(&state, LIB_IDX, "ep2"));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::ShowsQueue {
            lib_idx: LIB_IDX,
            item_id: "ep2".to_owned(),
        }],
        "the enqueue command names the hovered episode"
    );
}

/// Contract (`LibraryShowsHero.test.ts`, Q6 of the Hero look audit): the
/// series Hero's Play sends the play command for the selected season — a
/// season is a folder the core's shared folder paths expand. The press
/// contract rides the handler function the component's own handler calls.
#[test]
fn the_series_hero_play_sends_the_play_command_for_the_selected_season() {
    let mut fixture = built_series();

    let detail =
        library_shows_detail(Some(&fixture.detail)).expect("the fixture carries a Shows detail");
    let target = action_target(detail);
    let state = fixture.state.clone();
    fixture.in_runtime(move || act_on(&state, ShowsAction::Play, LIB_IDX, target.as_deref()));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::ShowsPlay {
            lib_idx: LIB_IDX,
            item_id: "s1".to_owned(),
        }],
        "the play command names the selected season"
    );
}

/// Contract (`LibraryShowsHero.test.ts`, Q6 of the Hero look audit): the
/// Latest Hero's Add to queue sends the queue command for the selected
/// episode. The press contract rides the handler function the component's own
/// handler calls.
#[test]
fn the_latest_hero_add_to_queue_sends_the_queue_command_for_the_selected_episode() {
    let mut fixture = built(Some(hero(|facts| facts.workspace = None)));

    let detail =
        library_shows_detail(Some(&fixture.detail)).expect("the fixture carries a Shows detail");
    let target = action_target(detail);
    let state = fixture.state.clone();
    fixture.in_runtime(move || act_on(&state, ShowsAction::Queue, LIB_IDX, target.as_deref()));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::ShowsQueue {
            lib_idx: LIB_IDX,
            item_id: "series1".to_owned(),
        }],
        "the enqueue command names the frame's selected episode"
    );
}
