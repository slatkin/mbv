//! Headless tests of the Library Status bar (D16): the contracts of
//! `LibraryStatusBar.test.ts` — the canvas's fixed key hints and the
//! location line, and the armed prefix or live toast replacing the hints
//! while the location stays. The location's own composition rules are
//! `window/library/status/tests.rs`; these tests own what the band paints
//! from them. No test opens a window, sleeps, or reaches the core.

use blitz_dom::{Document as _, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryStatusBar, LibraryStatusBarProps};
use crate::bridge::frame::chrome::{GuiStatusBar, GuiToast, GuiToastSeverity};
use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiCollectionEntry,
    GuiCollectionsFrame, GuiDestinationList, GuiEmbyDetail, GuiEmbyList, GuiEmbyRowFrame,
    GuiEmbyRowKindFrame, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsList, GuiHomeRowStateFrame,
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMusicAlbumFrame, GuiMusicDetail,
    GuiMusicHeroFrame, GuiMusicList, GuiPodcastsDetail, GuiPodcastsList,
};
use crate::window::sections::SectionRef;

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
        collections: Some(GuiCollectionsFrame {
            target: mbv::GuiCollectionTarget::Music(0),
            all: false,
            entries: vec![GuiCollectionEntry {
                key: "g1".to_owned(),
                name: "Hip-Hop".to_owned(),
                checked: true,
            }],
            noun: mbv::GuiCollectionNoun::Groups,
            order: None,
        }),
        collection_names: vec!["Hip-Hop".to_owned()],
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

/// The Music bar the first three tests render.
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
        status,
    }
}

/// The harness root: paints the bar over the props the test passes.
fn bar_app(props: LibraryStatusBarProps) -> Element {
    rsx! {
        LibraryStatusBar { ..props }
    }
}

/// Build the document over the bar's props and let its first render land.
fn built(props: LibraryStatusBarProps) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(bar_app, props),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    doc
}

/// The trimmed text of every `[data-library-hint]` node, in paint order.
fn hint_texts(doc: &DioxusDocument) -> Vec<String> {
    let dom = doc.inner();
    dom.query_selector_all("[data-library-hint]")
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

/// The trimmed text of the first node the selector names.
fn text_of(doc: &DioxusDocument, selector: &str) -> String {
    let dom = doc.inner();
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

/// Whether the selector names a node in the painted tree.
fn presents(doc: &DioxusDocument, selector: &str) -> bool {
    matches!(doc.inner().query_selector(selector), Ok(Some(_)))
}

/// The `style` attribute of the first node the selector names.
fn style_of(doc: &DioxusDocument, selector: &str) -> String {
    let dom = doc.inner();
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

/// Contract (`LibraryStatusBar.test.ts`): the bar paints the canvas's five
/// fixed key hints and the location line — the destination, the selected
/// collection, and the picked album's artist, joined by the canvas's
/// separator.
#[test]
fn the_bar_paints_the_canvas_key_hints_and_the_location() {
    let doc = built(music_props(idle()));

    assert_eq!(
        hint_texts(&doc),
        ["/ filter", "⏎ play", "n next", "a queue", "1–7 library",]
    );
    assert_eq!(
        text_of(&doc, "[data-library-status-location]"),
        "Music › Hip-Hop › Beastie Boys"
    );
}

/// Contract (`LibraryStatusBar.test.ts`): a live toast replaces the hints
/// and keeps the location.
#[test]
fn a_live_toast_replaces_the_hints_and_keeps_the_location() {
    let doc = built(music_props(GuiStatusBar {
        prefix_armed: false,
        toast: Some(GuiToast {
            text: "Scanning 'Movies'...".to_owned(),
            severity: GuiToastSeverity::Neutral,
        }),
    }));

    assert!(!presents(&doc, "[data-library-hint]"));
    assert!(
        text_of(&doc, "[data-library-status-toast]").contains("Scanning 'Movies'..."),
        "the toast text paints"
    );
    assert!(
        text_of(&doc, "[data-library-status-location]").contains("Music › Hip-Hop › Beastie Boys"),
        "the location stays"
    );
    // The toast paints in its severity colour (the shared `toast_color`).
    assert!(
        style_of(&doc, "[data-library-status-toast]").contains("color: var(--mbv-textPrimary)"),
        "the neutral toast paints its severity colour"
    );
}

/// Contract (`LibraryStatusBar.test.ts`): an armed prefix replaces the hints
/// and keeps the location.
#[test]
fn an_armed_prefix_replaces_the_hints_and_keeps_the_location() {
    let doc = built(music_props(GuiStatusBar {
        prefix_armed: true,
        toast: None,
    }));

    assert!(!presents(&doc, "[data-library-hint]"));
    assert!(
        text_of(&doc, "[data-library-status-prefix]").contains("PREFIX"),
        "the prefix badge paints"
    );
    assert!(
        text_of(&doc, "[data-library-status-location]").contains("Music › Hip-Hop › Beastie Boys"),
        "the location stays"
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
        collections: Some(GuiCollectionsFrame {
            target: mbv::GuiCollectionTarget::Podcasts(1),
            all: false,
            entries: vec![GuiCollectionEntry {
                key: "show1".to_owned(),
                name: "Bits and Bites".to_owned(),
                checked: true,
            }],
            noun: mbv::GuiCollectionNoun::Shows,
            order: Some(mbv::GuiDateOrder::Newest),
        }),
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
        text_of(&doc, "[data-library-status-location]"),
        "Movies › Heat"
    );
    assert_eq!(hint_texts(&doc).len(), 5);
}

/// Contract (`LibraryStatusBar.test.ts`): the bar's carried Audiobooks
/// content becomes the location while that destination is active (design
/// D10, D12).
#[test]
fn the_bar_paints_the_audiobooks_location_for_the_carried_content() {
    let doc = built(destination_props("Audiobooks", audiobooks_content()));

    assert_eq!(
        text_of(&doc, "[data-library-status-location]"),
        "Audiobooks › Dune"
    );
    assert_eq!(hint_texts(&doc).len(), 5);
}

/// The carried Feeds list part: two checked subscriptions and one selected
/// entry (spec "The Library Status bar shows hints and the location").
fn feeds_content() -> (GuiDestinationList, GuiLibraryDetail) {
    let list = GuiDestinationList::Feeds(GuiFeedsList {
        collections: Some(GuiCollectionsFrame {
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
        }),
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
        text_of(&doc, "[data-library-status-location]"),
        "Feeds › Tifo Football, Off Menu › The Long Road Home"
    );
    assert_eq!(hint_texts(&doc).len(), 5);
}

/// Contract (`LibraryStatusBar.test.ts`): the bar's carried Podcasts content
/// becomes the location while that destination is active (design D10, D12).
#[test]
fn the_bar_paints_the_podcasts_location_for_the_carried_content() {
    let doc = built(destination_props("Podcasts", podcasts_content()));

    assert_eq!(
        text_of(&doc, "[data-library-status-location]"),
        "Podcasts › Bits and Bites › Episode 12: The Long Road"
    );
    assert_eq!(hint_texts(&doc).len(), 5);
}
