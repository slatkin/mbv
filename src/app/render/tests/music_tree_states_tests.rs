//! How a tree row's projected semantic state and tree depth choose its
//! colour roles.

use super::test_helpers::{
    album, album_leaf, artist, expand_root, music_tree_frame, select_target, tree_browser,
};
use super::*;

use crate::app::components::list::tree_browser::{TreeMarkPolicy, TreeNode, TreeTitleRole};
use crate::app::components::media_list::MediaSemanticState;
use crate::app::components::music_tree_target::MusicTreeTarget;

#[test]
fn music_tree_ordinary_rows_keep_depth_roles_and_live_playback_emphasis() {
    let root = artist("semantic-artist");
    let mut browser = tree_browser(vec![
        TreeNode::new(
            root.clone(),
            None,
            "Semantic Artist",
            "Semantic Artist",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Aggregate,
        ),
        album_leaf(
            &root,
            "played-album",
            "Played Album",
            None,
            MediaSemanticState::Ordinary,
        ),
        album_leaf(
            &root,
            "active-album",
            "Active Album",
            None,
            MediaSemanticState::active(Some(50)),
        ),
    ]);
    expand_root(&mut browser, &root);
    browser.set_focused(false);
    let area = Rect::new(0, 0, 40, 3);
    let term = music_tree_frame(&mut browser, area, 40, 3);
    let buf = term.backend().buffer();

    assert_eq!(
        buf[(4, 1)].fg,
        palette::TEXT_FOCUS_ACCENT,
        "a played music row remains the ordinary album colour"
    );
    assert_eq!(
        buf[(4, 2)].fg,
        palette::TEXT_EMPHASIS,
        "live playback keeps tree-row emphasis"
    );
}

#[test]
fn music_tree_selected_live_row_uses_selected_foreground() {
    let root = artist("selected-live-artist");
    let mut browser = tree_browser(vec![
        TreeNode::new(
            root.clone(),
            None,
            "Selected Live Artist",
            "Selected Live Artist",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Aggregate,
        ),
        album_leaf(
            &root,
            "selected-live-album",
            "Now Playing Album",
            None,
            MediaSemanticState::NowPlaying {
                progress: Some(crate::app::components::media_list::ActiveProgress::new(47)),
            },
        ),
    ]);
    expand_root(&mut browser, &root);
    select_target(&mut browser, &album("selected-live-album"));
    let area = Rect::new(0, 0, 40, 2);
    let term = music_tree_frame(&mut browser, area, area.width, area.height);

    assert_eq!(
        term.backend().buffer()[(4, 1)].fg,
        palette::SELECTED_ROW_FG,
        "selected live-playback tree rows use the Ink bar foreground"
    );
}

#[test]
fn music_tree_depth_roles_use_ordinary_level_colours() {
    let root = artist("depth-role-artist");
    let album_target = album("depth-album");
    let mut browser = tree_browser(vec![
        TreeNode::new(
            root.clone(),
            None,
            "Depth Artist",
            "Depth Artist",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Aggregate,
        )
        .with_title_role(TreeTitleRole::Heading),
        album_leaf(
            &root,
            "depth-album",
            "Depth Album",
            None,
            MediaSemanticState::Ordinary,
        ),
        TreeNode::new(
            MusicTreeTarget::Track {
                album: "depth-album".into(),
                track: "depth-track".into(),
            },
            Some(album_target.clone()),
            "Depth Track",
            "Depth Track",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Excluded,
        ),
    ]);
    expand_root(&mut browser, &root);
    expand_root(&mut browser, &album_target);
    browser.set_focused(false);

    let area = Rect::new(0, 0, 40, 3);
    let term = music_tree_frame(&mut browser, area, area.width, area.height);
    let buf = term.backend().buffer();
    // The shared painter indents two plain columns per depth, so the album
    // leaf starts at column 2 and the cached track at column 4.
    assert_eq!(buf[(0, 0)].fg, palette::TEXT_EMPHASIS);
    assert_eq!(buf[(2, 1)].fg, palette::TEXT_FOCUS_ACCENT);
    assert_eq!(buf[(4, 2)].fg, palette::ACCENT);
}
