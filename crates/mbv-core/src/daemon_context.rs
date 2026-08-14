/// Process policy for the shared Player-owner daemon core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DaemonRole {
    Local,
    Packaged,
}

/// Owner-local Emby state. Constructing this value never authenticates; the
/// daemon may start even when the configured server is unavailable.
#[derive(Clone)]
pub struct EmbyOwnerContext {
    pub client: std::sync::Arc<std::sync::Mutex<crate::api::EmbyClient>>,
    pub generation: crate::service_runtime::SetupGeneration,
    pub revision: u64,
}

impl EmbyOwnerContext {
    pub fn from_client(client: crate::api::EmbyClient, revision: u64) -> Self {
        Self {
            client: std::sync::Arc::new(std::sync::Mutex::new(client)),
            generation: crate::service_runtime::SetupGeneration::default(),
            revision,
        }
    }

    pub fn from_packaged_storage_result(config: &crate::config::Config) -> Result<Self, String> {
        let setup = config
            .emby_setup
            .as_ref()
            .ok_or_else(|| "Emby setup is missing from owner storage".to_string())?;
        let token = crate::config::load_service_secret(crate::config::ServiceKind::Emby)
            .ok_or_else(|| "Emby Service secret is unavailable".to_string())?;
        if setup.server_url.trim().is_empty() || setup.user_id.trim().is_empty() {
            return Err("Emby setup is incomplete in owner storage".to_string());
        }
        let mut client = crate::api::EmbyClient::new(config.clone());
        client.config.server_url = setup.server_url.clone();
        client.user_id = setup.user_id.clone();
        client.token = token;
        Ok(Self::from_client(client, setup.revision))
    }
}

/// Common startup input for Local and packaged daemons.
#[derive(Clone)]
pub struct DaemonStartupContext {
    pub role: DaemonRole,
    pub config: crate::config::Config,
    pub emby: Option<EmbyOwnerContext>,
}

impl DaemonStartupContext {
    pub fn new(config: crate::config::Config, role: DaemonRole) -> Self {
        let emby = EmbyOwnerContext::from_packaged_storage_result(&config).ok();
        Self { role, config, emby }
    }
}
