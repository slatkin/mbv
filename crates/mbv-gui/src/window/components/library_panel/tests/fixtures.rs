//! The carried-section fixtures the panel's test modules share: the port of
//! `LibraryPanel.test.ts`'s fixture builders, one per destination, with the
//! Hero facts each mount contract needs.

use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksHeroFrame, GuiAudiobooksList, GuiAudiobooksRowFrame,
    GuiAudiobooksWorkspaceFrame, GuiCollectionEntry, GuiCollections, GuiDestinationList,
    GuiEmbyDetail, GuiEmbyHeroFrame, GuiEmbyList, GuiEmbyRowFrame, GuiEmbyRowKindFrame,
    GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsHeroFrame, GuiFeedsList, GuiHomeDetail,
    GuiHomeHeroFrame, GuiHomeItemRowFrame, GuiHomeList, GuiHomeRowFrame, GuiHomeRowIdFrame,
    GuiHomeRowKindFrame, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryTabFrame, GuiMetaKindFrame, GuiMetaSegmentFrame, GuiMusicAlbumFrame, GuiMusicDetail,
    GuiMusicHeroFrame, GuiMusicList, GuiRowArtFrame, GuiRowMediaKindFrame, GuiShowsDetail,
    GuiShowsHeroFrame, GuiShowsList, GuiShowsRowFrame,
};

/// The carried Music list part with no album picked.
pub(super) fn music_list() -> GuiMusicList {
    GuiMusicList {
        lib_idx: 0,
        collections: GuiCollections {
            target: mbv::GuiCollectionTarget::Music(0),
            all: true,
            entries: Vec::new(),
            noun: mbv::GuiCollectionNoun::Groups,
            order: None,
        },
        loading: false,
        failure: None,
        artists: Vec::new(),
    }
}

/// The carried Music sections, with `picked` as the Hero's album.
pub(super) fn music_sections(
    picked: Option<GuiMusicHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    (
        GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            }],
            current_tab: Some(1),
            destination: GuiDestinationList::Music(music_list()),
        },
        GuiLibraryDetail::Music(GuiMusicDetail {
            hero: picked,
            art: None,
            playing_track_id: None,
        }),
    )
}

/// The picked album's Hero facts: the album block a pick fills (design D5).
pub(super) fn music_hero() -> GuiMusicHeroFrame {
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

/// A meta segment the fixtures' Hero rows carry.
pub(super) fn segment(text: &str, kind: GuiMetaKindFrame) -> Vec<GuiMetaSegmentFrame> {
    vec![GuiMetaSegmentFrame {
        text: text.to_owned(),
        kind,
    }]
}

/// The carried Home sections: one item row and its Hero.
pub(super) fn home_sections(with_hero: bool) -> (GuiLibraryList, GuiLibraryDetail) {
    let row_id = GuiHomeRowIdFrame::Continue("e1".to_owned());
    (
        GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            }],
            current_tab: None,
            destination: GuiDestinationList::Home(GuiHomeList {
                loading: false,
                rows: vec![
                    GuiHomeRowFrame::Heading {
                        text: "Continue watching".to_owned(),
                    },
                    GuiHomeRowFrame::Item(GuiHomeItemRowFrame {
                        id: row_id.clone(),
                        title: "Better Call Saul".to_owned(),
                        subtitle: None,
                        trailing: None,
                        kind: GuiHomeRowKindFrame::Media,
                        state: GuiHomeRowStateFrame::Ordinary,
                        art: GuiRowArtFrame::NoSource,
                        media: GuiRowMediaKindFrame::Video,
                    }),
                ],
            }),
        },
        GuiLibraryDetail::Home(GuiHomeDetail {
            selected: Some(row_id),
            hero: with_hero.then(|| GuiHomeHeroFrame {
                title: "Better Call Saul".to_owned(),
                meta_rows: vec![segment("1 h 47 m left", GuiMetaKindFrame::Time)],
                duration_row: Some(0),
                progress_row: None,
                overview: None,
                art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Square,
                art: None,
            }),
        }),
    )
}

/// The carried Shows list part: the series list, and no collection selector
/// (gui-collection-dropdown design D6, D8).
pub(super) fn shows_list() -> GuiShowsList {
    GuiShowsList {
        view: mbv::GuiShowsView::All,
        lib_idx: 0,
        loading: false,
        series: Vec::new(),
        upcoming_failed: false,
        rows: vec![GuiShowsRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            display_only: false,
            id: "s1".to_owned(),
            title: "Atlanta".to_owned(),
            subtitle: None,
            date: None,
            sort_letter: Some('A'),
        }],
    }
}

/// The carried Shows detail part: the selected series and its Hero.
pub(super) fn shows_detail() -> GuiShowsDetail {
    GuiShowsDetail {
        workspace_loading: false,
        selected: Some("s1".to_owned()),
        hero: Some(GuiShowsHeroFrame {
            title: "Atlanta".to_owned(),
            parent: None,
            meta_rows: vec![segment("4 seasons", GuiMetaKindFrame::Count)],
            duration_row: None,
            progress_row: None,
            overview: None,
            art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Landscape,
            art: None,
            workspace: None,
        }),
    }
}

/// The Shows sections, under the Shows tab.
pub(super) fn shows_sections() -> (GuiLibraryList, GuiLibraryDetail) {
    (
        GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 3,
                label: "Shows".to_owned(),
            }],
            current_tab: Some(3),
            destination: GuiDestinationList::Shows(shows_list()),
        },
        GuiLibraryDetail::Shows(shows_detail()),
    )
}

/// The plain Emby-library sections: the plain mode's list, or the feed-group
/// mode's collection selector and date-ordered rows.
pub(super) fn emby_sections(feed_group: bool, label: &str) -> (GuiLibraryList, GuiLibraryDetail) {
    (
        GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 3,
                label: label.to_owned(),
            }],
            current_tab: Some(3),
            destination: GuiDestinationList::Emby(GuiEmbyList {
                lib_idx: 3,
                library_name: "Movies".to_owned(),
                collections: feed_group.then(|| GuiCollections {
                    target: mbv::GuiCollectionTarget::EmbyFeed(3),
                    all: true,
                    entries: Vec::new(),
                    noun: mbv::GuiCollectionNoun::Folders,
                    order: Some(mbv::GuiDateOrder::Newest),
                }),
                collection_names: if feed_group {
                    vec!["All".to_owned()]
                } else {
                    Vec::new()
                },
                feed_group,
                loading: false,
                failed: false,
                rows: vec![GuiEmbyRowFrame {
                    art: GuiRowArtFrame::NoSource,
                    media: GuiRowMediaKindFrame::Video,
                    id: "m1".to_owned(),
                    title: "Alien".to_owned(),
                    subtitle: Some("1979".to_owned()),
                    date: feed_group.then(|| "1979".to_owned()),
                    sort_letter: (!feed_group).then_some('A'),
                    kind: GuiEmbyRowKindFrame::Media,
                }],
            }),
        },
        GuiLibraryDetail::Emby(GuiEmbyDetail {
            selected: Some("m1".to_owned()),
            hero: Some(GuiEmbyHeroFrame {
                kind: "movie".to_owned(),
                title: "Alien".to_owned(),
                meta_rows: vec![segment("1979", GuiMetaKindFrame::Date)],
                duration_row: Some(0),
                progress_row: None,
                overview: None,
                art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Landscape,
                art: None,
                credits: Vec::new(),
                imdb_url: None,
            }),
        }),
    )
}

/// The carried Audiobooks sections, with `hero` the selected book's Hero the
/// frame paints (row 9.1).
pub(super) fn audiobooks_sections(
    hero: Option<GuiAudiobooksHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    (
        GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 4,
                label: "Audiobooks".to_owned(),
            }],
            current_tab: Some(4),
            destination: GuiDestinationList::Audiobooks(GuiAudiobooksList {
                lib_idx: 4,
                loading: false,
                failed: false,
                rows: vec![GuiAudiobooksRowFrame {
                    art: GuiRowArtFrame::NoSource,
                    media: GuiRowMediaKindFrame::Audio,
                    id: "b1".to_owned(),
                    title: "Aftermath".to_owned(),
                    subtitle: Some("Bill Wyman".to_owned()),
                    duration: "4:32".to_owned(),
                    sort_letter: 'A',
                }],
            }),
        },
        GuiLibraryDetail::Audiobooks(GuiAudiobooksDetail {
            workspace_loading: false,
            selected: Some("b1".to_owned()),
            playing_book_id: None,
            hero,
        }),
    )
}

/// The book Hero the Audiobooks fixtures paint, with `workspace` its
/// chapters Workspace.
pub(super) fn audiobooks_hero(
    workspace: Option<GuiAudiobooksWorkspaceFrame>,
) -> GuiAudiobooksHeroFrame {
    GuiAudiobooksHeroFrame {
        title: "Aftermath".to_owned(),
        meta_rows: vec![
            segment("Bill Wyman", GuiMetaKindFrame::Plain),
            segment("4:32", GuiMetaKindFrame::Time),
        ],
        duration_row: Some(1),
        progress_row: None,
        overview: None,
        art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Portrait,
        art: None,
        workspace,
    }
}

/// The carried Feeds sections, with `hero` the selected entry's Hero the
/// frame paints (row 11.1).
pub(super) fn feeds_sections(
    hero: Option<GuiFeedsHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    (
        GuiLibraryList {
            tabs: vec![GuiLibraryTabFrame {
                position: 5,
                label: "Feeds".to_owned(),
            }],
            current_tab: Some(5),
            destination: GuiDestinationList::Feeds(GuiFeedsList {
                collections: GuiCollections {
                    target: mbv::GuiCollectionTarget::Feeds,
                    all: true,
                    entries: vec![GuiCollectionEntry {
                        key: "https://sub.one".to_owned(),
                        name: "Sub One".to_owned(),
                        checked: false,
                    }],
                    noun: mbv::GuiCollectionNoun::Subscriptions,
                    order: Some(mbv::GuiDateOrder::Newest),
                },
                loading: false,
                failed: false,
                platform: None,
                rows: vec![GuiEntryRowFrame {
                    art: GuiRowArtFrame::NoSource,
                    media: GuiRowMediaKindFrame::Video,
                    id: "s1::e1".to_owned(),
                    title: "Entry A".to_owned(),
                    subtitle: Some("Sub One".to_owned()),
                    date: Some("12 May".to_owned()),
                    state: GuiHomeRowStateFrame::Ordinary,
                }],
            }),
        },
        GuiLibraryDetail::Feeds(GuiFeedsDetail {
            selected: Some("s1::e1".to_owned()),
            hero,
        }),
    )
}

/// The entry Hero the Feeds fixtures paint.
pub(super) fn feeds_hero() -> GuiFeedsHeroFrame {
    GuiFeedsHeroFrame {
        title: "Entry A".to_owned(),
        parent: None,
        meta_rows: vec![
            segment("Sub One", GuiMetaKindFrame::Plain),
            segment("12:30", GuiMetaKindFrame::Time),
        ],
        duration_row: Some(1),
        progress_row: None,
        art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Square,
    }
}
