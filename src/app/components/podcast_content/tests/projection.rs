use super::*;

#[test]
fn reanchor_launch_state_falls_back_to_first_show_scope_and_episode() {
    let mut owner = owner();
    let state = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::PodcastShow("gone".into()),
        }),
        item: Some(LibraryItemIdentity::Audiobookshelf {
            id: "gone\0episode".into(),
        }),
    };
    assert!(owner.reanchor_launch_state(&state));
    assert_eq!(
        owner.pill(),
        &PillSelection::State(AudiobookshelfEpisodeFilter::All)
    );
    assert_eq!(
        owner.selected_episode_target().unwrap().library_item_id(),
        "alpha"
    );
}

#[test]
fn launch_snapshot_uses_filter_and_episode_identity() {
    let mut owner = owner();
    owner.set_pill(PillSelection::State(AudiobookshelfEpisodeFilter::Played));

    let (selector, item) = owner.launch_snapshot();
    assert_eq!(
        selector,
        Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::PodcastFilter(AudiobookshelfPodcastFilter::Played,),
        })
    );
    assert_eq!(
        item,
        Some(LibraryItemIdentity::Audiobookshelf {
            id: "alpha\0dated".into(),
        })
    );
}

#[test]
fn launch_snapshot_uses_show_id_and_selected_episode_identity() {
    let mut owner = owner();
    owner.set_pill(PillSelection::Show("beta".into()));

    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::PodcastShow("beta".into()),
            }),
            Some(LibraryItemIdentity::Audiobookshelf {
                id: "beta\0beta-one".into(),
            }),
        )
    );
}

#[test]
fn launch_snapshot_is_empty_without_shows_or_selected_episode() {
    let mut owner = PodcastContent::new();
    owner.set_content(&AudiobookshelfBrowseState::new(library()), false);
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::PodcastFilter(AudiobookshelfPodcastFilter::All),
            }),
            None
        )
    );
}

fn item_rows(owner: &PodcastContent) -> Vec<(&str, &str)> {
    owner
        .episodes
        .rows()
        .iter()
        .filter_map(|row| match row {
            MediaListRow::Item {
                target, secondary, ..
            } => Some((
                target.episode_id(),
                secondary.as_deref().unwrap_or_default(),
            )),
            _ => None,
        })
        .collect()
}

fn pills(content: &LibraryPanelContent) -> Vec<String> {
    content.selector.as_ref().expect("pill bar").pills.clone()
}

#[test]
fn pill_selection_starts_at_all_on_construction() {
    let owner = PodcastContent::new();
    assert_eq!(
        owner.pill,
        PillSelection::State(AudiobookshelfEpisodeFilter::All)
    );
}

#[test]
fn content_has_one_selector_row_for_state_and_show_pills() {
    let mut owner = owner();
    let content = owner.content();
    assert_eq!(
        pills(&content),
        [
            "Latest",
            "All",
            "Unplayed",
            "Played",
            "Alpha Show",
            "Beta Show"
        ]
    );
    assert_eq!(content.selector.unwrap().active, Some(1));
    // Secondary-row absence is owned by the shared panel skeleton test;
    // this owner test covers the combined state/show selector only.

    // `]` walks the whole bar uniformly, state pills included.
    owner.set_focused(true);
    owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    let content = owner.content();
    assert_eq!(content.selector.unwrap().active, Some(2));
    assert_eq!(
        owner.pill,
        PillSelection::State(AudiobookshelfEpisodeFilter::Unplayed)
    );
}

#[test]
fn show_pills_truncate_like_feed_group_labels() {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(
        0,
        20,
        1,
        vec![show("long", "A Very Long Podcast Show Name Indeed Indeed")],
    );
    let mut owner = PodcastContent::new();
    owner.set_now_secs(NOW);
    owner.set_content(&state, false);
    assert_eq!(
        pills(&owner.content())[4],
        trunc_str(
            "A Very Long Podcast Show Name Indeed Indeed",
            MAX_GROUP_LABEL
        )
    );
}

#[test]
fn painted_active_index_is_derived_from_the_stored_value() {
    let mut owner = owner();
    owner.set_focused(true);
    // Land on the Beta show pill (painted index 4).
    owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    assert_eq!(owner.pill, PillSelection::Show("beta".into()));

    // A new show page that sorts before Beta shifts Beta's painted
    // position; the value keeps identifying the same pill.
    let mut state = fixture_state();
    state.append_page(1, 20, 3, vec![show("aardvark", "Aardvark Show")]);
    owner.set_content(&state, false);
    assert_eq!(owner.pill, PillSelection::Show("beta".into()));
    assert_eq!(owner.content().selector.unwrap().active, Some(6));
}

#[test]
fn keyboard_pill_walk_wraps_at_both_ends() {
    let mut owner = owner();
    owner.set_focused(true);
    // Two `[` steps from `All` wrap through Latest to the last show.
    owner.on_key(&KeyEvent::new(Key::Char('['), KeyModifiers::NONE));
    owner.on_key(&KeyEvent::new(Key::Char('['), KeyModifiers::NONE));
    assert_eq!(owner.pill, PillSelection::Show("beta".into()));
    // `]` advances back to the Latest pill.
    owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    assert_eq!(owner.pill, PillSelection::Latest);
}

#[test]
fn remembered_pill_survives_an_ordinary_refresh() {
    let mut owner = owner();
    owner.set_focused(true);
    owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    owner.set_content(&fixture_state(), false);
    assert_eq!(
        owner.pill,
        PillSelection::State(AudiobookshelfEpisodeFilter::Unplayed)
    );
}

#[test]
fn a_refresh_that_drops_the_show_pill_resets_to_all() {
    let mut owner = owner();
    owner.set_focused(true);
    // Walk to the Beta show pill, then refresh without it.
    for _ in 0..4 {
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    }
    assert_eq!(owner.pill, PillSelection::Show("beta".into()));
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(0, 20, 1, vec![show("alpha", "Alpha Show")]);
    owner.set_content(&state, false);
    assert_eq!(
        owner.pill,
        PillSelection::State(AudiobookshelfEpisodeFilter::All)
    );
    assert_eq!(owner.content().selector.unwrap().active, Some(1));
}

#[test]
fn episode_rows_are_split_rows_with_played_and_in_progress_state() {
    let owner = owner();
    let rows = owner.episodes.rows().to_vec();
    let dated = rows
        .iter()
        .find_map(|row| match row {
            MediaListRow::Item { target, .. } if target.episode_id() == "dated" => {
                Some(row.clone())
            }
            _ => None,
        })
        .expect("dated episode row");
    match dated {
        MediaListRow::Item {
            primary,
            secondary,
            trailing,
            duration,
            semantic_state,
            kind,
            ..
        } => {
            assert_eq!(primary, "Alpha Show", "the split row names its podcast");
            assert_eq!(secondary.as_deref(), Some("dated"));
            assert_eq!(duration, None, "library episode rows carry no time");
            assert_eq!(
                trailing,
                Some(MediaListTrailing::Gutter("30 Jan".into())),
                "the row carries its publish date for the right-hand gutter"
            );
            assert_eq!(semantic_state, MediaSemanticState::Played);
            assert_eq!(kind, MediaKind::Media);
        }
        _ => panic!("expected an episode item row"),
    }
    let undated_trailing = rows
        .iter()
        .find_map(|row| match row {
            MediaListRow::Item {
                target, trailing, ..
            } if target.episode_id() == "undated" => Some(trailing.clone()),
            _ => None,
        })
        .expect("undated episode row");
    assert_eq!(
        undated_trailing, None,
        "an episode with no publish date reserves no gutter"
    );
    let in_progress = rows
        .iter()
        .find_map(|row| match row {
            MediaListRow::Item {
                target,
                semantic_state,
                ..
            } if target.episode_id() == "undated" => Some(semantic_state.clone()),
            _ => None,
        })
        .expect("in-progress episode row");
    assert_eq!(
        in_progress,
        MediaSemanticState::active(Some(16)),
        "in-progress progress renders the shared in-progress badge"
    );
}

#[test]
fn grouped_rows_carry_stable_episode_targets() {
    let owner = owner();
    let rows = owner.episodes.rows();
    assert!(matches!(rows.first(), Some(MediaListRow::Heading { .. })));
    assert!(matches!(rows.last(), Some(MediaListRow::Item { .. })));
    assert!(rows
        .iter()
        .any(|row| matches!(row, MediaListRow::Heading { .. })));
    // Heading insertion never changes episode targeting: the dated and
    // undated rows carry the provider-native identity pair.
    assert_eq!(
        item_rows(&owner),
        [
            ("dated", "dated"),
            ("undated", "undated"),
            ("beta-one", "beta-one")
        ],
        "the All view lists every fetched show's episodes"
    );
}

#[test]
fn show_pill_scopes_the_view_to_that_show() {
    let mut owner = owner();
    owner.on_slot_event(LibrarySlotEvent::SelectorPicked(5));
    assert_eq!(
        item_rows(&owner),
        [("beta-one", "beta-one")],
        "a show pill ignores play state and excludes other shows"
    );
}
