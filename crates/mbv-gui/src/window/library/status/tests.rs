//! Headless tests of the Library Status bar's location (D16): the contracts
//! of `status.test.ts` — the destination, the selected collection names in
//! pill order, and the picked item's artist or title, joined by the canvas's
//! separator.

use super::*;
use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiCollectionEntry, GuiEmbyDetail, GuiEmbyList,
    GuiEmbyRowKindFrame, GuiFeedsDetail, GuiFeedsList, GuiHomeDetail, GuiHomeHeroFrame,
    GuiHomeList, GuiMusicAlbumFrame, GuiMusicDetail, GuiMusicList, GuiShowsDetail, GuiShowsList,
};

/// The fixture's tabs: Music at place 1, Audiobooks at place 2.
fn tabs() -> Vec<GuiLibraryTabFrame> {
    vec![
        GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        },
        GuiLibraryTabFrame {
            position: 2,
            label: "Audiobooks".to_owned(),
        },
    ]
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

/// The Music list part: the selector's checked groups and the carried names
/// the line reads (design D6).
fn music_list() -> GuiLibraryList {
    GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Music(GuiMusicList {
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
            collection_names: vec!["Hip-Hop".to_owned(), "Soul".to_owned()],
            loading: false,
            failure: None,
            artists: Vec::new(),
        }),
    }
}

/// The Music detail part with `picked` as its Hero.
fn music_detail(picked: Option<GuiMusicHeroFrame>) -> GuiLibraryDetail {
    GuiLibraryDetail::Music(GuiMusicDetail {
        hero: picked,
        art: None,
        playing_track_id: None,
    })
}

/// One plain-mode Emby row.
fn emby_row(id: &str, title: &str, sort_letter: Option<char>) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some("1979".to_owned()),
        date: None,
        sort_letter,
        kind: GuiEmbyRowKindFrame::Media,
    }
}

/// The plain mode's Emby library content (row 8.1).
fn emby_plain() -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Emby(GuiEmbyList {
            lib_idx: 2,
            library_name: "Movies".to_owned(),
            collections: None,
            collection_names: Vec::new(),
            feed_group: false,
            loading: false,
            failed: false,
            rows: vec![
                emby_row("m1", "Alien", Some('A')),
                emby_row("m2", "Heat", Some('H')),
            ],
        }),
    };
    let detail = GuiLibraryDetail::Emby(GuiEmbyDetail {
        selected: Some("m2".to_owned()),
        hero: None,
    });
    (list, detail)
}

/// The feed-group mode's Emby library content (row 8.1).
fn emby_feed() -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Emby(GuiEmbyList {
            lib_idx: 2,
            library_name: "Movies".to_owned(),
            collections: Some(GuiCollectionsFrame {
                target: mbv::GuiCollectionTarget::EmbyFeed(2),
                all: false,
                entries: vec![GuiCollectionEntry {
                    key: "f1".to_owned(),
                    name: "Home videos".to_owned(),
                    checked: true,
                }],
                noun: mbv::GuiCollectionNoun::Folders,
                order: Some(mbv::GuiDateOrder::Newest),
            }),
            collection_names: vec!["Home videos".to_owned()],
            feed_group: true,
            loading: false,
            failed: false,
            rows: vec![GuiEmbyRowFrame {
                id: "v1".to_owned(),
                title: "Holiday".to_owned(),
                subtitle: None,
                date: Some("2019".to_owned()),
                sort_letter: None,
                kind: GuiEmbyRowKindFrame::Media,
            }],
        }),
    };
    let detail = GuiLibraryDetail::Emby(GuiEmbyDetail {
        selected: Some("v1".to_owned()),
        hero: None,
    });
    (list, detail)
}

/// The Audiobooks content (row 9.1).
fn audiobooks_content(selected: Option<&str>) -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Audiobooks(GuiAudiobooksList {
            lib_idx: 1,
            loading: false,
            failed: false,
            rows: vec![
                GuiAudiobooksRowFrame {
                    id: "b1".to_owned(),
                    title: "Dune".to_owned(),
                    subtitle: Some("Frank Herbert".to_owned()),
                    duration: "15:12:00".to_owned(),
                    sort_letter: 'D',
                },
                GuiAudiobooksRowFrame {
                    id: "b2".to_owned(),
                    title: "Hyperion".to_owned(),
                    subtitle: Some("Dan Simmons".to_owned()),
                    duration: "9:18:00".to_owned(),
                    sort_letter: 'H',
                },
            ],
        }),
    };
    let detail = GuiLibraryDetail::Audiobooks(GuiAudiobooksDetail {
        workspace_loading: false,
        selected: selected.map(str::to_owned),
        playing_book_id: None,
        hero: None,
    });
    (list, detail)
}

/// The Feeds content (row 11.1).
fn feeds_content() -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Feeds(GuiFeedsList {
            collections: Some(GuiCollectionsFrame {
                target: mbv::GuiCollectionTarget::Feeds,
                all: false,
                entries: vec![GuiCollectionEntry {
                    key: "sub1".to_owned(),
                    name: "Bits and Bites".to_owned(),
                    checked: true,
                }],
                noun: mbv::GuiCollectionNoun::Subscriptions,
                order: Some(mbv::GuiDateOrder::Newest),
            }),
            loading: false,
            failed: false,
            rows: vec![GuiEntryRowFrame {
                id: "sub1:guid12".to_owned(),
                title: "The Long Road Home".to_owned(),
                subtitle: Some("Bits and Bites".to_owned()),
                date: Some("08 Oct".to_owned()),
                state: crate::bridge::frame::library::GuiHomeRowStateFrame::Ordinary,
            }],
        }),
    };
    let detail = GuiLibraryDetail::Feeds(GuiFeedsDetail {
        selected: Some("sub1:guid12".to_owned()),
        hero: None,
    });
    (list, detail)
}

/// The Home content (row 6.2).
fn home_content() -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Home(GuiHomeList {
            loading: false,
            rows: Vec::new(),
        }),
    };
    let detail = GuiLibraryDetail::Home(GuiHomeDetail {
        selected: None,
        hero: Some(GuiHomeHeroFrame {
            title: "Better Call Saul".to_owned(),
            meta_rows: Vec::new(),
            duration_row: None,
            progress_row: None,
            overview: None,
            art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Square,
            art: None,
        }),
    });
    (list, detail)
}

/// The Shows content (row 7.1).
fn shows_content() -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Shows(GuiShowsList {
            lib_idx: 0,
            collection_names: Vec::new(),
            loading: false,
            series: Vec::new(),
            upcoming_failed: false,
            rows: vec![
                GuiShowsRowFrame {
                    id: "s1".to_owned(),
                    title: "Atlanta".to_owned(),
                    subtitle: None,
                    date: None,
                    sort_letter: Some('A'),
                },
                GuiShowsRowFrame {
                    id: "s2".to_owned(),
                    title: "The Bear".to_owned(),
                    subtitle: None,
                    date: None,
                    sort_letter: Some('B'),
                },
            ],
        }),
    };
    let detail = GuiLibraryDetail::Shows(GuiShowsDetail {
        workspace_loading: false,
        selected: Some("s2".to_owned()),
        hero: None,
    });
    (list, detail)
}

/// One tab frame.
fn tab(position: usize, label: &str) -> GuiLibraryTabFrame {
    GuiLibraryTabFrame {
        position,
        label: label.to_owned(),
    }
}

// The location joins the destination, the selected collections, and the
// picked artist.
#[test]
fn the_location_joins_the_destination_the_selected_collections_and_the_picked_artist() {
    let list = music_list();
    assert_eq!(
        library_location(
            &tabs(),
            Some(1),
            Some(&list),
            Some(&music_detail(Some(hero())))
        ),
        "Music › Hip-Hop › Soul › Beastie Boys"
    );
}

// A library that holds no group selects none, so the location names no
// collection and leaves the segment out.
#[test]
fn a_library_with_no_group_names_no_collection() {
    let mut list = music_list();
    let GuiDestinationList::Music(music) = &mut list.destination else {
        panic!("music fixture");
    };
    music.collection_names.clear();
    let location = library_location(&tabs(), Some(1), Some(&list), Some(&music_detail(None)));
    assert!(!location.contains("All"));
}

// An unpicked Hero carries no item, so the location stops at the collections
// the canvas joins.
#[test]
fn an_unpicked_item_leaves_the_location_without_its_last_segment() {
    let list = music_list();
    assert_eq!(
        library_location(&tabs(), Some(1), Some(&list), Some(&music_detail(None))),
        "Music › Hip-Hop › Soul"
    );
}

// On a plain Emby library the location reads the destination tab's label and
// the selected row's title, and the plain mode's letter rail replaces the
// pill row, so no collection segment appears (design D10, D12, row 8.1).
#[test]
fn the_location_reads_the_plain_emby_destination_and_the_selected_row() {
    let (list, detail) = emby_plain();
    assert_eq!(
        library_location(&[tab(2, "Movies")], Some(2), Some(&list), Some(&detail)),
        "Movies › Heat"
    );
}

// A feed-group library names the selected feed pill between the destination
// and the row (design D10, D12, row 8.1).
#[test]
fn the_location_reads_the_feed_pill_name_in_feed_group_mode() {
    let (list, detail) = emby_feed();
    assert_eq!(
        library_location(
            &[tab(2, "Home videos")],
            Some(2),
            Some(&list),
            Some(&detail)
        ),
        "Home videos › Home videos › Holiday"
    );
}

// On an Audiobooks library the location reads the destination tab's label and
// the selected book's title, joined by the canvas's separator (design D10,
// D12, row 9.1).
#[test]
fn the_location_reads_the_audiobooks_destination_and_the_selected_book() {
    let (list, detail) = audiobooks_content(Some("b2"));
    assert_eq!(
        library_location(&tabs(), Some(2), Some(&list), Some(&detail)),
        "Audiobooks › Hyperion"
    );
}

// A frame whose selection names no carried book fills no last segment, so the
// location stops at the destination (design D10, D12).
#[test]
fn an_audiobooks_selection_with_no_carried_book_leaves_the_location_at_the_destination() {
    let (list, detail) = audiobooks_content(None);
    assert_eq!(
        library_location(&tabs(), Some(2), Some(&list), Some(&detail)),
        "Audiobooks"
    );
}

// On a Feeds destination the location reads the destination tab's label, the
// selected pill's name, and the selected entry's title, joined by the canvas's
// separator (design D10, D12, row 11.1).
#[test]
fn the_location_reads_the_feeds_destination_the_selected_pill_and_the_selected_entry() {
    let (list, detail) = feeds_content();
    assert_eq!(
        library_location(&[tab(5, "Feeds")], Some(5), Some(&list), Some(&detail)),
        "Feeds › Bits and Bites › The Long Road Home"
    );
}

// On Home the location reads `Home`, then the selected row's Hero title
// (design D10, D12, row 6.2).
#[test]
fn the_location_reads_home_and_the_home_hero_title() {
    let (list, detail) = home_content();
    assert_eq!(
        library_location(&tabs(), None, Some(&list), Some(&detail)),
        "Home › Better Call Saul"
    );
}

// On Shows the location reads the destination tab's label and the selected
// row's title: the destination paints no collection selector (design D10,
// D12, D8).
#[test]
fn the_location_reads_the_shows_destination_and_the_selected_row() {
    let (list, detail) = shows_content();
    assert_eq!(
        library_location(&[tab(3, "Shows")], Some(3), Some(&list), Some(&detail)),
        "Shows › The Bear"
    );
}
