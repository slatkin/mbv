//! The destination-mount contracts (`LibraryPanel.test.ts`): the panel
//! mounts the active destination's painters in place of the others', with
//! the pill row, list, and Hero places as placeholder child markers until
//! rows 7.2-7.7 replace them.

use super::{built, fixtures, seed_idle_status};
use crate::bridge::frame::library::GuiShowsModeFrame;
use crate::window::arrangement::GuiArrangement;

/// Contract (`LibraryPanel.test.ts`): the panel paints the top bar and the
/// pill row above the list region, and the Wide Hero pane between them.
#[test]
fn the_panel_paints_the_top_bar_the_pill_row_and_the_hero_above_the_list_region() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::music_sections(None);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    let regions = fixture.regions();
    assert_eq!(regions[..3], ["topBar", "pills", "hero"]);
    assert!(regions.contains(&"list".to_owned()));
    // The pills region and the placeholder marker name one box until row 7.4
    // replaces the marker with the pill row's painter.
    assert!(
        fixture.presents("[data-panel-child=\"LibraryPills\"]"),
        "the Music pill row's painter mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 6.2): while
/// the Home content is present (design D12, the destinations are exclusive),
/// the panel mounts the Home painters in place of the Music ones, and the
/// pill row and the letter rail do not paint.
#[test]
fn the_home_content_mounts_the_home_list_and_hero_and_no_pills() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::home_sections(true);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        !fixture.presents("[data-library-region=\"pills\"]"),
        "Home paints no pill row"
    );
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]"),
        "Home paints no letter rail"
    );
    assert!(
        fixture.presents("[data-panel-child=\"LibraryHomeList\"]"),
        "the Home list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-panel-child=\"LibraryHomeHero\"]"),
        "the Home Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryList\"]"),
        "the Music list's painter does not mount"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 7.1): while
/// the Shows content is present (design D12, the destinations are
/// exclusive), the panel mounts the Shows painters in place of the Music and
/// Home ones — its mode pills, its list, and the series Hero.
#[test]
fn the_shows_content_mounts_the_mode_pills_the_shows_list_and_the_series_hero() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::shows_sections(GuiShowsModeFrame::All);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-region=\"pills\"]"),
        "the mode pills mount"
    );
    assert!(
        fixture.presents("[data-panel-child=\"LibraryShowsList\"]"),
        "the Shows list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-panel-child=\"LibraryShowsHero\"]"),
        "the series Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryHomeList\"]"),
        "no other destination's list mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`): the Upcoming mode carries no Hero
/// (design D12) — the panel paints its flat list and no Hero in the Narrow
/// arrangement.
#[test]
fn the_upcoming_shows_content_paints_no_hero() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::shows_sections(GuiShowsModeFrame::Upcoming);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-panel-child=\"LibraryShowsList\"]"),
        "the Shows list's painter mounts"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryShowsHero\"]"),
        "no Hero paints"
    );
    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "no overlay mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 8.1): while
/// the plain Emby-library content is present (design D12, the destinations
/// are exclusive), the panel mounts the Emby painters in place of the Music,
/// Home, and Shows ones — its list and its Hero, and no pill row.
#[test]
fn the_plain_emby_content_mounts_the_emby_list_and_hero_and_none_of_the_other_destinations() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::emby_sections(false, "Movies");
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-panel-child=\"LibraryEmbyList\"]"),
        "the Emby list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-panel-child=\"LibraryEmbyHero\"]"),
        "the Emby Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-region=\"pills\"]"),
        "a plain library paints no pill row"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryHomeList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryShowsList\"]"),
        "no other destination's list mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, row 8.1): the feed-group Emby content
/// mounts its pill row.
#[test]
fn the_feed_group_emby_content_mounts_its_pill_row() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::emby_sections(true, "Home videos");
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-region=\"pills\"]"),
        "the feed-group pill row mounts"
    );
    assert!(
        fixture.presents("[data-panel-child=\"LibraryEmbyList\"]"),
        "the Emby list's painter mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 9.1): while
/// the Audiobooks content is present (design D12, the destinations are
/// exclusive), the panel mounts the book list and the book Hero in place of
/// the other destinations' painters, and no pill row rides on the
/// destination.
#[test]
fn the_audiobooks_content_mounts_the_book_list_and_hero_and_none_of_the_other_destinations() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::audiobooks_sections(Some(fixtures::audiobooks_hero(None)));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-panel-child=\"LibraryAudiobooksList\"]"),
        "the book list's painter mounts"
    );
    assert!(
        fixture.presents(
            "[data-library-region=\"hero\"] [data-panel-child=\"LibraryAudiobooksHero\"]"
        ),
        "the book Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-library-region=\"pills\"]"),
        "no pill row rides on the destination"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryHomeList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryShowsList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryEmbyList\"]"),
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
        fixture.presents("[data-panel-child=\"LibraryAudiobooksList\"]"),
        "the book list's painter mounts"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryAudiobooksHero\"]"),
        "no Hero paints"
    );
    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "no overlay mounts"
    );
}
/// Contract (`LibraryPanel.test.ts`, add-gui-library-panel row 11.1): while
/// the Feeds content is present (design D12, the destinations are
/// exclusive), the panel mounts the view pills, the entry list, and the
/// entry Hero, and none of the other destinations' painters.
#[test]
fn the_feeds_content_mounts_the_feeds_pills_list_and_hero_and_none_of_the_other_destinations() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::feeds_sections(Some(fixtures::feeds_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-region=\"pills\"]"),
        "the view pills mount"
    );
    assert!(
        fixture.presents("[data-panel-child=\"LibraryFeedsList\"]"),
        "the Feeds list's painter mounts"
    );
    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-panel-child=\"LibraryFeedsHero\"]"),
        "the entry Hero's painter mounts"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryPodcastsList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryHomeList\"]")
            && !fixture.presents("[data-panel-child=\"LibraryEmbyList\"]"),
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
        fixture.presents("[data-panel-child=\"LibraryFeedsList\"]"),
        "the Feeds list's painter mounts"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"LibraryFeedsHero\"]"),
        "no Hero paints"
    );
    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "no overlay mounts"
    );
}
