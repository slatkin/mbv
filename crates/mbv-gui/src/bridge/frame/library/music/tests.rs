//! Tests of the Music Hero frame's subject mirror (gui-music-artist-page
//! design D1, D6, D8): the frame carries the subject the core's Hero holds,
//! and `picked_name`/`picked_row_id` read it in the row-id encodings
//! `window/library/list.rs` paints, so the window readers never match on the
//! subject.

use rstest::rstest;

use super::*;

/// One album as the core's Hero holds it.
fn album(id: &str, name: &str) -> GuiMusicAlbum {
    GuiMusicAlbum {
        album_id: id.to_owned(),
        name: name.to_owned(),
        year: "1986".to_owned(),
        tracks: None,
        total: None,
    }
}

/// The core Hero over `subject`, with the header facts the subject's artist
/// reads.
fn hero(subject: GuiMusicHeroSubject) -> GuiMusicHero {
    GuiMusicHero {
        artist: "Beastie Boys".to_owned(),
        collections: vec!["Hip-Hop".to_owned()],
        album_count: 2,
        artist_tracks: None,
        subject,
    }
}

/// The artist page of `key`, with the union's two albums.
fn artist_page(key: ArtistKey) -> GuiMusicArtistPage {
    GuiMusicArtistPage {
        artist_key: key,
        albums: vec![
            album("alb-ill", "Licensed to Ill"),
            album("alb-paul", "Paul's Boutique"),
        ],
        years: Some(("1986".to_owned(), "1989".to_owned())),
    }
}

/// `picked_name` and `picked_row_id` follow the subject (design D6): the
/// album subject names the album and its album row, and the artist page names
/// the artist and its artist row, in the exact encodings the window's list
/// paints (`album:<id>`, `artist:service:<id>`, `artist:fallback:<name>`).
#[rstest]
#[case::album_subject(
    GuiMusicHeroSubject::Album(album("alb-ill", "Licensed to Ill")),
    "Licensed to Ill",
    "album:alb-ill"
)]
#[case::service_artist_page(
    GuiMusicHeroSubject::Artist(artist_page(ArtistKey::Service("art-1".to_owned()))),
    "Beastie Boys",
    "artist:service:art-1"
)]
#[case::fallback_artist_page(
    GuiMusicHeroSubject::Artist(artist_page(ArtistKey::Fallback("Beastie Boys".to_owned()))),
    "Beastie Boys",
    "artist:fallback:Beastie Boys"
)]
fn the_picked_name_and_row_id_follow_the_subject(
    #[case] subject: GuiMusicHeroSubject,
    #[case] picked_name: &str,
    #[case] picked_row_id: &str,
) {
    let frame = GuiMusicHeroFrame::from(&hero(subject));
    assert_eq!(frame.picked_name(), picked_name);
    assert_eq!(frame.picked_row_id(), picked_row_id);
}
