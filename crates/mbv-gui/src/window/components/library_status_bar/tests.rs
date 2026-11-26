//! Headless tests of the Library Status bar (D16): the contracts of
//! `LibraryStatusBar.test.ts` — the location line, and the armed prefix
//! painting while the location stays (design D9 of
//! `move-playback-header-into-queue-footer`). The bar shows no toast: toasts
//! show as floating cards over the window (`gui-floating-toasts`).
//! The Search field's contracts are the top bar's Search control's now (user
//! request 2026-11-19): they moved with the field to its component's suite.
//! The location's own composition rules are `window/library/status/tests.rs`;
//! these tests own what the band paints from them. No test opens a window,
//! sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryStatusBar, LibraryStatusBarProps};
use crate::bridge::frame::chrome::GuiStatusBar;
use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiCollectionEntry,
    GuiCollections, GuiDestinationList, GuiEmbyDetail, GuiEmbyList, GuiEmbyRowFrame,
    GuiEmbyRowKindFrame, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsList, GuiHomeRowStateFrame,
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMusicAlbumFrame, GuiMusicDetail,
    GuiMusicHeroFrame, GuiMusicHeroSubjectFrame, GuiMusicList, GuiPodcastsDetail, GuiPodcastsList,
    GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::window::layout::LayoutEpoch;
use crate::window::sections::SectionRef;

/// The context handles the root hands the test: the layout epoch the
/// re-measures read. The bar consumes no store of its own — the Search facts
/// it once read are the top bar's control's now (user request 2026-11-19).
type FixtureHandles = LayoutEpoch;

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

/// The frame's Status bar facts: nothing armed.
fn idle() -> GuiStatusBar {
    GuiStatusBar {
        prefix_armed: false,
    }
}

/// The picked album's Hero facts (design D5).
fn hero() -> GuiMusicHeroFrame {
    GuiMusicHeroFrame {
        artist: "Beastie Boys".to_owned(),
        collections: vec!["Hip-Hop".to_owned()],
        album_count: 1,
        artist_tracks: None,
        subject: GuiMusicHeroSubjectFrame::Album(GuiMusicAlbumFrame {
            album_id: "al1".to_owned(),
            name: "Licensed To Ill".to_owned(),
            year: "1986".to_owned(),
            tracks: None,
            total: None,
        }),
        menu: Vec::new(),
    }
}

/// The Music list part: one checked collection.
fn music_list() -> GuiDestinationList {
    GuiDestinationList::Music(GuiMusicList {
        lib_idx: 0,
        collections: GuiCollections {
            target: crate::gui_core::GuiCollectionTarget::Music(0),
            all: false,
            entries: vec![GuiCollectionEntry {
                key: "g1".to_owned(),
                name: "Hip-Hop".to_owned(),
                checked: true,
            }],
            noun: crate::gui_core::GuiCollectionNoun::Groups,
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
            refreshing: false,
            tabs: vec![GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            }],
            current_tab: Some(1),
            destination: music_list(),
        })),
        detail: Some(SectionRef::new(music_detail())),
        status,
    }
}

/// The harness's props: the bar's own props, and the handle the test reads
/// the epoch through.
#[derive(Clone)]
struct FixtureProps {
    props: LibraryStatusBarProps,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store over the mock core link and paints
/// the bar with the section's carried Search facts, as the Library panel
/// composes it.
fn status_bar_app(props: FixtureProps) -> Element {
    let FixtureProps { props, handle } = props;
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    handle.borrow_mut().replace(epoch);
    let LibraryStatusBarProps {
        tabs,
        current_tab,
        list,
        detail,
        status,
    } = props;
    rsx! {
        LibraryStatusBar {
            tabs,
            current_tab,
            list,
            detail,
            status,
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
    epoch: LayoutEpoch,
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

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, so the `Truncate` measures and the dropdown's anchor read
    /// laid-out boxes, and let the re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
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
}

/// Build the document over the bar's props and let its first render and
/// effects land.
fn built(props: LibraryStatusBarProps) -> Fixture {
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            status_bar_app,
            FixtureProps {
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
    let epoch = handle
        .borrow_mut()
        .take()
        .expect("the root handed the epoch over");
    let mut fixture = Fixture { doc, epoch };
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
}

/// Contract (`LibraryStatusBar.test.ts`, the spec's "The Library Status bar
/// shows hints and the location"): the bar paints the location line — the
/// destination, the selected collection, and the picked album — and
/// no key hints. The Search field is the top bar's now (user request
/// 2026-11-19), so the bar paints no field and no hint.
#[test]
fn the_bar_paints_the_location_and_no_key_hints() {
    let doc = built(music_props(idle()));

    assert_eq!(
        doc.text_of("[data-library-status-location]"),
        "Music › Hip-Hop › Licensed To Ill"
    );
    assert!(
        !doc.presents("[data-library-search-input]"),
        "the field left the bar"
    );
    assert!(
        !doc.presents("[data-library-search-hint]"),
        "the chord's hint left the bar"
    );
    assert!(
        !doc.presents("[data-library-hint]"),
        "the canvas's key hints are gone"
    );
}

/// Contract (`LibraryStatusBar.test.ts`, spec `gui-client` "A toast
/// arrives"): the bar paints no toast node, and the location stays. No
/// geometry assertions (a test that pins a control's position broke CI twice
/// on presentation-only moves; user decision 2026-11-19).
#[test]
fn the_bar_shows_no_toast_and_keeps_the_location() {
    let doc = built(music_props(idle()));

    assert!(!doc.presents("[data-library-status-toast]"));
    assert!(
        doc.text_of("[data-library-status-location]")
            .contains("Music › Hip-Hop › Licensed To Ill"),
        "the location stays"
    );
}

/// Contract (`LibraryStatusBar.test.ts`): an armed prefix paints and keeps
/// the location. No geometry assertions (see the no-toast test's comment).
#[test]
fn an_armed_prefix_paints_and_keeps_the_location() {
    let doc = built(music_props(GuiStatusBar { prefix_armed: true }));

    assert!(
        doc.text_of("[data-library-status-prefix]")
            .contains("PREFIX"),
        "the prefix badge paints"
    );
    assert!(
        doc.text_of("[data-library-status-location]")
            .contains("Music › Hip-Hop › Licensed To Ill"),
        "the location stays"
    );
}

/// The carried plain Emby-library list part: one selected row.
fn emby_content() -> (GuiDestinationList, GuiLibraryDetail) {
    let list = GuiDestinationList::Emby(GuiEmbyList {
        sort: None,
        lib_idx: 2,
        library_name: "Movies".to_owned(),
        collections: None,
        collection_names: Vec::new(),
        feed_group: false,
        loading: false,
        failed: false,
        rows: vec![GuiEmbyRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
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
        sort: None,
        lib_idx: 1,
        loading: false,
        failed: false,
        rows: vec![GuiAudiobooksRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Audio,
            id: "b1".to_owned(),
            title: "Dune".to_owned(),
            subtitle: Some("Frank Herbert".to_owned()),
            duration: "15:12:00".to_owned(),
            sort_letter: Some('D'),
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
        sort: None,
        lib_idx: 1,
        collections: GuiCollections {
            target: crate::gui_core::GuiCollectionTarget::Podcasts(1),
            all: false,
            entries: vec![GuiCollectionEntry {
                key: "show1".to_owned(),
                name: "Bits and Bites".to_owned(),
                checked: true,
            }],
            noun: crate::gui_core::GuiCollectionNoun::Shows,
        },
        loading: false,
        failed: false,
        rows: vec![GuiEntryRowFrame {
            sort_letter: None,
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
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
            refreshing: false,
            tabs: vec![GuiLibraryTabFrame {
                position: 2,
                label: label.to_owned(),
            }],
            current_tab: Some(2),
            destination,
        })),
        detail: Some(SectionRef::new(detail)),
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
        sort: None,
        collections: GuiCollections {
            target: crate::gui_core::GuiCollectionTarget::Feeds,
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
            noun: crate::gui_core::GuiCollectionNoun::Subscriptions,
        },
        loading: false,
        failed: false,
        platform: None,
        rows: vec![GuiEntryRowFrame {
            sort_letter: None,
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "sub1:guid12".to_owned(),
            title: "The Long Road Home".to_owned(),
            subtitle: Some("Tifo Football".to_owned()),
            date: Some("08 Oct".to_owned()),
            state: GuiHomeRowStateFrame::Ordinary,
        }],
    });
    let detail = GuiLibraryDetail::Feeds(GuiFeedsDetail {
        playing_entry_id: None,
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
