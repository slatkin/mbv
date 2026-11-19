use mbv_emby::test_support::make_session;
use mbv_emby_model::test_support::make_item;

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
/// receiver id, an Emby Session by the output's device id, and the local row
/// only while no remote output holds the playback (design D4a). The Emby mark rides the session whose device the output names, not
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
    // Nothing is attached: the local row alone marks itself.
    let choices = app.queue_playback_projection().target_choices;
    assert_eq!(choices.len(), 4);
    assert_eq!(choices[0].key, ChooserTargetKey::Local);
    assert!(choices[0].is_active);
    assert!(choices[1..].iter().all(|choice| !choice.is_active));

    // A Cast output marks the receiver with its id.
    app.playback_output = Some(mbv_ctrl::OutputState::Cast {
        receiver_id: "receiver-9".to_string(),
        name: "Den TV".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    });
    let choices = app.queue_playback_projection().target_choices;
    let cast = choices
        .iter()
        .find(|choice| {
            choice.key == ChooserTargetKey::Target(SessionTargetKey::Cast("receiver-9".to_string()))
        })
        .expect("the cast receiver is a choice");
    assert!(cast.is_active);
    assert!(
        choices
            .iter()
            .filter(|choice| choice.key
                != ChooserTargetKey::Target(SessionTargetKey::Cast("receiver-9".to_string())))
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
        .find(|choice| {
            choice.key == ChooserTargetKey::Target(SessionTargetKey::Emby("session-2".to_string()))
        })
        .expect("the session is a choice");
    assert!(session.is_active);
    assert!(
        choices
            .iter()
            .filter(|choice| choice.key
                != ChooserTargetKey::Target(SessionTargetKey::Emby("session-2".to_string())))
            .all(|choice| !choice.is_active)
    );
}

/// The episode badge joins the season and episode with the canvas's middle
/// dot (`main` audit 7): `S4 · E1`, never a space.
#[test]
fn episode_badge_separates_season_and_episode_with_the_canvas_dot() {
    let mut item = make_item("Nether Zone", "Episode");
    item.parent_index_number = 4;
    item.index_number = 1;

    assert_eq!(episode_badge(&item).as_deref(), Some("S4 · E1"));
}
