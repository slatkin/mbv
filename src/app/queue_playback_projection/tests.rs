use mbv_emby::test_support::make_session;
use mbv_emby_model::TICKS_PER_SECOND;

use super::*;
use crate::app::tests::make_app_stub;

/// One discovered cast receiver with the identity the chooser keys it by.
fn receiver(id: &str, name: &str) -> mbv_cast::discovery::CastReceiver {
    mbv_cast::discovery::CastReceiver {
        id: id.to_string(),
        friendly_name: name.to_string(),
        host: "192.168.0.50".to_string(),
        port: 8009,
    }
}

/// The chooser marks the Owner's attached output: a Cast receiver by its
/// receiver id, an Emby Session by the output's device id, and nothing while
/// the output is local (the chooser's rows are remote targets only, design
/// D4a). The Emby mark rides the session whose device the output names, not
/// a name comparison.
#[test]
fn target_choices_mark_the_attached_output() {
    let mut app = make_app_stub();
    // The chooser's cast half rides the Owner's playback-output capability;
    // the stub's baseline advertises none, so the test swaps in a remote
    // whose capability list carries it.
    let (mut remote, _event_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
    remote.set_ctrl_compatibility_for_test(mbv_ctrl::CtrlCompatibility {
        supports_playback_output: true,
        ..mbv_ctrl::CtrlCompatibility::current()
    });
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    let mut local_session = make_session("woims", "Emby for mbv");
    local_session.id = "session-1".to_string();
    local_session.device_id = "device-1".to_string();
    let mut other_session = make_session("poo", "Emby for mbv");
    other_session.id = "session-2".to_string();
    other_session.device_id = "device-2".to_string();
    app.sessions = vec![local_session, other_session];
    app.cast_receivers = vec![receiver("receiver-9", "Den TV")];
    app.rebuild_panel_targets();
    // Nothing is attached: no choice marks itself.
    let choices = app.queue_playback_projection().target_choices;
    assert_eq!(choices.len(), 3);
    assert!(choices.iter().all(|choice| !choice.is_active));

    // A Cast output marks the receiver with its id.
    app.playback_output = Some(mbv_ctrl::OutputState::Cast {
        receiver_id: "receiver-9".to_string(),
        name: "Den TV".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    });
    let choices = app.queue_playback_projection().target_choices;
    let cast = choices
        .iter()
        .find(|choice| choice.key == SessionTargetKey::Cast("receiver-9".to_string()))
        .expect("the cast receiver is a choice");
    assert!(cast.is_active);
    assert!(
        choices
            .iter()
            .filter(|choice| choice.key != SessionTargetKey::Cast("receiver-9".to_string()))
            .all(|choice| !choice.is_active)
    );

    // An Emby Session output marks the session with the output's device id.
    app.playback_output = Some(mbv_ctrl::OutputState::EmbySession {
        device_id: "device-2".to_string(),
        name: "Living Room TV".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
        audio_only: false,
        now_playing: None,
    });
    let choices = app.queue_playback_projection().target_choices;
    let session = choices
        .iter()
        .find(|choice| choice.key == SessionTargetKey::Emby("session-2".to_string()))
        .expect("the session is a choice");
    assert!(session.is_active);
    assert!(
        choices
            .iter()
            .filter(|choice| choice.key != SessionTargetKey::Emby("session-2".to_string()))
            .all(|choice| !choice.is_active)
    );
}

/// The tile a position names inside a trickplay grid (task 6.7): the Emby web
/// client's arithmetic — the thumbnail is `ticks / interval`, and the sheet's
/// capacity divides it into a sheet ordinal plus the tile's place in that
/// sheet.
#[rstest::rstest]
#[case::first_thumbnail(0, TrickplayTile { sheet: 0, column: 0, row: 0 })]
#[case::middle_thumbnail(
    11 * 10 * TICKS_PER_SECOND,
    TrickplayTile { sheet: 0, column: 1, row: 1 }
)]
#[case::second_sheet(
    100 * 10 * TICKS_PER_SECOND,
    TrickplayTile { sheet: 1, column: 0, row: 0 }
)]
#[case::past_the_last_thumbnail(
    400 * 10 * TICKS_PER_SECOND,
    TrickplayTile { sheet: 1, column: 9, row: 9 }
)]
fn tile_at_names_the_thumbnail_and_its_place_in_the_sheet(
    #[case] ticks: i64,
    #[case] expected: TrickplayTile,
) {
    assert_eq!(
        TrickplayGrid {
            // 200 thumbnails at a ten-second interval: 100 per 10x10 sheet.
            thumbnail_count: 200,
            ..grid()
        }
        .tile_at(ticks),
        Some(expected)
    );
}

/// A grid with no usable geometry answers no tile, so the preview paints no
/// frame: a zero interval has no thumbnail series, and a zero tile size has no
/// sheet capacity.
#[test]
fn tile_at_needs_an_interval_a_tile_size_and_a_thumbnail() {
    assert_eq!(
        TrickplayGrid {
            interval_ms: 0,
            ..grid()
        }
        .tile_at(TICKS_PER_SECOND),
        None
    );
    assert_eq!(
        TrickplayGrid {
            thumbnail_count: 0,
            ..grid()
        }
        .tile_at(TICKS_PER_SECOND),
        None
    );
}

/// One usable grid: a 320-wide thumbnail series of 10x10 sheets.
fn grid() -> TrickplayGrid {
    TrickplayGrid {
        width: 320,
        height: 180,
        tile_width: 10,
        tile_height: 10,
        thumbnail_count: 200,
        interval_ms: 10_000,
        media_source_id: String::new(),
    }
}
