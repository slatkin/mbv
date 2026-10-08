use mbv_config::save_home_latest_launch;

#[test]
fn home_latest_launch_rejects_zero_timestamp_as_launch_error() {
    let error = save_home_latest_launch(0).unwrap_err();

    assert!(error.is_launch());
}
