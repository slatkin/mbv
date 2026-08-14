#[cfg(test)]
mod shared_data_endpoint_tests {
    use super::{validate_shared_data_config, validate_shared_data_endpoint, Config};

    #[test]
    fn accepts_loopback_and_private_shared_endpoints_without_tls() {
        for endpoint in [
            "tcp://localhost:47789",
            "tcp://127.0.0.1:47789",
            "tcp://10.0.0.8:47789",
            "tcp://172.16.4.8:47789",
            "tcp://192.168.1.8:47789",
            "tcp://[fd12:3456::8]:47789",
        ] {
            assert!(
                validate_shared_data_endpoint(endpoint).is_ok(),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn rejects_wan_shared_endpoints() {
        for endpoint in [
            "tcp://0.0.0.0:47789",
            "tcp://8.8.8.8:47789",
            "tls://8.8.8.8:47789",
            "tcp://mbvd.example.test:47789",
        ] {
            assert!(
                validate_shared_data_endpoint(endpoint).is_err(),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn hosting_allows_plaintext_private_tcp_and_optional_tls() {
        let config = Config {
            shared_data_enabled: true,
            shared_data_listen: "192.168.1.8:47789".to_string(),
            ..Config::default()
        };
        assert!(validate_shared_data_config(&config).is_ok());

        let config = Config {
            shared_data_tls_cert_path: "cert.p12".to_string(),
            ..config
        };
        assert!(validate_shared_data_config(&config).is_err());
        let config = Config {
            shared_data_tls_key_path: "key.pem".to_string(),
            ..config
        };
        assert!(validate_shared_data_config(&config).is_ok());
    }

    #[test]
    fn shared_tcp_schemes_are_bindable_after_normalization() {
        for address in ["tcp://127.0.0.1:0", "tls://127.0.0.1:0"] {
            let listener = std::net::TcpListener::bind(super::shared_tcp_address(address)).unwrap();
            assert_eq!(
                listener.local_addr().unwrap().ip(),
                "127.0.0.1".parse::<std::net::IpAddr>().unwrap()
            );
        }
    }

    #[test]
    fn shared_data_uses_daemon_bind_address_with_its_own_port() {
        let config = Config {
            daemon_server_tcp_listen: "0.0.0.0:47788".to_string(),
            ..Config::default()
        };
        assert_eq!(super::shared_data_listen(&config), "0.0.0.0:47789");
    }
}
