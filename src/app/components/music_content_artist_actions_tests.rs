//! Grouped Music artist-action tests: Play/Enqueue materialization over the
//! settled leaves, filter interaction, and unresolved-target feedback.

use super::tree_tests::{press, tree_owner};
use super::*;

fn tree_owner_with_stable_keys(artists: &[(&str, &str, &[&str])]) -> MusicContent {
    let mut items = Vec::new();
    let mut album_info = Vec::new();
    let mut artist_keys = Vec::new();
    for (artist, artist_id, targets) in artists {
        for target in *targets {
            let mut album = make_item(target, "MusicAlbum");
            album.id = (*target).to_string();
            album.artist = (*artist).to_string();
            album.is_folder = true;
            items.push(album);
            album_info.push((
                (*artist).to_string(),
                "2001".to_string(),
                (*target).to_string(),
            ));
            artist_keys.push(crate::app::music_grouping::ArtistKey::Service(
                (*artist_id).to_string(),
            ));
        }
    }
    let selected = items.first().cloned();
    let order: Vec<usize> = (0..items.len()).collect();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 0),
        selected,
        String::new(),
        Vec::new(),
        0,
        album_info,
        artist_keys,
        order,
        None,
    ));
    owner.selection_origin = Some(SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Service(
            crate::app::components::library_panel::owner::LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: "music-library".into(),
                kind: crate::app::components::library_panel::owner::LibraryKind::Music,
            },
        ),
    ));
    owner
}

fn artist_action_ids(owner: &mut MusicContent, code: Key) -> Vec<String> {
    let message = owner.on_key(&KeyEvent {
        code,
        modifiers: if matches!(code, Key::Char('p' | 'a' | 's')) {
            KeyModifiers::CONTROL
        } else {
            KeyModifiers::NONE
        },
    });
    let items = match message {
        Some(Msg::Shell(ShellRequest::MusicArtistAction {
            items,
            unresolved_targets,
            ..
        })) => {
            assert!(
                unresolved_targets.is_empty(),
                "settled artist fixture should resolve every album target"
            );
            assert!(
                items.iter().all(|item| item.is_folder),
                "artist play/enqueue/shuffle requests carry folder album targets"
            );
            items
        }
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            _,
        ))) => {
            assert!(
                items.iter().all(|item| item.is_folder),
                "artist context requests carry folder album targets"
            );
            items
        }
        other => panic!("expected an artist album action for {code:?}, got {other:?}"),
    };
    items.into_iter().map(|item| item.id).collect()
}

#[test]
fn artist_actions_materialize_all_albums_for_collapsed_and_expanded_roots() {
    let expected = vec!["a-0".to_string(), "a-1".to_string(), "a-2".to_string()];
    let mut collapsed = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2"])]);
    press(&mut collapsed, Key::Home);
    let root = collapsed
        .browser
        .selected_id()
        .expect("artist root selected");
    collapsed.browser.collapse_root(root);
    for code in [
        Key::Char('p'),
        Key::Char('a'),
        Key::Char('s'),
        Key::Char('.'),
    ] {
        assert_eq!(artist_action_ids(&mut collapsed, code), expected);
    }

    let mut expanded = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2"])]);
    press(&mut expanded, Key::Home);
    let root = expanded
        .browser
        .selected_id()
        .expect("artist root selected");
    expanded.browser.expand_root(root);
    for code in [
        Key::Char('p'),
        Key::Char('a'),
        Key::Char('s'),
        Key::Char('.'),
    ] {
        assert_eq!(artist_action_ids(&mut expanded, code), expected);
    }
}

#[test]
fn filtered_artist_actions_materialize_only_matching_leaves_in_settled_order() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2", "a-3", "a-4"])]);
    press(&mut owner, Key::Home);
    let matching: Vec<usize> = owner
        .browser
        .projected_nodes()
        .iter()
        .filter_map(|node| {
            matches!(owner.browser.target_of(node.id()), Some("a-1" | "a-3")).then_some(node.id())
        })
        .collect();
    owner.browser.set_filter_matches(Some(&matching));

    assert_eq!(
        artist_action_ids(&mut owner, Key::Char('p')),
        vec!["a-1".to_string(), "a-3".to_string()]
    );
    assert_eq!(
        artist_action_ids(&mut owner, Key::Char('.')),
        vec!["a-1".to_string(), "a-3".to_string()]
    );
}

#[test]
fn partially_unresolved_artist_actions_keep_ordered_targets_and_report_misses() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    // Simulate a tree/content sync race: the tree still has both leaves, but
    // the latest content snapshot only resolves the first one.
    owner.context.album_targets = vec!["a-0".into(), "stale-target".into()];
    owner.context.list.items.truncate(1);

    for code in [Key::Char('p'), Key::Char('a'), Key::Char('s')] {
        let message = owner.on_key(&KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
        });
        match message {
            Some(Msg::Shell(ShellRequest::MusicArtistAction {
                items,
                unresolved_targets,
                ..
            })) => {
                assert_eq!(
                    items.into_iter().map(|item| item.id).collect::<Vec<_>>(),
                    vec!["a-0"],
                    "a resolved album must survive a stale sibling target"
                );
                assert_eq!(unresolved_targets, vec!["a-1"]);
            }
            other => panic!("expected a partial artist action for {code:?}, got {other:?}"),
        }
    }
}

#[test]
fn fully_unresolved_artist_action_requests_shell_feedback() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    owner.context.album_targets.clear();
    owner.context.list.items.clear();

    match owner.on_key(&KeyEvent {
        code: Key::Char('p'),
        modifiers: KeyModifiers::CONTROL,
    }) {
        Some(Msg::Shell(ShellRequest::MusicArtistAction {
            items,
            unresolved_targets,
            ..
        })) => {
            assert!(items.is_empty());
            assert_eq!(unresolved_targets, vec!["a-0", "a-1"]);
        }
        other => panic!("expected shell feedback request, got {other:?}"),
    }
}

#[test]
fn empty_visible_artist_emits_no_action_target() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    owner.browser.set_filter_matches(Some(&[]));

    for (code, modifiers) in [
        (Key::Char('p'), KeyModifiers::CONTROL),
        (Key::Char('a'), KeyModifiers::CONTROL),
        (Key::Char('s'), KeyModifiers::CONTROL),
        (Key::Char('.'), KeyModifiers::NONE),
    ] {
        assert!(
            owner.on_key(&KeyEvent { code, modifiers }).is_none(),
            "an artist with no visible album leaves has no action"
        );
    }
}

#[test]
fn equal_name_artists_resolve_actions_by_stable_root_identity() {
    let mut owner = tree_owner_with_stable_keys(&[
        ("Same Name", "artist-one", &["one-album"]),
        ("Same Name", "artist-two", &["two-album"]),
    ]);
    press(&mut owner, Key::Home);
    press(&mut owner, Key::Down);
    press(&mut owner, Key::Down);
    assert!(owner.browser.selected_is_artist());

    for code in [
        Key::Char('p'),
        Key::Char('a'),
        Key::Char('s'),
        Key::Char('.'),
    ] {
        assert_eq!(
            artist_action_ids(&mut owner, code),
            vec!["two-album".to_string()],
            "the second equal-name root owns only its album"
        );
    }
}
