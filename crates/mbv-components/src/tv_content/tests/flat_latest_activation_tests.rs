use super::*;
use mbv_queue::TvContentMode;
use rstest::rstest;
use tuirealm::event::KeyModifiers;

/// Latest resolves keyboard actions against the displayed row
/// (newest-first), not the alphabetical ordinal the ordinal fallback indexes:
/// the newest episode ("Zeta") is not alphabetically first, so Enter and
/// Ctrl+P must address it, not "Alpha" — in both Narrow and Wide geometry.
#[rstest]
#[case::narrow(false)]
#[case::wide(true)]
fn latest_activation_addresses_the_displayed_row(#[case] wide: bool) {
    // Pushed newest-first: Zeta is the newest episode displayed first.
    let items = vec![
        tv_episode("Zeta", "episode-zeta"),
        tv_episode("Alpha", "episode-alpha"),
    ];
    let mut owner = TvContent::new();
    let mut context = tv_tree_context(items, None, None, false);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    owner.set_content(context);
    owner.set_is_wide(wide);
    owner.set_focused(true);

    let key = |code, modifiers| KeyEvent { code, modifiers };
    let episode_id = |message: &Option<Msg>| match message {
        Some(Msg::Shell(shell)) => match shell.as_ref() {
            ShellRequest::TvEpisodeActivate { episode } => Some(episode.id.clone()),
            ShellRequest::EmbyLibraryPlay { item } => Some(item.id.clone()),
            _ => None,
        },
        _ => None,
    };

    let activate = owner.on_key(&key(Key::Enter, KeyModifiers::NONE));
    assert_eq!(
        episode_id(&activate).as_deref(),
        Some("episode-zeta"),
        "Enter must activate the displayed newest-first row"
    );

    let play = owner.on_key(&key(Key::Char('p'), KeyModifiers::CONTROL));
    assert_eq!(
        episode_id(&play).as_deref(),
        Some("episode-zeta"),
        "Ctrl+P must play the displayed newest-first row"
    );
}

/// The tv-library-content-modes contract "Wide Latest shows the episode
/// hero" (owner layer): a Wide TV owner in Latest with a selected episode
/// returns that episode's hero from `hero_data`.
#[test]
fn wide_latest_hero_data_returns_the_selected_episode_hero() {
    let items = vec![
        tv_episode("Zeta", "episode-zeta"),
        tv_episode("Alpha", "episode-alpha"),
    ];
    let mut owner = TvContent::new();
    let mut context = tv_tree_context(items, None, None, false);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    owner.set_content(context);
    owner.set_is_wide(true);
    owner.set_focused(true);

    let hero = owner
        .hero_data()
        .expect("Wide Latest shows the selected episode's hero");
    assert_eq!(
        hero.facts.title, "Zeta",
        "the Wide Latest hero is the displayed newest-first episode"
    );
}
