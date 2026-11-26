//! The artist row's contracts: a press picks the artist and toggles no
//! node, the row paints the picked fill while the artist page shows
//! (gui-music-artist-page design D6), the open state and hover controls paint
//! (gui-tree-double-click, gui-row-hover-actions), and the top-level row
//! paints the shared part's thumb while its nested rows paint none
//! (gui-delineated-rows design D1). Over the shared harness.

use super::*;

/// The Music detail part whose Hero shows the Beastie Boys artist page
/// (gui-music-artist-page design D6).
fn artist_page() -> GuiMusicDetail {
    GuiMusicDetail {
        hero: Some(GuiMusicHeroFrame {
            artist: "Beastie Boys".to_owned(),
            collections: vec!["Hip-Hop".to_owned()],
            album_count: 2,
            artist_tracks: None,
            subject: GuiMusicHeroSubjectFrame::Artist(GuiMusicArtistPageFrame {
                artist_key: GuiArtistKey::Service("a1".to_owned()),
                albums: Vec::new(),
                years: Some(("1986".to_owned(), "1989".to_owned())),
            }),
            menu: Vec::new(),
        }),
        art: None,
        playing_track_id: None,
    }
}

/// Contract (gui-music-artist-page design D6, spec "An artist row is
/// pressed"): a press on an artist row picks the artist — the command names
/// the artist's stable key and the Music library index — and the row neither
/// opens nor closes.
#[test]
fn a_press_on_an_artist_row_picks_the_artist_and_toggles_no_node() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    measure(&mut fixture, 240.0);

    press_row(&mut fixture, "artist:service:a1");

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::ShowArtist {
            lib_idx: 0,
            artist_key: mbv_ui_model::music_grouping::ArtistKey::Service("a1".to_owned()),
        }]
    );
    assert_eq!(
        fixture.attr_of(
            "[data-library-row-id=\"artist:service:a1\"]",
            local_name!("aria-expanded"),
        ),
        Some("false".to_owned()),
        "the pressed artist row stays closed"
    );
}

/// Contract (gui-music-artist-page design D6, spec "An artist row is
/// pressed"): while the Hero shows the artist page, the artist's row paints
/// the picked fill — it is the picked row.
#[test]
fn the_artist_row_paints_picked_while_the_artist_page_shows() {
    let mut fixture = built(&music(&["Hip-Hop"]), &artist_page());
    measure(&mut fixture, 240.0);

    assert!(
        fixture.presents("[data-library-row-id=\"artist:service:a1\"][data-library-selected]"),
        "the artist row paints the picked fill while the subject is the artist page"
    );
    assert!(
        !fixture.presents("[data-library-row-id=\"artist:service:a2\"][data-library-selected]"),
        "no other row paints the picked fill"
    );
}

/// Contract (gui-tree-double-click design D2, D4; gui-row-hover-actions design
/// D1-D3): the Music artist row is a tree parent — its open state paints as
/// `aria-expanded` and its hover offers the Expand control, which reuses the
/// row's toggle, beside Play, Add, and Shuffle over the artist's item id.
#[test]
fn the_artist_row_paints_its_open_state_and_offers_every_control() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    measure(&mut fixture, 240.0);

    let row = "[data-library-row-id=\"artist:service:a1\"]";
    assert_eq!(
        fixture.attr_of(row, local_name!("aria-expanded")),
        Some("false".to_owned()),
        "the closed artist paints its open state"
    );
    assert_eq!(
        fixture.hover_controls_of("artist:service:a1"),
        ["play", "queue", "shuffle", "toggle"],
        "the artist with an item id offers Play, Add, Shuffle, and Expand"
    );
}

/// Contract (gui-delineated-rows design D1): an artist row is a top-level row
/// and paints the shared part's thumb with its art state, while the album and
/// track rows under it keep their own painters and paint none.
#[test]
fn the_artist_row_paints_a_thumb_and_its_nested_rows_paint_none() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    open_licensed_to_ill(&mut fixture);
    measure(&mut fixture, 240.0);

    assert_eq!(
        fixture.attr_of(
            "[data-library-row-id=\"artist:service:a1\"] [data-library-thumb]",
            blitz_dom::LocalName::from("data-library-thumb"),
        ),
        Some("loading".to_owned()),
        "the artist row paints the shared part's thumb with its art state"
    );
    assert!(
        !fixture.presents("[data-library-row-id=\"album:al1\"] [data-library-thumb]"),
        "the nested album row paints no thumb"
    );
    assert!(
        !fixture.presents("[data-library-row-id=\"track:t1\"] [data-library-thumb]"),
        "the nested track row paints no thumb"
    );
}
