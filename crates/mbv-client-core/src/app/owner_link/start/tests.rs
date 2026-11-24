//! Unit tests for starting this machine's Owner process.

use super::{display_env_from, local_daemon_args};
use rstest::rstest;

#[rstest]
#[case::wayland_and_x11(
    "HOME=/h\nWAYLAND_DISPLAY=wayland-1\nDISPLAY=:0\nXAUTHORITY=/run/xauth\n",
    Some(vec![("WAYLAND_DISPLAY", "wayland-1"), ("DISPLAY", ":0"), ("XAUTHORITY", "/run/xauth")])
)]
#[case::no_graphical_session("HOME=/h\nPATH=/bin\n", None)]
fn display_env_is_read_from_the_user_manager(
    #[case] show_environment: &str,
    #[case] expected: Option<Vec<(&'static str, &str)>>,
) {
    let expected = expected.map(|vars| {
        vars.into_iter()
            .map(|(name, value)| (name, value.to_string()))
            .collect::<Vec<_>>()
    });
    assert_eq!(display_env_from(show_environment), expected);
}

#[test]
fn spawn_args_forward_only_an_explicit_log_level() {
    assert_eq!(local_daemon_args(None), Vec::<String>::new());
    assert_eq!(
        local_daemon_args(Some(
            mbv_core::applog::LogSpec::parse("debug,player=trace").expect("valid spec"),
        )),
        ["--log-level", "debug,player=trace"]
    );
}
