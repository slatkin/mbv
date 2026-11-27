//! The destination-mount contracts (`LibraryPanel.test.ts`): the panel
//! mounts the active destination's painters in place of the others' — the
//! collection dropdown for the destinations that have collections, the Heroes
//! since rows 7.5-7.6, and the destination lists since row 7.7, each named by
//! its own list column hook.

use super::{built, fixtures, seed_idle_status};
use crate::window::arrangement::GuiArrangement;

/// Contract (`LibraryPanel.test.ts`, gui-swap-queue-and-library-sides D7,
/// gui-collection-dropdown D6a; design D7 of
/// `move-playback-header-into-queue-footer`): the panel mounts no top bar —
/// the window's global header sits above both columns — and it paints the
/// list region before the Wide Hero pane, with no region above the list.
#[test]
fn the_panel_paints_the_list_and_the_hero_region_and_no_header() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::music_sections(None);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    let regions = fixture.regions();
    assert_eq!(
        regions,
        ["list", "rail", "hero"],
        "no region paints above the list"
    );
    assert!(
        !fixture.presents("[data-library-region=\"topBar\"]"),
        "the panel mounts no header of its own"
    );
    // The dropdown rides the list header (gui-collection-dropdown row 8.2), so
    // the Music list's header paints its trigger and the list's painter mounts
    // under it.
    assert!(
        fixture.presents("[data-library-collections-trigger]"),
        "the Music collection dropdown's trigger mounts"
    );
    assert!(
        fixture.presents("[data-library-list-column]"),
        "the Music list's painter mounts"
    );
    // The Library zone (`gui-pointer-target-interaction` 3.3): every key whose
    // pointer rests in this column is the Library's.
    assert!(
        fixture.presents("[data-pointer-zone=\"library\"]"),
        "the panel root carries the Library zone marker"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 6.2): while
/// the Home content is present (design D12, the destinations are exclusive),
/// the panel mounts the Home painters in place of the Music ones, and the
/// collection dropdown and the letter rail do not paint.
#[test]
fn the_home_content_mounts_the_home_list_and_hero_and_no_collection_dropdown() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::home_sections(true);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        !fixture.presents("[data-library-collections-trigger]"),
        "Home paints no collection dropdown"
    );
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]"),
        "Home paints no letter rail"
    );
    assert!(
        fixture.presents("[data-library-home-list-column]"),
        "the Home list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-library-home-hero]"),
        "the Home Hero mounts"
    );
    assert!(
        !fixture.presents("[data-library-list-column]"),
        "the Music list's painter does not mount"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 7.1): while
/// the Shows content is present (design D12, the destinations are
/// exclusive), the panel mounts the Shows painters in place of the Music and
/// Home ones — no collection selector, its list, and the series Hero.
#[test]
fn the_shows_content_mounts_the_shows_list_and_the_series_hero() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::shows_sections();
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-shows-list-column]"),
        "the Shows list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-library-shows-hero]"),
        "the series Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-list-column]")
            && !fixture.presents("[data-library-home-list-column]"),
        "no other destination's list mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 8.1): while
/// the plain Emby-library content is present (design D12, the destinations are
/// exclusive), the panel mounts the Emby painters in place of the Music,
/// Home, and Shows ones — its list and its Hero, and no collection dropdown.
#[test]
fn the_plain_emby_content_mounts_the_emby_list_and_hero_and_none_of_the_other_destinations() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::emby_sections(false, "Movies");
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-emby-list-column]"),
        "the Emby list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-library-emby-hero]"),
        "the Emby Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-collections-trigger]"),
        "a plain library paints no collection dropdown"
    );
    assert!(
        !fixture.presents("[data-library-list-column]")
            && !fixture.presents("[data-library-home-list-column]")
            && !fixture.presents("[data-library-shows-list-column]"),
        "no other destination's list mounts"
    );
}
/// Contract (emby-favourites spec "A Favourites view replaces the destination's
/// list column"): the Favourites view mounts the favourites list in place of
/// the destination's own list, with its picked favourite's Hero, no letter rail,
/// no sort control, and no collection dropdown.
#[test]
fn the_favourites_view_mounts_the_favourites_list_and_hero_in_place_of_the_emby_list() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::emby_favourites_sections();
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-favourites-list-column]"),
        "the favourites list mounts in the list column"
    );
    assert!(
        !fixture.presents("[data-library-emby-list-column]"),
        "the destination's All list does not paint"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-library-favourites-hero]"),
        "the Favourites Hero mounts from the favourites list part"
    );
    assert!(
        !fixture.presents("[data-library-emby-hero]"),
        "the destination's own Hero does not paint"
    );
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]")
            && !fixture.presents("[data-library-favourites-letter]"),
        "the Favourites list paints no letter headings and no rail"
    );
    assert!(
        !fixture.presents("[data-library-sort-control]"),
        "the Favourites view paints no sort control"
    );
}

/// Contract (`LibraryPanel.test.ts`, row 8.1): the feed-group Emby content
/// mounts its collection dropdown and its list (gui-collection-dropdown row
/// 5.3).
#[test]
fn the_feed_group_emby_content_mounts_its_collection_dropdown() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::emby_sections(true, "Home videos");
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-collections-trigger]"),
        "the feed-group collection dropdown mounts"
    );
    assert!(
        fixture.presents("[data-library-emby-list-column]"),
        "the Emby list's painter mounts"
    );
}

/// Contract (gui-collection-dropdown spec "A destination without
/// collections"): the Shows content paints its series list and no collection
/// dropdown.
#[test]
fn the_shows_content_paints_no_collection_dropdown() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::shows_sections();
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-shows-list-column]"),
        "the Shows list's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-collections-trigger]"),
        "Shows paints no collection dropdown"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 9.1): while
/// the Audiobooks content is present (design D12, the destinations are
/// exclusive), the panel mounts the book list and the book Hero in place of
/// the other destinations' painters, and no collection dropdown rides on the
/// destination.
#[test]
fn the_audiobooks_content_mounts_the_book_list_and_hero_and_none_of_the_other_destinations() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::audiobooks_sections(Some(fixtures::audiobooks_hero(None)));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-abs-list-column]"),
        "the book list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-library-abs-hero]"),
        "the book Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-collections-trigger]"),
        "no collection dropdown rides on the destination"
    );
    assert!(
        !fixture.presents("[data-library-list-column]")
            && !fixture.presents("[data-library-home-list-column]")
            && !fixture.presents("[data-library-shows-list-column]")
            && !fixture.presents("[data-library-emby-list-column]"),
        "no other destination's list mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, row 9.1): a frame without a Hero paints
/// no Hero, and the Narrow overlay stays closed while the selection carries
/// none.
#[test]
fn the_hero_less_audiobooks_content_paints_its_list_and_no_hero() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::audiobooks_sections(None);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-abs-list-column]"),
        "the book list's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-abs-hero]"),
        "no Hero paints"
    );
    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "no overlay mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 11.1): while
/// the Feeds content is present (design D12, the destinations are exclusive),
/// the panel mounts the collection dropdown, the entry list, and the entry
/// Hero, and none of the other destinations' painters.
#[test]
fn the_feeds_content_mounts_the_feeds_list_and_hero_and_none_of_the_other_destinations() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::feeds_sections(Some(fixtures::feeds_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-collections-trigger]"),
        "the collection dropdown mounts"
    );
    assert!(
        fixture.presents("[data-library-feeds-list-column]"),
        "the Feeds list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-library-feeds-hero]"),
        "the entry Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-list-column]")
            && !fixture.presents("[data-library-podcasts-list-column]")
            && !fixture.presents("[data-library-home-list-column]")
            && !fixture.presents("[data-library-emby-list-column]"),
        "no other destination's list mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, row 11.1): a frame without a Hero
/// paints no Hero, and the Narrow overlay stays closed while the selection
/// carries none.
#[test]
fn the_hero_less_feeds_content_paints_its_list_and_no_hero() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::feeds_sections(None);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-feeds-list-column]"),
        "the Feeds list's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-feeds-hero]"),
        "no Hero paints"
    );
    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "no overlay mounts"
    );
}
