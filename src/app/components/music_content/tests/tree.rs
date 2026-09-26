//! Grouped Music tree-owner content and selection contracts: the projected
//! row/label/hero content, and the selection-summary state the owner reports.

use super::tree_fixtures::{find, paint_tree, tree_owner, tree_owner_with_tracks, tree_point};
use super::*;
use crate::app::components::music_content::workspace::build_track_rows;

/// double-click/Right Hero entry while filtered Enter stays local.
#[test]
fn artist_roots_are_hero_eligible_only_when_unfiltered() {
    use crate::app::components::library_panel::owner::LibraryContentOwner;

    let mut owner = artist_workspace_owner();
    assert!(
        owner.hero_overlay_target_available(),
        "an artist root is a hero-bearing row"
    );
    assert!(
        owner.hero_overlay_enter_available(),
        "an unfiltered artist root enters its Hero"
    );

    owner.on_key(&KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    owner
        .browser
        .apply(TreeOperation::EditFilter("Alpha".to_string()));
    assert!(
        !owner.hero_overlay_enter_available(),
        "a filtered artist root keeps Enter local"
    );

    let mut album_owner = tree_owner(&[("Alpha", &["a-0"])]);
    assert!(album_owner.hero_overlay_enter_available());
    assert!(album_owner.hero_overlay_target_available());
}
