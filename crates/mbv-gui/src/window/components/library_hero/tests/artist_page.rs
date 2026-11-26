//! The artist branch's painted-structure and send contracts
//! (gui-music-artist-page design D6), over the shared harness.

use super::*;

/// The artist page's Hero facts (gui-music-artist-page design D6): two albums
/// in the page's order — Licensed To Ill with a loaded tracklist, Paul's
/// Boutique with none — under their year span.
fn artist_page_hero() -> GuiMusicHeroFrame {
    GuiMusicHeroFrame {
        artist: "Beastie Boys".to_owned(),
        collections: vec!["Hip-Hop".to_owned()],
        album_count: 2,
        artist_tracks: None,
        subject: GuiMusicHeroSubjectFrame::Artist(GuiMusicArtistPageFrame {
            artist_key: GuiArtistKey::Service("a1".to_owned()),
            albums: vec![
                GuiMusicAlbumFrame {
                    album_id: "al1".to_owned(),
                    name: "Licensed To Ill".to_owned(),
                    year: "1986".to_owned(),
                    tracks: Some(vec![
                        GuiMusicTrackFrame {
                            track_id: "t1".to_owned(),
                            name: "Rhymin & Stealin".to_owned(),
                            number: 1,
                            duration: "4:08".to_owned(),
                        },
                        GuiMusicTrackFrame {
                            track_id: "t2".to_owned(),
                            name: "The New Style".to_owned(),
                            number: 2,
                            duration: "4:35".to_owned(),
                        },
                    ]),
                    total: None,
                },
                pauls_boutique(),
            ],
            years: Some(("1986".to_owned(), "1989".to_owned())),
        }),
        menu: Vec::new(),
    }
}

/// The union behind the artist page: the artist row the page pick names, with
/// the page's albums.
fn artist_page_union(facts: &GuiMusicHeroFrame) -> Vec<GuiMusicArtistFrame> {
    let GuiMusicHeroSubjectFrame::Artist(page) = &facts.subject else {
        panic!("the fixture's subject is the artist page");
    };
    vec![GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: GuiArtistKey::Service("a1".to_owned()),
        name: "Beastie Boys".to_owned(),
        sort_letter: 'B',
        albums: page.albums.clone(),
    }]
}

/// The Hero mounted for the artist page.
fn built_artist_page() -> Fixture {
    let facts = artist_page_hero();
    built(artist_page_union(&facts), detail(facts, None))
}

/// Contract (gui-music-artist-page design D6, spec "A picked Music artist
/// shows an artist page"): the artist branch paints the artist's name in the
/// header, one label per album in the list band, one tracklist row per loaded
/// track, and no More actions control; an album whose tracks have not loaded
/// shows its label and no row.
#[test]
fn the_artist_page_paints_its_header_labels_and_loaded_tracks() {
    let fixture = built_artist_page();

    assert_eq!(
        fixture.text_of("[data-library-hero-title]"),
        "Beastie Boys",
        "the header shows the artist's name"
    );
    assert_eq!(
        fixture.count_of("[data-library-artist-album]"),
        2,
        "the list band shows one label per album"
    );
    assert!(
        fixture.presents("[data-library-artist-album=\"al2\"]"),
        "the album without loaded tracks shows its label"
    );
    assert_eq!(
        fixture.count_of("[data-library-track]"),
        2,
        "one tracklist row per loaded track paints, and none for the unloaded album"
    );
    assert!(
        fixture.presents("[data-library-track=\"t1\"]"),
        "the loaded track's row paints"
    );
    assert!(
        !fixture.presents("[data-library-hero-more]"),
        "the artist page offers no More actions control"
    );
}

/// Contract (gui-music-artist-page design D6, spec "Play acts on the whole
/// artist" and "Add to queue acts on the whole artist"): the artist branch's
/// Play and Add to queue send the page's album ids in the page's order.
#[test]
fn play_and_add_to_queue_send_the_pages_albums_in_order() {
    let mut fixture = built_artist_page();

    let album_ids = vec!["al1".to_owned(), "al2".to_owned()];
    let state = fixture.state.clone();
    let play_ids = album_ids.clone();
    fixture.in_runtime(move || play_artist(&state, &play_ids));
    let state = fixture.state.clone();
    fixture.in_runtime(move || add_artist_to_queue(&state, &album_ids));

    assert_eq!(
        fixture.library_commands(),
        vec![
            crate::gui_core::GuiLibraryCommand::PlayArtist {
                album_ids: vec!["al1".to_owned(), "al2".to_owned()],
            },
            crate::gui_core::GuiLibraryCommand::AddArtistToQueue {
                album_ids: vec!["al1".to_owned(), "al2".to_owned()],
            },
        ],
        "the commands name the page's albums in order"
    );
}

/// Contract (gui-music-artist-page design D6, spec "Enter plays the hovered
/// artist page track"): a track row's play gesture while the page shows sends
/// `PlayArtistTrack` with the page's album ids in order, so the sequence runs
/// through the page's albums and does not stop at the end of the track's
/// album.
#[test]
fn a_track_play_on_the_artist_page_sends_play_artist_track() {
    let mut fixture = built_artist_page();

    let track_id = "t2".to_owned();
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_library_track(&track_id));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::PlayArtistTrack {
            album_ids: vec!["al1".to_owned(), "al2".to_owned()],
            track_id: "t2".to_owned(),
        }],
        "the play command names the page's albums and the played track"
    );
}
