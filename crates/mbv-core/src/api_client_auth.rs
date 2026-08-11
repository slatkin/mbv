impl EmbyClient {
    fn service_failure(context: &str, error: ureq::Error) -> crate::service_runtime::EmbyFailure {
        let class = match error {
            ureq::Error::Status(401 | 403, _) => {
                crate::service_runtime::EmbyFailureClass::AuthenticationRejected
            }
            _ => crate::service_runtime::EmbyFailureClass::Unavailable,
        };
        crate::service_runtime::EmbyFailure {
            class,
            message: format!("{context}: {error}"),
        }
    }

    // ── HTTP infrastructure ──────────────────────────────────────────────────

    pub fn new(config: Config) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build();
        EmbyClient {
            config,
            user_id: String::new(),
            token: String::new(),
            device_name: device_name(),
            device_id: device_id(),
            chapter_api_available: false,
            agent,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.server_url, path)
    }

    fn auth_header(&self) -> String {
        format!(
            "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\", Token=\"{}\"",
            self.device_name,
            self.device_id,
            env!("CARGO_PKG_VERSION"),
            self.token
        )
    }

    fn get(&self, path: &str) -> ureq::Request {
        self.agent
            .get(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    fn post(&self, path: &str) -> ureq::Request {
        self.agent
            .post(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    fn with_request_timeout(&self, timeout: std::time::Duration) -> Self {
        let mut client = self.clone();
        client.agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout(timeout)
            .build();
        client
    }

    fn delete(&self, path: &str) -> ureq::Request {
        self.agent
            .delete(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    // ── Authentication ───────────────────────────────────────────────────────

    pub fn authenticate(&mut self) -> Result<(), String> {
        let Some((cached_url, token, user_id)) = load_cached_token() else {
            return Err("No cached credentials".to_string());
        };

        if self.config.server_url.is_empty() {
            if cached_url.is_empty() {
                return Err("No server URL configured".to_string());
            }
            self.config.server_url = cached_url;
        }

        self.token = token;
        self.user_id = user_id;

        match self.get(&format!("/Users/{}", self.user_id)).call() {
            Ok(resp) => {
                if let Ok(user) = resp.into_json::<Value>() {
                    if let Some(name) = user["Name"].as_str().filter(|name| !name.is_empty()) {
                        self.config.username = name.to_string();
                    }
                }
                Ok(())
            }
            Err(ureq::Error::Status(401 | 403, _)) => {
                clear_cached_token();
                self.token.clear();
                self.user_id.clear();
                Err("Cached credentials expired".to_string())
            }
            Err(e) => {
                // Connectivity failure (timeout/refused/DNS/TLS): the token
                // itself is untouched and may still be valid, so keep it in
                // memory and on disk for the next attempt (issue #192). Only
                // 401/403 (above) counts as "credentials expired".
                Err(format!("Cached credential validation failed: {e}"))
            }
        }
    }

    /// Hard wall-clock bound for `authenticate_bounded`, independent of
    /// ureq's own connect/total timeouts (see issue #191: those don't
    /// reliably cover every stall mode, e.g. TLS handshake hangs). 5s
    /// matches the connect timeout; it is the worst-case wait before mbv
    /// reports the server unreachable (issue #192).
    pub const AUTHENTICATE_HARD_BOUND: std::time::Duration = std::time::Duration::from_secs(5);

    /// Runs `authenticate()` on a clone, bounded by `hard_bound` wall-clock
    /// time. On success, returns the authenticated clone -- callers should
    /// use it in place of the original, since `self` is never mutated. On
    /// timeout (or any other failure), `self` is left untouched.
    pub fn authenticate_bounded(
        &self,
        hard_bound: std::time::Duration,
    ) -> Result<EmbyClient, String> {
        let mut clone = self.clone();
        crate::bounded::run_with_hard_bound(
            move || clone.authenticate().map(|()| clone),
            hard_bound,
        )
    }

    /// Validate a token from the Service secret store without consulting the
    /// legacy token cache. The whole operation is bounded so startup can leave
    /// the TUI responsive even when Emby is unreachable.
    pub fn authenticate_service_token_bounded(
        &self,
        token: String,
        hard_bound: std::time::Duration,
    ) -> Result<EmbyClient, String> {
        let mut clone = self.clone();
        clone.token = token;
        crate::bounded::run_with_hard_bound(
            move || {
                let users: Value = clone
                    .get("/Users")
                    .call()
                    .map_err(|e| format!("service credential validation failed: {e}"))?
                    .into_json()
                    .map_err(|e| format!("service credential response failed: {e}"))?;
                let users = users
                    .as_array()
                    .ok_or_else(|| "service credential response was not a user list".to_string())?;
                let user = clone
                    .config
                    .username
                    .is_empty()
                    .then(|| users.first())
                    .flatten()
                    .or_else(|| {
                        users.iter().find(|user| {
                            user["Name"].as_str().is_some_and(|name| {
                                name.eq_ignore_ascii_case(&clone.config.username)
                            })
                        })
                    })
                    .ok_or_else(|| "no matching Emby user".to_string())?;
                clone.user_id = user["Id"].as_str().unwrap_or_default().to_string();
                if let Some(name) = user["Name"].as_str() {
                    clone.config.username = name.to_string();
                }
                Ok(clone)
            },
            hard_bound,
        )
    }

    /// Validate a token against the persisted provider identity. Unlike the
    /// older compatibility method, this never rediscovers the user via `/Users`.
    pub fn authenticate_service_setup_bounded(
        &self,
        token: String,
        setup: &crate::config::EmbySetup,
        hard_bound: std::time::Duration,
    ) -> Result<EmbyClient, crate::service_runtime::EmbyFailure> {
        let mut clone = self.clone();
        clone.config.server_url = setup.server_url.trim_end_matches('/').to_string();
        clone.user_id = setup.user_id.clone();
        clone.token = token;
        crate::bounded::run_with_hard_bound(
            move || {
                clone
                    .get(&format!("/Users/{}", clone.user_id))
                    .call()
                    .map_err(|e| {
                        Self::service_failure("service credential validation failed", e)
                    })?;
                Ok(clone)
            },
            hard_bound,
        )
    }

    /// Exchange transient setup credentials for an Emby Service credential.
    /// This deliberately has no persistence side effect: callers decide when
    /// the validated token is safe to commit.
    pub fn exchange_credentials_bounded(
        &self,
        server_url: &str,
        username: &str,
        password: &str,
        hard_bound: std::time::Duration,
    ) -> Result<EmbyCredentialExchange, String> {
        let mut client = self.clone();
        client.config.server_url = server_url.trim().trim_end_matches('/').to_string();
        let username = username.trim().to_string();
        let password = password.to_string();
        if client.config.server_url.is_empty() || username.is_empty() || password.is_empty() {
            return Err("server URL, username, and password are required".to_string());
        }
        crate::bounded::run_with_hard_bound(
            move || {
                let resp: Value = client
                    .agent
                    .post(&client.url("/Users/AuthenticateByName"))
                    .set(
                        "Authorization",
                        &format!(
                            "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
                            client.device_name,
                            client.device_id,
                            env!("CARGO_PKG_VERSION")
                        ),
                    )
                    .send_json(ureq::json!({"Username": username, "Pw": password}))
                    .map_err(|e| format!("Emby authentication failed: {e}"))?
                    .into_json()
                    .map_err(|e| format!("Emby authentication response parse failed: {e}"))?;
                let token = resp["AccessToken"]
                    .as_str()
                    .map(str::trim)
                    .filter(|token| !token.is_empty())
                    .ok_or_else(|| "Emby authentication returned an empty token".to_string())?;
                let user_id = resp["User"]["Id"]
                    .as_str()
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| "Emby authentication returned an empty user ID".to_string())?;
                Ok(EmbyCredentialExchange {
                    server_url: client.config.server_url,
                    user_id: user_id.to_string(),
                    token: token.to_string(),
                })
            },
            hard_bound,
        )
    }

    pub fn apply_credential_exchange(&mut self, exchange: &EmbyCredentialExchange) {
        self.config.server_url = exchange.server_url.clone();
        self.config.username.clear();
        self.config.password.clear();
        self.config.api_key.clear();
        self.user_id = exchange.user_id.clone();
        self.token = exchange.token.clone();
    }

    /// Load the minimum Home/library data required by the existing TUI after
    /// Service authentication. This runs as one bounded operation so no
    /// Emby request is made from the event loop.
    pub fn load_startup_data_bounded(
        &self,
        hard_bound: std::time::Duration,
    ) -> Result<crate::service_runtime::EmbyBootstrap, crate::service_runtime::EmbyFailure> {
        let client = self.clone();
        crate::bounded::run_with_hard_bound(
            move || {
                let continue_items = client.get_continue_watching(20).unwrap_or_default();
                let views = client.get_views_classified()?;
                let user_views = client.get_user_views().unwrap_or_default();
                let latest = user_views
                    .into_iter()
                    .filter(|view| view.collection_type != "playlists")
                    .map(|view| {
                        let items = if view.collection_type == "tvshows" {
                            client.get_latest_episodes(&view.id, 30).unwrap_or_default()
                        } else {
                            client.get_latest(&view.id, 30).unwrap_or_default()
                        };
                        (view.name, view.id, items)
                    })
                    .collect();
                Ok(crate::service_runtime::EmbyBootstrap {
                    continue_items,
                    views,
                    latest,
                })
            },
            hard_bound,
        )
    }

    // Authenticate using credentials in self.config (password or api_key).
    // Does not check the token cache. Saves a fresh token to the cache on success.
    // Called by authenticate() on cache miss, and directly by the login screen.
    pub fn authenticate_credentials(&mut self) -> Result<(), String> {
        // Prefer password auth: yields a user-scoped token so sessions are attributed to the
        // correct user (required for activity tracking and progress saving).
        // API key auth yields an admin token with no user association — use only as fallback.
        if !self.config.password.is_empty() {
            let resp: Value = self
                .agent
                .post(&self.url("/Users/AuthenticateByName"))
                .set(
                    "Authorization",
                    &format!(
                        "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
                        self.device_name,
                        self.device_id,
                        env!("CARGO_PKG_VERSION")
                    ),
                )
                .send_json(ureq::json!({
                    "Username": self.config.username,
                    "Pw": self.config.password,
                }))
                .map_err(|e| format!("Auth failed: {e}"))?
                .into_json()
                .map_err(|e| format!("Auth parse failed: {e}"))?;
            self.token = resp["AccessToken"].as_str().unwrap_or("").to_string();
            self.user_id = resp["User"]["Id"].as_str().unwrap_or("").to_string();
            if let Some(name) = resp["User"]["Name"]
                .as_str()
                .filter(|name| !name.is_empty())
            {
                self.config.username = name.to_string();
            }
            save_cached_token(&self.config.server_url, &self.token, &self.user_id);
        } else if !self.config.api_key.is_empty() {
            self.token = self.config.api_key.clone();
            let users: Value = self
                .agent
                .get(&self.url("/Users"))
                .query("api_key", &self.token)
                .call()
                .map_err(|e| format!("Auth failed: {e}"))?
                .into_json()
                .map_err(|e| format!("Auth parse failed: {e}"))?;
            let users = users.as_array().ok_or("Expected array of users")?;
            if users.is_empty() {
                return Err("No users found on server".to_string());
            }
            if !self.config.username.is_empty() {
                let uname = self.config.username.to_lowercase();
                let found = users
                    .iter()
                    .find(|u| u["Name"].as_str().unwrap_or("").to_lowercase() == uname);
                match found {
                    Some(u) => {
                        self.user_id = u["Id"].as_str().unwrap_or("").to_string();
                        if let Some(name) = u["Name"].as_str().filter(|name| !name.is_empty()) {
                            self.config.username = name.to_string();
                        }
                    }
                    None => return Err(format!("User '{}' not found", self.config.username)),
                }
            } else {
                self.user_id = users[0]["Id"].as_str().unwrap_or("").to_string();
                if let Some(name) = users[0]["Name"].as_str().filter(|name| !name.is_empty()) {
                    self.config.username = name.to_string();
                }
            }
        } else {
            return Err("No credentials configured".to_string());
        }
        Ok(())
    }

    /// Fetch the current user's subtitle and audio language preferences from Emby.
    pub fn get_user_subtitle_prefs(&self) -> Result<crate::player::SubtitlePrefs, String> {
        let resp: serde_json::Value = self
            .get("/Users/Me")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let cfg = &resp["Configuration"];
        let mode = cfg["SubtitleMode"]
            .as_str()
            .unwrap_or("Default")
            .to_string();
        let subtitle_lang = cfg["SubtitleLanguagePreference"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let audio_lang = cfg["AudioLanguagePreference"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(crate::player::SubtitlePrefs {
            mode,
            subtitle_lang,
            audio_lang,
        })
    }

    pub fn validate_presented_token(&self, token: &str) -> Result<String, String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("missing Emby auth token".to_string());
        }

        let auth_header = format!(
            "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\", Token=\"{}\"",
            self.device_name,
            self.device_id,
            env!("CARGO_PKG_VERSION"),
            token
        );

        let me_resp = self
            .agent
            .get(&self.url("/Users/Me"))
            .set("Authorization", &auth_header)
            .set("X-Emby-Token", token)
            .call();
        if let Ok(resp) = me_resp {
            let resp: serde_json::Value = resp
                .into_json()
                .map_err(|e| format!("presented Emby token validation parse failed: {e}"))?;
            let user_id = resp["Id"].as_str().unwrap_or("").trim();
            if !user_id.is_empty() {
                return Ok(user_id.to_string());
            }
        }

        let users_resp = self
            .agent
            .get(&self.url("/Users"))
            .query("api_key", token)
            .call()
            .map_err(|e| format!("presented Emby token rejected: {e}"))?;
        let users: serde_json::Value = users_resp
            .into_json()
            .map_err(|e| format!("presented Emby token API-key validation parse failed: {e}"))?;
        let users = users
            .as_array()
            .ok_or("presented Emby token API-key validation expected user array")?;
        if users.is_empty() {
            return Err("presented Emby token API-key validation returned no users".to_string());
        }
        Ok(users[0]["Id"].as_str().unwrap_or("").to_string())
    }

    /// Validate a presented Emby token for shared-data access. Unlike
    /// `validate_presented_token`, this does NOT fall back to API-key user
    /// list validation — a successful `/Users/Me` response with a non-empty
    /// user ID is required unless the peer supplies an explicit user ID using
    /// the shared-data user-id capability. The token is never persisted or
    /// logged.
    pub fn validate_shared_data_token(
        &self,
        token: &str,
        expected_user_id: Option<&str>,
    ) -> Result<String, String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("missing Emby auth token".to_string());
        }

        let user_path = match expected_user_id {
            Some(user_id) if !user_id.trim().is_empty() => {
                if !user_id
                    .chars()
                    .all(|ch| ch.is_ascii_hexdigit() || ch == '-')
                {
                    return Err("shared-data user ID contains invalid characters".to_string());
                }
                format!("/Users/{user_id}")
            }
            _ => "/Users/Me".to_string(),
        };

        let auth_header = format!(
            "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\", Token=\"{}\"",
            self.device_name,
            self.device_id,
            env!("CARGO_PKG_VERSION"),
            token
        );

        let resp = self
            .agent
            .get(&self.url(&user_path))
            .set("Authorization", &auth_header)
            .set("X-Emby-Token", token)
            .call()
            .map_err(|e| format!("shared-data token validation failed: {e}"))?;

        let resp: serde_json::Value = resp
            .into_json()
            .map_err(|e| format!("shared-data token validation parse failed: {e}"))?;

        let user_id = resp["Id"].as_str().unwrap_or("").trim();
        if user_id.is_empty() {
            return Err("shared-data token validation returned empty user ID; \
                 API keys are not accepted for shared-data access"
                .to_string());
        }

        if let Some(expected_user_id) = expected_user_id.filter(|id| !id.trim().is_empty()) {
            if user_id != expected_user_id {
                return Err("shared-data token validation returned a different user ID".to_string());
            }
        }

        Ok(user_id.to_string())
    }

    // ── Browse / fetch ───────────────────────────────────────────────────────
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbyCredentialExchange {
    pub server_url: String,
    pub user_id: String,
    pub token: String,
}
