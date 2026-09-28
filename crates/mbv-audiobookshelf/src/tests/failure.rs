use mbv_net::mock_http::MockHttp;

fn audiobookshelf_response(
    status: u16,
    body: &'static str,
) -> (MockHttp, crate::AudiobookshelfClient) {
    let http = MockHttp::new();
    let agent = http.agent();
    http.respond(status, body);
    let client = crate::AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(agent);
    (http, client)
}

#[test]
fn audiobookshelf_me_http_boundary_uses_bearer_and_redacts_failures() {
    use crate::AudiobookshelfFailureClass as Class;

    let cases = [
        (
            200,
            r#"{"id":"user-1","username":"reader","isActive":true}"#,
            None,
        ),
        (401, "", Some(Class::AuthenticationRejected)),
        (500, "server details", Some(Class::Server)),
        (404, "missing", Some(Class::Protocol)),
        (200, "not-json", Some(Class::MalformedResponse)),
    ];
    for (status, body, expected_class) in cases {
        let (http, client) = audiobookshelf_response(status, body);
        let result = client.me_bounded("test-api-key", std::time::Duration::from_secs(1));
        match expected_class {
            None => assert_eq!(
                result.unwrap(),
                crate::AudiobookshelfUser {
                    id: "user-1".into(),
                    username: "reader".into(),
                }
            ),
            Some(class) => {
                let error = result.unwrap_err();
                assert_eq!(error.class, class);
                for text in [format!("{error}"), format!("{error:?}")] {
                    assert!(!text.contains("test-api-key"));
                    assert!(!text.contains("Bearer"));
                    assert!(!text.contains("Authorization"));
                }
                assert!(std::error::Error::source(&error).is_none());
            }
        }
        // The bearer header must reach the wire even for the failure cases.
        assert!(
            http.requests()[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer test-api-key\r\n")
        );
    }
}

#[test]
fn dead_audiobookshelf_endpoint_is_connectivity() {
    use crate::AudiobookshelfFailureClass as Class;

    let http = MockHttp::new();
    let agent = http.agent();
    http.fail(std::io::ErrorKind::ConnectionRefused);
    let client = crate::AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(agent);
    assert_eq!(
        client
            .me_bounded("test-api-key", std::time::Duration::from_secs(1))
            .unwrap_err()
            .class,
        Class::Connectivity
    );
}
