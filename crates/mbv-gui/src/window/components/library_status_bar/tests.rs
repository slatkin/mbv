//! Headless tests of the Library Status bar (D16): the contracts of
//! `LibraryStatusBar.test.ts` — the Search field at the bar's left with its
//! `Ctrl K` hint, the results dropdown above it, the location line, and the
//! armed prefix or live toast painting after the field while the field and
//! the location stay (design D9 of `move-playback-header-into-queue-footer`).
//! The field's mount, focus, and highlight contracts moved here with the field
//! from the top bar's suite. The location's own composition rules are
//! `window/library/status/tests.rs`; these tests own what the band paints from
//! them. No test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryStatusBar, LibraryStatusBarProps};
use crate::bridge::frame::chrome::{GuiStatusBar, GuiToast, GuiToastSeverity};
use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiCollectionEntry,
    GuiCollections, GuiDestinationList, GuiEmbyDetail, GuiEmbyList, GuiEmbyRowFrame,
    GuiEmbyRowKindFrame, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsList, GuiHomeRowStateFrame,
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMusicAlbumFrame, GuiMusicDetail,
    GuiMusicHeroFrame, GuiMusicList, GuiPodcastsDetail, GuiPodcastsList, GuiSearchFrame,
    GuiSearchRowFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The context handles the root hands the test: the Library store, the
/// conditional-mount toggle the unmount test drives, and the layout epoch the
/// re-measures read.
type FixtureHandles = (LibraryState, Signal<bool>, LayoutEpoch);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the bar tree
/// sizes its boxes with (the same rules the player panel's harness uses), so
/// the `Truncate` measures and the dropdown's anchor read laid-out boxes.
const FIXTURE_CSS: &str = concat!(
    "[data-slot] { width: 100%; }",
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".justify-center { justify-content: center; }",
    ".justify-between { justify-content: space-between; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".shrink-0 { flex-shrink: 0; }",
    ".w-full { width: 100%; }",
    ".h-full { height: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
);

/// The frame's Status bar facts: nothing armed and no live toast.
fn idle() -> GuiStatusBar {
    GuiStatusBar {
        prefix_armed: false,
        toast: None,
    }
}

/// The picked album's Hero facts (design D5).
fn hero() -> GuiMusicHeroFrame {
    GuiMusicHeroFrame {
        artist: "Beastie Boys".to_owned(),
        collections: vec!["Hip-Hop".to_owned()],
        album_count: 1,
        artist_tracks: None,
        album: GuiMusicAlbumFrame {
            album_id: "al1".to_owned(),
            name: "Licensed To Ill".to_owned(),
            year: "1986".to_owned(),
            tracks: None,
            total: None,
        },
        menu: Vec::new(),
    }
}

/// The Music list part: one checked collection.
fn music_list() -> GuiDestinationList {
    GuiDestinationList::Music(GuiMusicList {
        lib_idx: 0,
        collections: GuiCollections {
            target: mbv::GuiCollectionTarget::Music(0),
            all: false,
            entries: vec![GuiCollectionEntry {
                key: "g1".to_owned(),
                name: "Hip-Hop".to_owned(),
                checked: true,
            }],
            noun: mbv::GuiCollectionNoun::Groups,
            order: None,
        },
        loading: false,
        failure: None,
        artists: Vec::new(),
    })
}

/// The Music detail part with the picked album.
fn music_detail() -> GuiLibraryDetail {
    GuiLibraryDetail::Music(GuiMusicDetail {
        hero: Some(hero()),
        art: None,
        playing_track_id: None,
    })
}

/// The Music bar the location tests render.
fn music_props(status: GuiStatusBar) -> LibraryStatusBarProps {
    LibraryStatusBarProps {
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: Some(1),
        list: Some(SectionRef::new(GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            }],
            current_tab: Some(1),
            destination: music_list(),
        })),
        detail: Some(SectionRef::new(music_detail())),
        search: None,
        status,
    }
}

/// One found row, as the frame carries it.
fn row(id: &str, title: &str, subtitle: &str) -> GuiSearchRowFrame {
    GuiSearchRowFrame {
        id: id.to_owned(),
        item_type: if id == "r2" { "Album" } else { "Artist" }.to_owned(),
        title: title.to_owned(),
        subtitle: subtitle.to_owned(),
    }
}

/// Two found rows: the dropdown's rows in the list-row look. Each call
/// builds fresh row values that share only ids: the highlight must never
/// depend on object identity.
fn results() -> GuiSearchFrame {
    GuiSearchFrame {
        query: "ba".to_owned(),
        loading: false,
        failed: false,
        results: vec![
            row("r1", "Beastie Boys", "Artist"),
            row("r2", "Licensed To Ill", "Beastie Boys"),
        ],
    }
}

/// The harness's props: the bar's own props, the mock core sender, and the
/// handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    props: LibraryStatusBarProps,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store over the mock core link and paints
/// the bar with the section's carried Search facts, as the Library panel
/// composes it.
fn status_bar_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        props,
        handle,
    } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let state = use_hook(move || {
        // The playhead position the Search state reads: none — the tests
        // hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(sender),
            playhead_position,
            Arc::clone(&clock),
        )
    });
    provide_context(state.clone());
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let show = use_signal(|| true);
    handle.borrow_mut().replace((state.clone(), show, epoch));
    let search = state.sections.search.cloned();
    let LibraryStatusBarProps {
        tabs,
        current_tab,
        list,
        detail,
        status,
        ..
    } = props;
    if show.cloned() {
        rsx! {
            LibraryStatusBar {
                tabs,
                current_tab,
                list,
                detail,
                search,
                status,
            }
        }
    } else {
        rsx! {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    epoch: LayoutEpoch,
    show: Signal<bool>,
    /// The mock core channel's receiver, held so the link's sends land: the
    /// field's own commands are the state layer's contracts
    /// (`window/library/search/tests.rs`).
    _receiver: mpsc::Receiver<CoreInput>,
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
    /// their subscribers.
    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, so the `Truncate` measures and the dropdown's anchor read
    /// laid-out boxes, and let the re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Carry a Search frame on the section, as the core's batch would, and
    /// resolve the dropdown's laid-out boxes.
    fn set_search_frame(&mut self, frame: Option<GuiSearchFrame>) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || sections.search.set(frame));
        self.lay_out();
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
    fn attrs_of(&self, selector: &str, attribute: &blitz_dom::LocalName) -> Vec<Option<String>> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .map(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .attr(attribute.clone())
                    .map(str::to_owned)
            })
            .collect()
    }

    /// The `style` attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .expect("the node paints");
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(local_name!("style"))
            .map(str::to_owned)
            .unwrap_or_default()
    }

    /// The laid-out client rect of the first node the selector names.
    fn rect_of(&self, selector: &str) -> blitz_dom::BoundingRect {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_client_bounding_rect(id)
            .expect("the node is laid out")
    }
}

/// Build the document over the bar's props and let its first render and
/// effects land.
fn built(props: LibraryStatusBarProps) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            status_bar_app,
            FixtureProps {
                sender,
                props,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let (state, show, epoch) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        epoch,
        show,
        _receiver: receiver,
    };
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
}

/// Contract (`LibraryStatusBar.test.ts`, the spec's "The Library Status bar
/// shows hints and the location"): the bar paints the Search field with the
/// `Ctrl K` hint and the location line — the destination, the selected
/// collection, and the picked album's artist — and no key hints.
#[test]
fn the_bar_paints_the_search_field_and_the_location() {
    let doc = built(music_props(idle()));

    assert!(
        doc.presents("[data-library-search-input]"),
        "the field paints at the bar's left"
    );
    assert_eq!(doc.text_of("[data-library-search-hint]"), "Ctrl K");
    assert_eq!(
        doc.text_of("[data-library-status-location]"),
        "Music › Hip-Hop › Beastie Boys"
    );
    assert!(
        !doc.presents("[data-library-hint]"),
        "the canvas's key hints are gone"
    );
}

/// Contract (`LibraryStatusBar.test.ts`): a live toast paints after the
/// Search field and keeps the field and the location.
#[test]
fn a_live_toast_paints_after_the_search_field_and_keeps_the_location() {
    let doc = built(music_props(GuiStatusBar {
        prefix_armed: false,
        toast: Some(GuiToast {
            text: "Scanning 'Movies'...".to_owned(),
            severity: GuiToastSeverity::Neutral,
        }),
    }));

    assert!(
        doc.text_of("[data-library-status-toast]")
            .contains("Scanning 'Movies'..."),
        "the toast text paints"
    );
    assert!(
        doc.text_of("[data-library-status-location]")
            .contains("Music › Hip-Hop › Beastie Boys"),
        "the location stays"
    );
    // The toast paints in its severity colour (the shared `toast_color`).
    assert!(
        doc.style_of("[data-library-status-toast]")
            .contains("color: var(--mbv-textPrimary)"),
        "the neutral toast paints its severity colour"
    );
    // The field stays, and the toast sits to its right (design D9).
    assert!(
        doc.presents("[data-library-search-input]"),
        "the field stays"
    );
    assert!(
        doc.rect_of("[data-library-status-toast]").x
            >= doc.rect_of("[data-library-search]").x + doc.rect_of("[data-library-search]").width,
        "the toast paints after the field"
    );
}

/// Contract (`LibraryStatusBar.test.ts`): an armed prefix paints after the
/// Search field and keeps the field and the location.
#[test]
fn an_armed_prefix_paints_after_the_search_field_and_keeps_the_location() {
    let doc = built(music_props(GuiStatusBar {
        prefix_armed: true,
        toast: None,
    }));

    assert!(
        doc.text_of("[data-library-status-prefix]")
            .contains("PREFIX"),
        "the prefix badge paints"
    );
    assert!(
        doc.text_of("[data-library-status-location]")
            .contains("Music › Hip-Hop › Beastie Boys"),
        "the location stays"
    );
    assert!(
        doc.presents("[data-library-search-input]"),
        "the field stays"
    );
    assert!(
        doc.rect_of("[data-library-status-prefix]").x
            >= doc.rect_of("[data-library-search]").x + doc.rect_of("[data-library-search]").width,
        "the prefix paints after the field"
    );
}

/// The carried plain Emby-library list part: one selected row.
fn emby_content() -> (GuiDestinationList, GuiLibraryDetail) {
    let list = GuiDestinationList::Emby(GuiEmbyList {
        lib_idx: 2,
        library_name: "Movies".to_owned(),
        collections: None,
        collection_names: Vec::new(),
        feed_group: false,
        loading: false,
        failed: false,
        rows: vec![GuiEmbyRowFrame {
            id: "m1".to_owned(),
            title: "Heat".to_owned(),
            subtitle: Some("1995".to_owned()),
            date: None,
            sort_letter: Some('H'),
            kind: GuiEmbyRowKindFrame::Media,
        }],
    });
    let detail = GuiLibraryDetail::Emby(GuiEmbyDetail {
        selected: Some("m1".to_owned()),
        hero: None,
    });
    (list, detail)
}

/// The carried Audiobooks list part: one selected book.
fn audiobooks_content() -> (GuiDestinationList, GuiLibraryDetail) {
    let list = GuiDestinationList::Audiobooks(GuiAudiobooksList {
        lib_idx: 1,
        loading: false,
        failed: false,
        rows: vec![GuiAudiobooksRowFrame {
            id: "b1".to_owned(),
            title: "Dune".to_owned(),
            subtitle: Some("Frank Herbert".to_owned()),
            duration: "15:12:00".to_owned(),
            sort_letter: 'D',
        }],
    });
    let detail = GuiLibraryDetail::Audiobooks(GuiAudiobooksDetail {
        workspace_loading: false,
        selected: Some("b1".to_owned()),
        playing_book_id: None,
        hero: None,
    });
    (list, detail)
}

/// The carried Podcasts list part: one checked show and one selected episode
/// row.
fn podcasts_content() -> (GuiDestinationList, GuiLibraryDetail) {
    let list = GuiDestinationList::Podcasts(GuiPodcastsList {
        lib_idx: 1,
        collections: GuiCollections {
            target: mbv::GuiCollectionTarget::Podcasts(1),
            all: false,
            entries: vec![GuiCollectionEntry {
                key: "show1".to_owned(),
                name: "Bits and Bites".to_owned(),
                checked: true,
            }],
            noun: mbv::GuiCollectionNoun::Shows,
            order: Some(mbv::GuiDateOrder::Newest),
        },
        loading: false,
        failed: false,
        rows: vec![GuiEntryRowFrame {
            id: "show1:ep12".to_owned(),
            title: "Episode 12: The Long Road".to_owned(),
            subtitle: Some("Bits and Bites".to_owned()),
            date: Some("08 Oct".to_owned()),
            state: GuiHomeRowStateFrame::Ordinary,
        }],
    });
    let detail = GuiLibraryDetail::Podcasts(GuiPodcastsDetail {
        selected: Some("show1:ep12".to_owned()),
        hero: None,
    });
    (list, detail)
}

/// The bar over a carried destination's content, under the tab `label`.
fn destination_props(
    label: &str,
    content: (GuiDestinationList, GuiLibraryDetail),
) -> LibraryStatusBarProps {
    let (destination, detail) = content;
    LibraryStatusBarProps {
        tabs: vec![GuiLibraryTabFrame {
            position: 2,
            label: label.to_owned(),
        }],
        current_tab: Some(2),
        list: Some(SectionRef::new(GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 2,
                label: label.to_owned(),
            }],
            current_tab: Some(2),
            destination,
        })),
        detail: Some(SectionRef::new(detail)),
        search: None,
        status: idle(),
    }
}

/// Contract (`LibraryStatusBar.test.ts`): the bar's carried plain
/// Emby-library content becomes the location while that destination is
/// active (design D10, D12).
#[test]
fn the_bar_paints_the_plain_emby_location_for_the_carried_content() {
    let doc = built(destination_props("Movies", emby_content()));

    assert_eq!(
        doc.text_of("[data-library-status-location]"),
        "Movies › Heat"
    );
}

/// Contract (`LibraryStatusBar.test.ts`): the bar's carried Audiobooks
/// content becomes the location while that destination is active (design
/// D10, D12).
#[test]
fn the_bar_paints_the_audiobooks_location_for_the_carried_content() {
    let doc = built(destination_props("Audiobooks", audiobooks_content()));

    assert_eq!(
        doc.text_of("[data-library-status-location]"),
        "Audiobooks › Dune"
    );
}

/// The carried Feeds list part: two checked subscriptions and one selected
/// entry (spec "The Library Status bar shows hints and the location").
fn feeds_content() -> (GuiDestinationList, GuiLibraryDetail) {
    let list = GuiDestinationList::Feeds(GuiFeedsList {
        collections: GuiCollections {
            target: mbv::GuiCollectionTarget::Feeds,
            all: false,
            entries: vec![
                GuiCollectionEntry {
                    key: "sub1".to_owned(),
                    name: "Tifo Football".to_owned(),
                    checked: true,
                },
                GuiCollectionEntry {
                    key: "sub2".to_owned(),
                    name: "Off Menu".to_owned(),
                    checked: true,
                },
            ],
            noun: mbv::GuiCollectionNoun::Subscriptions,
            order: Some(mbv::GuiDateOrder::Newest),
        },
        loading: false,
        failed: false,
        rows: vec![GuiEntryRowFrame {
            id: "sub1:guid12".to_owned(),
            title: "The Long Road Home".to_owned(),
            subtitle: Some("Tifo Football".to_owned()),
            date: Some("08 Oct".to_owned()),
            state: GuiHomeRowStateFrame::Ordinary,
        }],
    });
    let detail = GuiLibraryDetail::Feeds(GuiFeedsDetail {
        selected: Some("sub1:guid12".to_owned()),
        hero: None,
    });
    (list, detail)
}

/// Contract (spec "The Library Status bar shows hints and the location"): two
/// checked subscriptions both name in the location, between the destination
/// and the picked entry.
#[test]
fn the_bar_names_every_checked_subscription_in_the_location() {
    let doc = built(destination_props("Feeds", feeds_content()));

    assert_eq!(
        doc.text_of("[data-library-status-location]"),
        "Feeds › Tifo Football, Off Menu › The Long Road Home"
    );
}

/// Contract (`LibraryStatusBar.test.ts`): the bar's carried Podcasts content
/// becomes the location while that destination is active (design D10, D12).
#[test]
fn the_bar_paints_the_podcasts_location_for_the_carried_content() {
    let doc = built(destination_props("Podcasts", podcasts_content()));

    assert_eq!(
        doc.text_of("[data-library-status-location]"),
        "Podcasts › Bits and Bites › Episode 12: The Long Road"
    );
}

mod search;
