//! Tests for the MPRIS publishers' pure parts.
//!
//! Nothing here touches D-Bus: a test process must never claim a real bus name
//! (issue #757), so the serving code is compiled out of test builds and these
//! tests cover the rule, the art-URI decision, and nothing else.

use std::path::PathBuf;

use mbv_ctrl::player::PlayerStatus;
use rstest::rstest;

use super::{client_publishes, resolve_art_url};

fn status(active: bool, paused: bool) -> PlayerStatus {
    PlayerStatus {
        active,
        paused,
        ..PlayerStatus::default()
    }
}

#[rstest]
#[case::local_endpoint(true, None, false)]
#[case::remote_with_local_playing(false, Some((true, false)), false)]
#[case::remote_with_local_paused(false, Some((true, true)), true)]
#[case::remote_with_no_local_status(false, None, true)]
fn publishing_rule(
    #[case] endpoint_is_local: bool,
    #[case] local: Option<(bool, bool)>,
    #[case] publishes: bool,
) {
    let local_status = local.map(|(active, paused)| status(active, paused));
    assert_eq!(
        client_publishes(endpoint_is_local, local_status.as_ref()),
        publishes
    );
}

// Regression guard: a Movie's queue card writes its art under the landscape
// key (`{id}:QB`), so MPRIS must fall back to that key after the portrait key
// misses instead of omitting `mpris:artUrl`.
#[test]
fn movie_art_cached_only_under_landscape_key_resolves() {
    let art_url = resolve_art_url("movie", "", |key| {
        (key == "movie:QB").then(|| PathBuf::from("/cache/movie-qb"))
    });
    assert_eq!(art_url.as_deref(), Some("file:///cache/movie-qb"));
}
