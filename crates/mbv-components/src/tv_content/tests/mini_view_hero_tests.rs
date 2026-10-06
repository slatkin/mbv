//! The mini-view hero-availability contract (owner layer): a selected flat
//! episode offers the compact Narrow hero only in Latest mode. Upcoming rows
//! are premiere placeholders for episodes that do not exist yet, so Upcoming
//! must never offer the overlay — not on selection, and not through a stale
//! Latest selection that survives a pill switch (2026-10-06 mini-view
//! Upcoming hero pop-up).

use super::*;
use mbv_queue::TvContentMode;
use tuirealm::event::KeyModifiers;

fn flat_owner(mode: TvContentMode, items: Vec<EmbyItem>) -> TvContent {
    let mut owner = TvContent::new();
    let mut context = tv_tree_context(items, None, None, false);
    context.set_tv_content_mode(Some(mode));
    owner.set_content(context);
    owner.set_is_wide(false);
    owner.set_focused(true);
    owner
}

#[test]
fn upcoming_never_offers_the_mini_view_hero() {
    let items = vec![tv_episode("Pilot", "episode-pilot")];
    let down = KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    };

    let mut latest = flat_owner(TvContentMode::Latest, items.clone());
    latest.on_key(&down);
    assert!(
        latest.mini_view_hero_available(),
        "Latest keeps the mini-view hero for its selected episode"
    );

    let mut upcoming = flat_owner(TvContentMode::Upcoming, items);
    upcoming.on_key(&down);
    assert!(
        !upcoming.mini_view_hero_available(),
        "Upcoming must not offer the mini-view hero"
    );
}
