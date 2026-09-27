use mbv_net::mock_http::MockHttp;

fn emby_client(http: &MockHttp) -> super::EmbyClient {
    let config = mbv_config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..Default::default()
    };
    super::EmbyClient::new(config).with_test_agent(http.agent())
}

#[test]
fn persisted_token_http_401_and_403_are_authentication_rejections() {
    for status in [401, 403] {
        let http = MockHttp::new();
        http.respond(status, "");
        let client = emby_client(&http);
        let Err(failure) = client.authenticate_service_setup_bounded(
            "persisted-token".into(),
            &mbv_config::EmbySetup::new("http://127.0.0.1:1", "user-id"),
            std::time::Duration::from_secs(1),
        ) else {
            panic!("rejected token unexpectedly authenticated");
        };
        assert_eq!(
            failure.class,
            crate::api::EmbyFailureClass::AuthenticationRejected
        );
    }
}

#[test]
fn persisted_token_http_5xx_transport_and_malformed_responses_are_unavailable() {
    let http = MockHttp::new();
    http.respond(500, "server failure");
    let client = emby_client(&http);
    let Err(failure) = client.authenticate_service_setup_bounded(
        "persisted-token".into(),
        &mbv_config::EmbySetup::new("http://127.0.0.1:1", "user-id"),
        std::time::Duration::from_secs(1),
    ) else {
        panic!("availability failure unexpectedly succeeded");
    };
    assert_eq!(failure.class, crate::api::EmbyFailureClass::Unavailable);

    let http = MockHttp::new();
    http.respond(200, "not-json");
    let client = emby_client(&http);
    let Err(failure) = client.get_views_classified() else {
        panic!("malformed availability response unexpectedly succeeded");
    };
    assert_eq!(failure.class, crate::api::EmbyFailureClass::Unavailable);

    let http = MockHttp::new();
    http.fail(std::io::ErrorKind::ConnectionRefused);
    let client = emby_client(&http);
    let Err(failure) = client.authenticate_service_setup_bounded(
        "persisted-token".into(),
        &mbv_config::EmbySetup::new("http://127.0.0.1:1", "user-id"),
        std::time::Duration::from_secs(1),
    ) else {
        panic!("dead endpoint unexpectedly authenticated");
    };
    assert_eq!(failure.class, crate::api::EmbyFailureClass::Unavailable);
}
