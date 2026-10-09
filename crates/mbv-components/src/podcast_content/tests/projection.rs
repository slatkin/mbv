use super::*;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

fn latest_catalog(episode_id: &str) -> AudiobookshelfEpisodeCatalog {
    AudiobookshelfEpisodeCatalog {
        library_item_id: "alpha".into(),
        episode_id: episode_id.into(),
        title: episode_id.into(),
        show_title: Some("Alpha Show".into()),
        author: None,
        description: None,
        duration_ticks: None,
        pub_date_secs: None,
        cover_path: None,
    }
}

#[test]
fn reanchor_launch_state_resolves_a_legacy_show_key_to_the_latest_scope() {
    // A show/show-item snapshot written by an older version decodes but
    // resolves to the persisted scope (spec: podcast restart starts on
    // Latest whether or not the podcast was the exit tab, and every
    // restart lands on the first row).
    let mut owner = owner();
    owner.set_latest_items(&[latest_catalog("latest-one")]);
    let state = mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::PodcastShow("gone".into()),
        }),
        item: Some(LibraryItemIdentity::Audiobookshelf {
            id: "gone\0episode".into(),
        }),
    };
    assert!(owner.reanchor_launch_state(&state));
    assert_eq!(owner.pill(), &PillSelection::Latest);
    assert_eq!(
        owner.selected_episode_target().unwrap().episode_id(),
        "latest-one"
    );
}

#[test]
fn launch_snapshot_records_latest_and_no_item_regardless_of_the_live_pill() {
    // The persisted pill scope is always Latest; a show or state pill is
    // session memory, and the selected episode never persists.
    let mut owner = owner();
    owner.set_pill(PillSelection::Show("beta".into()));

    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::Latest,
            }),
            None,
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
fn a_refresh_that_drops_the_show_pill_resets_to_all() {
    let mut owner = owner();
    owner.set_focused(true);
    // Walk to the Beta show pill.
    for _ in 0..4 {
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
    }
    assert_eq!(owner.pill, PillSelection::Show("beta".into()));

    // A completed refresh that still lists Beta replaces the catalog in
    // place (design D2): the pill and its selected show are retained.
    let mut kept = AudiobookshelfBrowseState::new(library());
    kept.append_page(
        0,
        20,
        2,
        vec![show("alpha", "Alpha Show"), show("beta", "Beta Show")],
    );
    owner.set_content(&kept, false);
    assert_eq!(owner.pill, PillSelection::Show("beta".into()));

    // A completed refresh without Beta drops the pill back to the state
    // pills.
    let mut dropped = AudiobookshelfBrowseState::new(library());
    dropped.append_page(0, 20, 1, vec![show("alpha", "Alpha Show")]);
    owner.set_content(&dropped, false);
    assert_eq!(
        owner.pill,
        PillSelection::State(AudiobookshelfEpisodeFilter::All)
    );
    assert_eq!(owner.content().selector.unwrap().active, Some(1));
}

/// #745: reset returns the podcast owner to its default state pill and first
/// episode, clearing marks and the prior viewport, and a refresh that lands
/// afterwards preserves that selection instead of re-adopting the pre-reset
/// pill or episode.
#[test]
fn reset_presentation_returns_to_all_and_survives_a_later_refresh() {
    let mut owner = owner();
    let first = owner.selected_episode_target();
    owner.on_slot_event(LibrarySlotEvent::SelectorPicked(5));
    assert_eq!(owner.pill(), &PillSelection::Show("beta".into()));
    assert_eq!(item_rows(&owner), [("beta-one", "beta-one")]);
    let selected = owner.selected_episode_target().expect("beta episode");
    owner.episodes.set_scroll(2);
    owner.episodes.toggle_selection(&selected);

    owner.reset_presentation();

    assert_eq!(
        owner.pill(),
        &PillSelection::State(AudiobookshelfEpisodeFilter::All)
    );
    assert_eq!(owner.selected_episode_target(), first);
    assert_eq!(owner.episodes.scroll(), 0);
    assert_eq!(owner.episodes.multi_selection().len(), 0);
    assert_eq!(owner.content().selector.unwrap().active, Some(1));

    // A refresh completing after the reset preserves the reset selection.
    owner.set_content(&fixture_state(), false);
    assert_eq!(
        owner.pill(),
        &PillSelection::State(AudiobookshelfEpisodeFilter::All)
    );
    assert_eq!(owner.selected_episode_target(), first);
}

#[test]
fn episode_rows_are_split_rows_with_played_and_resume_status() {
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
fn show_pill_scopes_the_view_to_that_show() {
    let mut owner = owner();
    owner.on_slot_event(LibrarySlotEvent::SelectorPicked(5));
    assert_eq!(
        item_rows(&owner),
        [("beta-one", "beta-one")],
        "a show pill ignores play state and excludes other shows"
    );
}
