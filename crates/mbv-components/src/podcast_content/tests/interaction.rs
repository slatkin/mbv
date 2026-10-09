use super::*;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

/// The scoped loading projection (row 3.3): a state pill is loading
/// while its required shows fetch (and shows nothing stale meanwhile);
/// a show pill only while its own show's fetch is in flight, and
/// partially arrived results keep painting as the rest loads (design
/// D5: no visible reload of already-listed rows).
#[test]
fn scoped_loading_projection_follows_the_active_pill() {
    // State pill (All) with no results yet and a fetch in flight: the
    // scoped loading state.
    let mut state = fixture_state();
    state.detail_cache.clear();
    state.detail_loading_ids.insert("beta".into(), 0);
    let mut owner = PodcastContent::new();
    owner.set_now_secs(NOW);
    owner.set_content(&state, false);
    assert!(matches!(
        owner.content().list,
        ListSlot::Empty { loading: true, .. }
    ));

    // A partial arrival under the state pill (All): the landed shows'
    // rows paint while beta's fetch is still in flight (no visible
    // reload).
    let mut state = fixture_state();
    state.detail_loading_ids.insert("beta".into(), 0);
    let mut owner = PodcastContent::new();
    owner.set_now_secs(NOW);
    owner.set_content(&state, false);
    assert!(matches!(owner.content().list, ListSlot::Media(_)));

    // Show pill for a fetched show: not loading, its rows paint.
    let mut owner = PodcastContent::new();
    owner.set_now_secs(NOW);
    owner.set_content(&state, false);
    owner.on_slot_event(LibrarySlotEvent::SelectorPicked(3));
    assert!(matches!(owner.content().list, ListSlot::Media(_)));

    // Show pill for an in-flight, unfetched show: the scoped loading
    // state.
    let mut fresh = AudiobookshelfBrowseState::new(library());
    fresh.append_page(
        0,
        20,
        2,
        vec![show("alpha", "Alpha Show"), show("beta", "Beta Show")],
    );
    fresh.detail_loading_ids.insert("alpha".into(), 0);
    let mut owner = PodcastContent::new();
    owner.set_now_secs(NOW);
    owner.set_content(&fresh, false);
    owner.on_slot_event(LibrarySlotEvent::SelectorPicked(3));
    assert!(matches!(
        owner.content().list,
        ListSlot::Empty { loading: true, .. }
    ));
}

/// Context-menu spec, "Audiobookshelf row is selected": `.` on the selected
/// podcast episode row opens the menu anchored to the selection (no pointer
/// anchor), targeting exactly that episode.
#[test]
fn dot_opens_the_selected_episode_context_menu_anchored_to_the_selection() {
    let mut owner = owner();
    owner.set_focused(true);
    assert!(matches!(
        owner.on_key(&KeyEvent::new(Key::Char('.'), KeyModifiers::NONE)),
        Some(Msg::Shell(ref shell_boxed))
            if matches!(
                shell_boxed.as_ref(),
                ShellRequest::RowContextMenu(ContextMenuTargets::Audiobookshelf(targets), None)
                    if targets.as_slice() == [AudiobookshelfMenuTarget::Episode {
                        library_item_id: "alpha".into(),
                        episode_id: "dated".into(),
                    }]
            )
    ));
}

/// Media-list-multi-select, "one entry path for every list": the same `.`
/// gesture over a Visual multi-selection emits every marked row in display
/// order.
#[test]
fn dot_over_a_visual_selection_emits_every_marked_episode_in_list_order() {
    let mut owner = owner();
    owner.set_focused(true);
    // Shift+V begins Visual mode anchored on the selected row; one Down
    // extends the marked range to the next row.
    owner.on_key(&KeyEvent::new(Key::Char('V'), KeyModifiers::SHIFT));
    owner.on_key(&KeyEvent::new(Key::Down, KeyModifiers::NONE));
    assert!(matches!(
        owner.on_key(&KeyEvent::new(Key::Char('.'), KeyModifiers::NONE)),
        Some(Msg::Shell(ref shell_boxed))
            if matches!(
                shell_boxed.as_ref(),
                ShellRequest::RowContextMenu(ContextMenuTargets::Audiobookshelf(targets), None)
                    if targets.as_slice() == [
                        AudiobookshelfMenuTarget::Episode {
                            library_item_id: "alpha".into(),
                            episode_id: "dated".into(),
                        },
                        AudiobookshelfMenuTarget::Episode {
                            library_item_id: "alpha".into(),
                            episode_id: "undated".into(),
                        },
                    ]
            )
    ));
}

#[test]
fn enter_emits_open_or_play_with_the_selected_episode_target() {
    let mut owner = owner();
    owner.set_focused(true);
    assert!(matches!(
        owner.on_key(&KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        Some(Msg::Shell(ref shell_boxed))
            if matches!(
                shell_boxed.as_ref(),
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    Some(target)
                )) if target.episode_id() == "dated"
            )
    ));
}

#[test]
fn pointer_click_resolves_the_episode_target_and_unknown_point_is_noop() {
    let mut owner = owner();
    owner.set_focused(true);

    let area = ratatui::layout::Rect::new(0, 0, 30, 4);
    owner.episodes.wide_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
    terminal
        .draw(|frame| owner.episodes.wide_mut().view(frame, area))
        .unwrap();

    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            Position { x: 0, y: 1 }
        ))),
        Some(Msg::Shell(ref shell_boxed))
            if matches!(
                shell_boxed.as_ref(),
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    Some(target)
                )) if target.episode_id() == "dated"
            )
    ));
    assert_eq!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            Position { x: 0, y: 99 }
        ))),
        None
    );
}
