use mbv_core::api::EmbyClient;
use mbv_core::config::{load_service_secret, EmbySetup, ServiceKind};
use mbv_core::service_runtime::{EmbyFailure, EmbyFailureClass, ServiceState, SetupGeneration};
use std::sync::mpsc;

pub(super) struct Completion {
    pub(super) generation: SetupGeneration,
    pub(super) result: Result<Startup, EmbyFailure>,
}

pub(super) struct Startup {
    pub(super) client: EmbyClient,
    pub(super) bootstrap: mbv_core::service_runtime::EmbyBootstrap,
    pub(super) setup: EmbySetup,
}

pub(super) struct SetupCompletion {
    pub(super) generation: SetupGeneration,
    pub(super) previous_state: ServiceState,
    pub(super) result: Result<Startup, String>,
}

pub(super) fn start_setup(
    config: crate::config::Config,
    server_url: String,
    username: String,
    password: String,
    generation: SetupGeneration,
    previous_state: ServiceState,
) -> mpsc::Receiver<SetupCompletion> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let probe = EmbyClient::new(config.clone());
            let exchange = probe.exchange_credentials_bounded(
                &server_url,
                &username,
                &password,
                EmbyClient::AUTHENTICATE_HARD_BOUND,
            )?;
            let setup = EmbySetup::new(&exchange.server_url, &exchange.user_id);
            let mut client = EmbyClient::new(config);
            client.apply_credential_exchange(&exchange);
            let bootstrap = client
                .load_startup_data_bounded(EmbyClient::AUTHENTICATE_HARD_BOUND)
                .map_err(|error| error.to_string())?;
            Ok(Startup {
                client,
                bootstrap,
                setup,
            })
        })();
        let _ = tx.send(SetupCompletion {
            generation,
            previous_state,
            result,
        });
    });
    rx
}

pub(super) fn setup_identity_allows_commit(
    existing: Option<&EmbySetup>,
    candidate: &EmbySetup,
) -> bool {
    existing.is_none_or(|existing| {
        EmbySetup::new(&existing.server_url, &existing.user_id) == *candidate
    })
}

pub(super) struct StartupReceiver {
    pub(super) generation: SetupGeneration,
    pub(super) rx: mpsc::Receiver<Completion>,
}

pub(super) fn start(config: crate::config::Config, generation: SetupGeneration) -> StartupReceiver {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = config
            .emby_setup
            .as_ref()
            .ok_or_else(|| EmbyFailure::unavailable("missing Emby Service setup"))
            .and_then(|setup| {
                load_service_secret(ServiceKind::Emby)
                    .ok_or_else(|| EmbyFailure::unavailable("missing Emby Service credential"))
                    .and_then(|token| authenticate(config.clone(), setup.clone(), token))
            });
        let _ = tx.send(Completion { generation, result });
    });
    StartupReceiver { generation, rx }
}

pub(super) fn initial_state(configured: bool, credential_present: bool) -> ServiceState {
    match (configured, credential_present) {
        (false, _) => ServiceState::NotConfigured,
        (true, true) => ServiceState::Connecting,
        (true, false) => ServiceState::NeedsAuthentication,
    }
}

/// Initial routing uses only validated setup and local feed subscriptions.
/// Legacy `[server]` data is intentionally not consulted.
pub(super) fn should_open_services(config: &crate::config::Config) -> bool {
    config.emby_setup.is_none() && config.feeds.is_empty()
}

pub(super) fn startup_status(state: ServiceState) -> &'static str {
    match state {
        ServiceState::NotConfigured => {
            "Emby is not configured; local and feed playback remain available"
        }
        ServiceState::Connecting => {
            "Emby is initializing; local and feed playback remain available"
        }
        ServiceState::Ready => "Emby is ready",
        ServiceState::NeedsAuthentication => {
            "Emby needs authentication; local and feed playback remain available"
        }
        ServiceState::Unavailable => {
            "Emby is unavailable; local and feed playback remain available"
        }
    }
}

fn authenticate(
    config: crate::config::Config,
    setup: EmbySetup,
    token: String,
) -> Result<Startup, EmbyFailure> {
    let client = EmbyClient::new(config);
    let client = client.authenticate_service_setup_bounded(
        token,
        &setup,
        EmbyClient::AUTHENTICATE_HARD_BOUND,
    )?;
    let bootstrap = client.load_startup_data_bounded(EmbyClient::AUTHENTICATE_HARD_BOUND)?;
    Ok(Startup {
        client,
        bootstrap,
        setup,
    })
}

pub(super) fn classify_failure(error: &EmbyFailure) -> ServiceState {
    match error.class {
        EmbyFailureClass::AuthenticationRejected => ServiceState::NeedsAuthentication,
        EmbyFailureClass::Unavailable => ServiceState::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_typed_auth_and_connectivity_failures() {
        assert_eq!(
            classify_failure(&EmbyFailure {
                class: EmbyFailureClass::AuthenticationRejected,
                message: "HTTP 401 unauthorized".into(),
            }),
            ServiceState::NeedsAuthentication
        );
        assert_eq!(
            classify_failure(&EmbyFailure::unavailable("timed out after 5s")),
            ServiceState::Unavailable
        );
    }

    #[test]
    fn no_setup_is_not_configured_and_does_not_need_authentication() {
        assert_eq!(initial_state(false, false), ServiceState::NotConfigured);
    }

    #[test]
    fn configured_credentials_start_connecting() {
        assert_eq!(initial_state(true, true), ServiceState::Connecting);
        assert_eq!(
            initial_state(true, false),
            ServiceState::NeedsAuthentication
        );
    }

    #[test]
    fn empty_setup_is_the_only_initial_services_route() {
        let mut config = crate::config::Config::default();
        assert!(should_open_services(&config));

        config.server_url = "https://legacy.example.test".into();
        assert!(should_open_services(&config));

        config.feeds.push(mbv_core::config::FeedSubscription {
            name: "News".into(),
            url: "https://example.test/feed".into(),
            kind: mbv_core::config::FeedKind::Audio,
        });
        assert!(!should_open_services(&config));

        config.feeds.clear();
        config.emby_setup = Some(EmbySetup::new("https://emby.example.test", "user-id"));
        assert!(!should_open_services(&config));
    }

    #[test]
    fn startup_status_is_truthful_for_each_emby_state() {
        assert!(!startup_status(ServiceState::NotConfigured).contains("initializing"));
        assert!(startup_status(ServiceState::Connecting).contains("initializing"));
        assert!(!startup_status(ServiceState::NeedsAuthentication).contains("initializing"));
        assert!(!startup_status(ServiceState::Unavailable).contains("initializing"));
        assert_eq!(startup_status(ServiceState::Ready), "Emby is ready");
    }
}
