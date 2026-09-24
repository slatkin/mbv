//! Grouped Music tree keyboard navigation: the visible-node cursor walk,
//! viewport paging, and the Enter activation/expansion contract, including the
//! filter's effect on both.

use super::tree_fixtures::{find, paint_tree, press, selected_row, tree_owner};
use super::*;
use crate::app::components::music_tree_target::MusicTreeTarget;

#[test]
fn home_end_and_page_move_over_the_tree_visible_nodes() {
    let mut owner = tree_owner(&[
        ("Alpha", &["a-0", "a-1", "a-2"]),
        ("Beta", &["b-0", "b-1", "b-2"]),
    ]);
    owner.expand_all_tree_roots();

    press(&mut owner, Key::Home);
    let first_visible = owner.browser.selected_target().cloned();
    assert!(
        owner.selected_is_artist(),
        "Home lands on the first visible node (the Alpha root), not a Heading group"
    );
    press(&mut owner, Key::End);
    assert_eq!(
        owner.selected_album_target().as_deref(),
        Some("b-2"),
        "End lands on the last visible album"
    );
    press(&mut owner, Key::Home);
    assert_eq!(owner.browser.selected_target(), first_visible.as_ref());

    // Paging moves by the established visible viewport (here 3 rows), never a
    // fixed stride and never a Heading-based group jump.
    paint_tree(&mut owner, Rect::new(0, 0, 48, 3));
    press(&mut owner, Key::PageDown);
    assert_eq!(
        selected_row(&owner),
        3,
        "PageDown moves one visible viewport"
    );
    press(&mut owner, Key::PageUp);
    assert_eq!(selected_row(&owner), 0, "PageUp mirrors the viewport page");
}

/// The page distance is the visible viewport height, not a fixed row count:
/// two different painted heights produce two different page distances, which
/// a fixed-stride implementation cannot satisfy.
#[test]
fn page_distance_tracks_the_visible_viewport_height() {
    let page_down_row = |height: u16| {
        let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2", "a-3", "a-4", "a-5"])]);
        owner.expand_all_tree_roots();
        press(&mut owner, Key::Home);
        paint_tree(&mut owner, Rect::new(0, 0, 48, height));
        assert_eq!(selected_row(&owner), 0, "Home starts the page from row 0");
        press(&mut owner, Key::PageDown);
        selected_row(&owner)
    };

    assert_eq!(page_down_row(2), 2, "a 2-row viewport pages 2 rows");
    assert_eq!(page_down_row(4), 4, "a 4-row viewport pages 4 rows");
}

/// Before any geometry is established there is no visible viewport to page,
/// so a page chord is a deterministic no-op rather than a made-up stride.
#[test]
fn paging_without_established_geometry_does_not_move() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2"])]);
    owner.expand_all_tree_roots();
    press(&mut owner, Key::Home);
    assert_eq!(selected_row(&owner), 0);

    press(&mut owner, Key::PageDown);
    assert_eq!(
        selected_row(&owner),
        0,
        "no viewport geometry means no page"
    );
    press(&mut owner, Key::PageUp);
    assert_eq!(
        selected_row(&owner),
        0,
        "no viewport geometry means no page"
    );
}

#[test]
fn up_and_down_move_across_artist_roots_and_album_leaves() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    press(&mut owner, Key::Home);
    let root = owner
        .browser
        .selected_target()
        .cloned()
        .expect("artist root selected");
    assert!(owner.browser.is_expanded(&root));

    // Down crosses from the root to its first leaf, then on to the next root;
    // both are visible tree nodes.
    press(&mut owner, Key::Down);
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));
    press(&mut owner, Key::Down);
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-1"));
    press(&mut owner, Key::Down);
    assert!(
        owner.selected_is_artist(),
        "the next visible node is Beta's artist root"
    );
    press(&mut owner, Key::Up);
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-1"));
    // `j`/`k` alias the plain arrows.
    press(&mut owner, Key::Char('k'));
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));
    press(&mut owner, Key::Char('j'));
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-1"));
}

#[test]
fn enter_preserves_unfiltered_artist_root_and_filter_enter_toggles_it() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    let root = owner
        .browser
        .selected_target()
        .cloned()
        .expect("artist root selected");
    // The initial album adoption expanded the first root's path.
    assert!(owner.browser.is_expanded(&root));

    // The mounted non-Wide panel owns the unfiltered Hero entry before the
    // owner sees Enter; the direct owner has no request to emit.
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        owner.browser.is_expanded(&root),
        "unfiltered Enter leaves expansion unchanged"
    );

    // Filtered artist Enter remains local and toggles expansion.
    assert!(owner
        .on_key(&KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE,
        })
        .is_some());
    owner
        .browser
        .apply(TreeOperation::EditFilter("Alpha".to_string()));
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(!owner.browser.is_expanded(&root));
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(owner.browser.is_expanded(&root));
    press(&mut owner, Key::Esc);

    // The existing album activation is preserved on a leaf.
    press(&mut owner, Key::Down);
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));
    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item })) => {
            assert_eq!(item.id, "a-0")
        }
        other => panic!("expected album activation, got {other:?}"),
    }

    // A filtered dismissal must not revert to the pre-filter anchor: `/`,
    // move onto another match, Enter activates and keeps the activated album
    // in the tree, the Workspace/Hero resolution, and the launch snapshot.
    assert!(owner
        .on_key(&KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE,
        })
        .is_some());
    owner
        .browser
        .apply(TreeOperation::EditFilter("2001".to_string()));
    assert_eq!(
        owner.selected_album_target().as_deref(),
        Some("a-0"),
        "the filter anchors on the current album"
    );
    press(&mut owner, Key::Down);
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-1"));
    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item })) => {
            assert_eq!(item.id, "a-1")
        }
        other => panic!("expected album activation, got {other:?}"),
    }
    assert_eq!(
        owner.selected_album_target().as_deref(),
        Some("a-1"),
        "the dismissed filter keeps the activated album, not its anchor"
    );
    assert_eq!(
        owner.launch_snapshot().1,
        Some(LibraryItemIdentity::Emby { id: "a-1".into() }),
        "the launch snapshot keeps the activated album"
    );

    // The filter can reach a leaf under a collapsed artist root: dismissing it
    // restores the pre-filter anchor, so a plain `Select` cannot re-address the
    // hidden leaf and the tree would revert. The re-selection must reveal the
    // ancestor path instead (legacy `select_album_target` parity).
    assert!(owner
        .on_key(&KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE,
        })
        .is_some());
    owner
        .browser
        .apply(TreeOperation::EditFilter("b-0".to_string()));
    let beta_root = find(&owner, |target| target.is_artist());
    assert!(
        !owner.browser.is_expanded(&beta_root),
        "the filter reaches the leaf while its artist root stays collapsed"
    );
    assert_eq!(
        owner.selected_album_target().as_deref(),
        None,
        "the collapsed root is the first filtered row"
    );
    press(&mut owner, Key::Down);
    assert_eq!(owner.selected_album_target().as_deref(), Some("b-0"));
    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item })) => {
            assert_eq!(item.id, "b-0")
        }
        other => panic!("expected album activation, got {other:?}"),
    }
    assert_eq!(
        owner.selected_album_target().as_deref(),
        Some("b-0"),
        "the dismissed filter keeps the collapsed root's activated album"
    );
    assert!(
        owner.browser.is_expanded(&beta_root),
        "the re-selection revealed the activated album's ancestor path"
    );
    assert!(
        owner
            .browser
            .visible_targets()
            .contains(&MusicTreeTarget::Album("b-0".into())),
        "the activated album is revealed in the tree"
    );
    assert_eq!(
        owner.launch_snapshot().1,
        Some(LibraryItemIdentity::Emby { id: "b-0".into() }),
        "the launch snapshot keeps the collapsed root's activated album"
    );
}

#[test]
fn launch_snapshot_preserves_album_target_when_list_content_is_stale() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));

    // Simulate the list snapshot becoming stale before orderly exit while the
    // tree still retains its stable album selection.
    owner.context.list.items.clear();
    assert!(owner.selected_item().is_none());
    assert_eq!(
        owner.launch_snapshot().1,
        Some(LibraryItemIdentity::Emby { id: "a-0".into() })
    );
}

/// An album-leaf Enter focuses the Wide inline track pane, but moving the tree
/// selection onto an artist root must not let stale pane focus swallow the
/// root's Hero entry. The root keeps its expansion unchanged.
#[test]
fn enter_enters_a_root_reached_while_the_track_pane_holds_focus() {
    let mut first = make_item("t-0", "Audio");
    first.id = "t-0".into();
    let mut second = make_item("t-1", "Audio");
    second.id = "t-1".into();
    let mut owner = tree_owner_with_tracks(
        &[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])],
        Some(vec![first, second]),
    );
    owner.set_inline_track_focus_enabled(true);

    // Album-leaf Enter focuses the inline track pane (Wide).
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        owner.track_focused(),
        "album-leaf Enter focuses the track pane"
    );

    // Wide `Home` is not track-focus gated and moves the tree selection onto
    // the Alpha artist root while the pane still holds focus.
    press(&mut owner, Key::Home);
    assert!(owner.selected_is_artist());
    let root = owner
        .browser
        .selected_target()
        .cloned()
        .expect("artist root selected");
    assert!(owner.browser.is_expanded(&root));

    // The stale pane must not swallow Enter: the unfiltered root reconciles
    // away the album carrier and arms the artist Workspace entry instead.
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        owner.pending_artist_workspace_focus_for_test(),
        "artist-root Enter arms entry when its rows have not landed"
    );
    assert!(
        !owner.track_focused(),
        "stale album rows never retain track focus under an artist root"
    );
    assert!(
        owner.browser.is_expanded(&root),
        "Enter preserves the focused root's expansion"
    );
    // The root has no artist-detail rows in this fixture, so it cannot take
    // the track cursor and must not paint the previous album's Workspace.
    assert!(
        owner.panel_content().hero.is_none(),
        "no stale album Workspace paints under an artist root"
    );
}
