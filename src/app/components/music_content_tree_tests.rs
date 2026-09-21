//! Grouped Music tree interaction tests: pointer gestures, multi-selection
//! marks, keyboard navigation/expansion, and the shared tree-owner fixtures.

use super::*;

/// Artist roots are hero-bearing rows, and unfiltered Enter shares the
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
    owner.browser.apply_filter_query("Alpha");
    assert!(
        !owner.hero_overlay_enter_available(),
        "a filtered artist root keeps Enter local"
    );

    let mut album_owner = tree_owner(&[("Alpha", &["a-0"])]);
    assert!(album_owner.hero_overlay_enter_available());
    assert!(album_owner.hero_overlay_target_available());
}

#[test]
fn hero_double_click_activates_the_selected_track() {
    let album = make_item("Album", "MusicAlbum");
    let track = make_item("Track", "Audio");
    let mut owner = MusicContent::new();
    let mut ctx = context(album.clone(), "overview");
    ctx.album_tracks = Some(vec![track.clone()]);
    owner.set_content(ctx);

    let area = Rect::new(0, 0, 30, 1);
    owner.track_list.wide_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| owner.track_list.wide_mut().view(frame, area))
        .unwrap();

    let message = owner.on_slot_event(LibrarySlotEvent::HeroPane(
        MediaListSurfaceInput::DoubleClick(Position { x: 0, y: 0 }),
    ));
    match message {
        Some(Msg::Shell(ShellRequest::MusicTrackActivate {
            album_id,
            track: activated,
        })) => {
            assert_eq!(album_id, album.id);
            assert_eq!(activated.id, track.id);
        }
        other => panic!("expected track activation, got {other:?}"),
    }
}

#[test]
fn album_wheel_emits_cursor_for_owner_target_and_noop_for_unknown_target() {
    let mut owner = MusicContent::new();
    owner.set_content(context(make_item("Album", "MusicAlbum"), "overview"));

    let area = Rect::new(0, 0, 30, 1);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| owner.browser.view(frame, area))
        .unwrap();

    let event = LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position { x: 0, y: 0 },
        delta: 1,
    });
    assert!(matches!(
        owner.on_slot_event(event),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 0,
            kind: AlbumCursorKind::Move,
        }))
    ));

    owner.context.album_targets.clear();
    assert_eq!(
        owner.on_slot_event(event),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    );
}

#[test]
fn wide_album_metadata_removes_artist_and_year_prefix() {
    // The old `wide_album_metadata` characterization (rehomed here by task
    // 9.2): a tagged album whose display name still carries the
    // `Artist (Year) Title` folder prefix must present the bare title and
    // the parsed release year, even though `derive_album_display_name`
    // leaves a tagged album's name untouched.
    let mut album = make_item("Bob Dylan (1970) New Morning", "MusicAlbum");
    album.artist = "Bob Dylan".into();
    album.production_year = 1970;

    assert_eq!(
        wide_album_metadata(&album, "Bob Dylan"),
        ("New Morning".to_string(), 1970)
    );
}

#[test]
fn resolved_hero_data_uses_parsed_title_year_and_cached_artist() {
    // The `album_artist_cache` fallback names the artist; the folder-name
    // parse supplies the title/year the Wide hero presents.
    let mut owner = MusicContent::new();
    let mut album = make_item("Folder Artist (2024) First Album", "MusicAlbum");
    album.artist.clear();
    album.production_year = 0;
    let mut ctx = context(album, "overview");
    ctx.album_info = vec![("Folder Artist".into(), "2024".into(), "First Album".into())];
    owner.set_content(ctx);

    let data = owner.hero_data().expect("hero data");
    assert_eq!(data.facts.title, "First Album");
    assert_eq!(data.facts.meta_rows, vec!["Folder Artist", "2024"]);
}

/// Music never tracks played/progress in its rows: a played track and a
/// half-played track both project the ordinary row (mbv never resumes a music
/// track, so its stored position means nothing either).
#[test]
fn played_tracks_project_the_ordinary_state() {
    let mut played = make_item("Finished Track", "Audio");
    played.played = true;
    let mut half_played = make_item("Half-Played Track", "Audio");
    half_played.runtime_ticks = 1000;
    half_played.playback_position_ticks = 500;
    let fresh = make_item("Fresh Track", "Audio");
    let rows = build_track_rows(&[played, half_played, fresh]);
    let states: Vec<&MediaSemanticState> = rows
        .iter()
        .map(|row| match row {
            MediaListRow::Item { semantic_state, .. } => semantic_state,
            _ => panic!("track rows are items"),
        })
        .collect();
    assert_eq!(states[0], &MediaSemanticState::Ordinary);
    assert_eq!(states[1], &MediaSemanticState::Ordinary);
    assert_eq!(states[2], &MediaSemanticState::Ordinary);
}

#[test]
fn track_rows_project_runtime_in_the_green_gutter() {
    let mut track = make_item("Track", "Audio");
    track.runtime_ticks = 65 * TICKS_PER_SECOND;
    let rows = build_track_rows(&[track]);
    let MediaListRow::Item {
        trailing, duration, ..
    } = &rows[0]
    else {
        panic!("track rows are items");
    };
    assert_eq!(trailing, &Some(MediaListTrailing::Gutter("1:05".into())));
    assert_eq!(duration, &None);
}

#[test]
fn tree_tracks_use_numbered_workspace_labels_with_index_fallback() {
    let mut indexed = make_item("Indexed Track", "Audio");
    indexed.id = "track-indexed".into();
    indexed.index_number = 7;
    let mut fallback = make_item("Fallback Track", "Audio");
    fallback.id = "track-fallback".into();
    fallback.index_number = 0;

    let rows = build_track_rows(&[indexed.clone(), fallback.clone()]);
    let labels: Vec<&str> = rows
        .iter()
        .map(|row| match row {
            MediaListRow::Item { primary, .. } => primary.as_str(),
            _ => panic!("track rows are items"),
        })
        .collect();
    assert_eq!(labels, ["7. Indexed Track", "2. Fallback Track"]);

    let mut owner = tree_owner_with_tracks(&[("Alpha", &["a-0"])], Some(vec![indexed, fallback]));
    owner.expand_all_tree_roots();
    let album_id = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-0"))
        .expect("album leaf")
        .id();
    owner.browser.expand_node(album_id);
    let labels: Vec<&str> = owner
        .browser
        .projected_nodes()
        .iter()
        .filter(|node| owner.browser.title_of(node.id()).contains("Track"))
        .map(|node| owner.browser.title_of(node.id()))
        .collect();
    assert_eq!(labels, ["7. Indexed Track", "2. Fallback Track"]);
}

// ── Task 2.4: tree chord mapping through the component boundary ──────────

/// A multi-artist Grouped Music owner whose tree starts on the first album of
/// the first root (`selected_album` drives the initial adoption). `artists` is
/// `(artist, [album target…])` in settled order; the derived title is the
/// target so assertions can name leaves.
pub(super) fn tree_owner(artists: &[(&str, &[&str])]) -> MusicContent {
    tree_owner_with_tracks(artists, None)
}

pub(super) fn tree_owner_with_tracks(
    artists: &[(&str, &[&str])],
    album_tracks: Option<Vec<EmbyItem>>,
) -> MusicContent {
    let mut items: Vec<EmbyItem> = Vec::new();
    let mut album_info: Vec<(String, String, String)> = Vec::new();
    let mut artist_keys: Vec<crate::app::music_grouping::ArtistKey> = Vec::new();
    for (artist, targets) in artists {
        for target in *targets {
            let mut album = make_item(target, "MusicAlbum");
            album.id = (*target).to_string();
            album.artist = (*artist).to_string();
            // Grouped Music album rows are folder targets: shell actions must
            // expand them through the existing per-folder playback effects.
            album.is_folder = true;
            items.push(album);
            album_info.push((
                (*artist).to_string(),
                "2001".to_string(),
                (*target).to_string(),
            ));
            artist_keys.push(crate::app::music_grouping::ArtistKey::Service(format!(
                "artist-{artist}"
            )));
        }
    }
    let selected = items.first().cloned();
    let order: Vec<usize> = (0..items.len()).collect();
    let ctx = MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 0),
        selected,
        String::new(),
        Vec::new(),
        0,
        album_info,
        artist_keys,
        order,
        album_tracks,
    );
    let mut owner = MusicContent::new();
    owner.set_content(ctx);
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

pub(super) fn press(owner: &mut MusicContent, code: Key) -> Option<Msg> {
    owner.on_key(&KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

pub(super) fn selected_row(owner: &MusicContent) -> usize {
    let id = owner.browser.selected_id().expect("a node is selected");
    owner
        .browser
        .projected_nodes()
        .iter()
        .position(|node| node.id() == id)
        .expect("the selected node is projected")
}

pub(super) fn paint_tree(owner: &mut MusicContent, area: Rect) {
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("tree terminal");
    terminal
        .draw(|frame| owner.browser.view(frame, area))
        .expect("tree frame");
}

pub(super) fn tree_point(owner: &MusicContent, area: Rect, id: usize) -> Position {
    let row = owner
        .browser
        .projected_nodes()
        .iter()
        .position(|node| node.id() == id)
        .expect("node is projected");
    let visible_row = row
        .checked_sub(owner.browser.offset())
        .expect("node is inside the painted viewport");
    Position::new(area.x, area.y.saturating_add(visible_row as u16))
}

#[test]
fn tree_pointer_gestures_resolve_latest_artist_and_album_rows() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);

    let root = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()).is_none())
        .expect("artist root")
        .id();
    let album_0 = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-0"))
        .expect("first album leaf")
        .id();
    let album_1 = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-1"))
        .expect("second album leaf")
        .id();
    let root_at = tree_point(&owner, area, root);
    let album_0_at = tree_point(&owner, area, album_0);

    // A click resolves the painted artist row and changes local selection;
    // the grouping root manufactures no album request, but its resolved
    // focus crosses as the typed artist-track request (design D7).
    match owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
        root_at,
    ))) {
        Some(Msg::Shell(ShellRequest::MusicArtistTracks { target })) => {
            assert_eq!(target.artist_name, "Alpha");
        }
        other => panic!("expected the typed artist-track request, got {other:?}"),
    }
    assert!(owner.browser.selected_is_artist());

    // The same completed frame resolves a leaf click to its stable album
    // target, then wheel keeps the existing one-visible-row step semantics.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 0, .. }))
    ));
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));

    // A modified click resolves the current painted row first, toggles only
    // the album leaf, and never emits a playback or Queue request.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ToggleClick(
            album_0_at,
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    assert_eq!(
        owner.browser.selected_album_targets(),
        vec!["a-0".to_string()]
    );
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
            at: album_0_at,
            delta: 1,
        })),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 1, .. }))
    ));
    assert_eq!(owner.browser.selected_id(), Some(album_1));

    // Double-click and right-click resolve the row under the latest retained
    // geometry, rather than the previously focused node.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            Some((x, y)),
        ))) if items.len() == 1 && items[0].id == "a-0" && (x, y) == (album_0_at.x, album_0_at.y)
    ));

    // A right-click on an artist root inside a marked tree selection acts on
    // that selection, not on every descendant of the root. The grouping root
    // itself never crosses the effect boundary.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
            root_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            Some((x, y)),
        ))) if items.len() == 1
            && items[0].id == "a-0"
            && (x, y) == (root_at.x, root_at.y)
    ));
    assert!(owner.browser.selected_is_artist());

    // Artist modified-click scopes the operation to its currently visible
    // album descendants, not to an artist or Queue identity.
    let _ = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ToggleClick(
        root_at,
    )));
    assert_eq!(
        owner.browser.selected_album_targets_in_display_order(),
        vec!["a-0".to_string(), "a-1".to_string()]
    );
}

#[test]
fn tree_pointer_noop_and_local_expansion_requests_focus_once() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"])]);
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);
    let root = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()).is_none())
        .expect("artist root")
        .id();
    let root_at = tree_point(&owner, area, root);

    // The first click resolves the root's artist request; repeating the same
    // painted selection has no other effect and emits the single focus request.
    let _ = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
        root_at,
    )));
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            root_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));

    // Double-click expansion is local and still crosses once for focus.
    let was_expanded = owner.browser.root_is_expanded(root);
    paint_tree(&mut owner, area);
    let root_at = tree_point(&owner, area, root);
    assert_eq!(
        owner.browser.hit_node(root_at).map(|(id, _)| id),
        Some(root)
    );
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            root_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    assert_ne!(owner.browser.root_is_expanded(root), was_expanded);
    if !owner.browser.root_is_expanded(root) {
        owner.browser.toggle_root(root);
    }

    paint_tree(&mut owner, area);
    let album = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-0"))
        .expect("album leaf")
        .id();
    let album_at = tree_point(&owner, area, album);
    // A childless album claims double-click without opening a Hero or changing
    // expansion, and a wheel at the final row focuses even when movement clamps.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            album_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    let _ = owner.browser.take_album_selection_change();
    paint_tree(&mut owner, area);
    let album_at = tree_point(&owner, area, album);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
            at: album_at,
            delta: 1,
        })),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
}

#[test]
fn tree_context_click_outside_selection_clears_only_tree_marks() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);
    let a0 = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-0"))
        .expect("first album")
        .id();
    let b0 = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("b-0"))
        .expect("second album")
        .id();
    owner.browser.set_marked(a0, true);
    let b0_at = tree_point(&owner, area, b0);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(b0_at))),
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            _,
        ))) if items.len() == 1 && items[0].id == "b-0"
    ));
    assert!(
        owner.browser.selected_album_targets().is_empty(),
        "a context click outside the marked set clears only this tree"
    );
}

#[test]
fn clearing_grouped_music_marks_removes_the_selection_summary() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"])]);
    owner.set_selection_origin(crate::app::components::media_list::SelectionOrigin::Queue);
    owner.expand_all_tree_roots();
    let album = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-0"))
        .expect("album leaf")
        .id();

    owner.browser.set_marked(album, true);
    assert_eq!(
        owner.selection_summary().map(|summary| summary.count),
        Some(1)
    );

    owner.clear_selection();
    assert_eq!(owner.selection_summary(), None);
}

#[test]
fn home_end_and_page_move_over_the_tree_visible_nodes() {
    let mut owner = tree_owner(&[
        ("Alpha", &["a-0", "a-1", "a-2"]),
        ("Beta", &["b-0", "b-1", "b-2"]),
    ]);
    owner.expand_all_tree_roots();

    press(&mut owner, Key::Home);
    let first_visible = owner.browser.selected_id();
    assert!(
        owner.browser.selected_is_artist(),
        "Home lands on the first visible node (the Alpha root), not a Heading group"
    );
    press(&mut owner, Key::End);
    assert_eq!(
        owner.browser.selected_album_target(),
        Some("b-2"),
        "End lands on the last visible album"
    );
    press(&mut owner, Key::Home);
    assert_eq!(owner.browser.selected_id(), first_visible);

    // The page stride is the shared visible-node stride (5): it must never
    // inherit a Heading-based group jump.
    press(&mut owner, Key::PageDown);
    assert_eq!(selected_row(&owner), 5, "PageDown moves one page of rows");
    press(&mut owner, Key::PageUp);
    assert_eq!(selected_row(&owner), 0, "PageUp mirrors the page stride");
}

#[test]
fn up_and_down_move_across_artist_roots_and_album_leaves() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    press(&mut owner, Key::Home);
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.root_is_expanded(root));

    // Down crosses from the root to its first leaf, then on to the next root;
    // both are visible tree nodes.
    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-1"));
    press(&mut owner, Key::Down);
    assert!(
        owner.browser.selected_is_artist(),
        "the next visible node is Beta's artist root"
    );
    press(&mut owner, Key::Up);
    assert_eq!(owner.browser.selected_album_target(), Some("a-1"));
    // `j`/`k` alias the plain arrows.
    press(&mut owner, Key::Char('k'));
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    press(&mut owner, Key::Char('j'));
    assert_eq!(owner.browser.selected_album_target(), Some("a-1"));
}

#[test]
fn enter_preserves_unfiltered_artist_root_and_filter_enter_toggles_it() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    let root = owner.browser.selected_id().expect("artist root selected");
    // The initial album adoption expanded the first root's path.
    assert!(owner.browser.root_is_expanded(root));

    // The mounted non-Wide panel owns the unfiltered Hero entry before the
    // owner sees Enter; the direct owner has no request to emit.
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        owner.browser.root_is_expanded(root),
        "unfiltered Enter leaves expansion unchanged"
    );

    // Filtered artist Enter remains local and toggles expansion.
    assert!(owner
        .on_key(&KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE,
        })
        .is_some());
    owner.browser.apply_filter_query("Alpha");
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(!owner.browser.root_is_expanded(root));
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(owner.browser.root_is_expanded(root));
    press(&mut owner, Key::Esc);

    // The existing album activation is preserved on a leaf.
    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item })) => {
            assert_eq!(item.id, "a-0")
        }
        other => panic!("expected album activation, got {other:?}"),
    }
}

/// An album-leaf Enter focuses the Wide inline track pane, but moving the tree
/// selection onto an artist root must not let stale pane focus swallow the
/// root's Hero entry. The root keeps its expansion unchanged.
#[test]
fn enter_enters_a_root_reached_while_the_track_pane_holds_focus() {
    let mut owner = tree_owner_with_tracks(
        &[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])],
        Some(vec![make_item("t-0", "Audio"), make_item("t-1", "Audio")]),
    );
    owner.set_inline_track_focus_enabled(true);

    // Album-leaf Enter focuses the inline track pane (Wide).
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        owner.track_focused(),
        "album-leaf Enter focuses the track pane"
    );

    // Wide `Home` is not track-focus gated and moves the tree selection onto
    // the Alpha artist root while the pane still holds focus.
    press(&mut owner, Key::Home);
    assert!(owner.browser.selected_is_artist());
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.root_is_expanded(root));

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
        owner.browser.root_is_expanded(root),
        "Enter preserves the focused root's expansion"
    );
    // The root has no artist-detail rows in this fixture, so it cannot take
    // the track cursor and must not paint the previous album's Workspace.
    assert!(
        owner.panel_content().hero.is_none(),
        "no stale album Workspace paints under an artist root"
    );
}
