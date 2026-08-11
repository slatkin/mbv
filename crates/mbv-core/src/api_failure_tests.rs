use std::io::{Read, Write};
use std::net::TcpListener;

fn audiobookshelf_response(
    status: u16,
    body: &'static str,
) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 2048];
        let size = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..size]);
        assert!(request.contains("GET /api/me "));
        assert!(request.contains("Authorization: Bearer test-api-key\r\n"));
        write!(
            stream,
            "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (address, handle)
}

#[test]
fn audiobookshelf_me_http_boundary_uses_bearer_and_redacts_failures() {
    use crate::audiobookshelf::{AudiobookshelfClient, AudiobookshelfFailureClass as Class};

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
        let (address, server) = audiobookshelf_response(status, body);
        let client = AudiobookshelfClient::new(&format!("http://{address}")).unwrap();
        let result = client.me_bounded("test-api-key", std::time::Duration::from_secs(1));
        match expected_class {
            None => assert_eq!(
                result.unwrap(),
                crate::audiobookshelf::AudiobookshelfUser {
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
        server.join().unwrap();
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let client = AudiobookshelfClient::new(&format!("http://{address}")).unwrap();
    assert_eq!(
        client
            .me_bounded("test-api-key", std::time::Duration::from_secs(1))
            .unwrap_err()
            .class,
        Class::Connectivity
    );
}

#[test]
fn persisted_token_http_401_and_403_are_authentication_rejections() {
    for status in [401, 403] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status} Rejected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        });

        let mut config = crate::config::Config::default();
        config.server_url = format!("http://{address}");
        let client = super::EmbyClient::new(config);
        let failure = match client.authenticate_service_setup_bounded(
            "persisted-token".into(),
            &crate::config::EmbySetup::new(&format!("http://{address}"), "user-id"),
            std::time::Duration::from_secs(1),
        ) {
            Ok(_) => panic!("rejected token unexpectedly authenticated"),
            Err(failure) => failure,
        };
        assert_eq!(
            failure.class,
            crate::service_runtime::EmbyFailureClass::AuthenticationRejected
        );
    }
}

#[test]
fn persisted_token_http_5xx_transport_and_malformed_responses_are_unavailable() {
    for (status, body) in [(500, "server failure"), (200, "not-json")] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let mut config = crate::config::Config::default();
        config.server_url = format!("http://{address}");
        let client = super::EmbyClient::new(config);
        let failure = if status == 200 {
            match client.get_views_classified() {
                Ok(_) => panic!("malformed availability response unexpectedly succeeded"),
                Err(failure) => failure,
            }
        } else {
            match client.authenticate_service_setup_bounded(
                "persisted-token".into(),
                &crate::config::EmbySetup::new(&format!("http://{address}"), "user-id"),
                std::time::Duration::from_secs(1),
            ) {
                Ok(_) => panic!("availability failure unexpectedly succeeded"),
                Err(failure) => failure,
            }
        };
        assert_eq!(
            failure.class,
            crate::service_runtime::EmbyFailureClass::Unavailable
        );
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut config = crate::config::Config::default();
    config.server_url = format!("http://{address}");
    let client = super::EmbyClient::new(config);
    let failure = match client.authenticate_service_setup_bounded(
        "persisted-token".into(),
        &crate::config::EmbySetup::new(&format!("http://{address}"), "user-id"),
        std::time::Duration::from_secs(1),
    ) {
        Ok(_) => panic!("dead endpoint unexpectedly authenticated"),
        Err(failure) => failure,
    };
    assert_eq!(
        failure.class,
        crate::service_runtime::EmbyFailureClass::Unavailable
    );
}
