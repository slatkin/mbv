use crate::app::tests::make_app_stub;
use mbv_ctrl::BuildInfo;
use mbv_ui_model::settings::SettingKey;

/// An Owner that reports no build identity and no wire identity (an Owner from
/// before the wire identity) shows both as unknown in the Owner row.
#[test]
fn owner_row_shows_unknown_for_an_owner_without_build_identity() {
    let mut app = make_app_stub();
    let (mut remote, _player_rx, _cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.set_owner_build_for_test(BuildInfo {
        app_version: "1.2.3".into(),
        build_id: String::new(),
        wire_id: String::new(),
    });
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);

    assert_eq!(
        app.version_row_value(SettingKey::OwnerVersion),
        "1.2.3 \u{b7} unknown \u{b7} unknown"
    );
}
