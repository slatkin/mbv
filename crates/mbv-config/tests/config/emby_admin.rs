use mbv_config::EmbySetup;

#[test]
fn emby_setup_normalizes_server_and_starts_at_revision_one() {
    let setup = EmbySetup::new("  https://emby.example/// ", " user-1 ");
    assert_eq!(setup.server_url, "https://emby.example");
    assert_eq!(setup.user_id, "user-1");
    assert_eq!(setup.revision, 1);
}
