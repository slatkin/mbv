use crate::app::tests::*;

#[test]
fn session_direct_endpoint_prefers_advertised_tcp_port() {
    let app = make_app_stub();
    let mut sess = make_session("remote-host", "mbv");
    sess.host = "192.168.1.20".into();
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];
    assert_eq!(
        app.session_direct_endpoint(&sess),
        Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
            "192.168.1.20:47788".parse().unwrap()
        ))
    );
}

#[test]
fn session_direct_endpoint_rejects_non_mbv_without_local_fallback() {
    let app = make_app_stub();
    let sess = make_session("other-host", "Emby");
    assert_eq!(app.session_direct_endpoint(&sess), None);
}
