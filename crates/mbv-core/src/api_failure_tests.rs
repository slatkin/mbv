use std::io::{Read, Write};
use std::net::TcpListener;

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
