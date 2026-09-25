use super::*;
use mbv_core::config::TvContentMode;
use tuirealm::event::KeyModifiers;

/// Narrow Latest resolves keyboard actions against the displayed row
/// (newest-first), not the alphabetical ordinal the ordinal fallback indexes:
/// the newest episode ("Zeta") is not alphabetically first, so Enter and
/// Ctrl+P must address it, not "Alpha".
#[test]
fn narrow_latest_activation_addresses_the_displayed_row() {
    // Pushed newest-first: Zeta is the newest episode displayed first.
    let items = vec![
        tv_episode("Zeta", "episode-zeta"),
        tv_episode("Alpha", "episode-alpha"),
    ];
    let mut owner = TvContent::new();
    let mut context = tv_tree_context(items, None, None, false);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    owner.set_content(context);
    owner.set_is_wide(false);
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
