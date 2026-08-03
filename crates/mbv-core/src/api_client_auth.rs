impl EmbyClient {
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
                self.token.clear();
                self.user_id.clear();
                Err(format!("Cached credential validation failed: {e}"))
            }
        }
    }

    /// Hard wall-clock bound for `authenticate_bounded`, independent of
    /// ureq's own connect/total timeouts (see issue #191: those don't
    /// reliably cover every stall mode, e.g. TLS handshake hangs).
    pub const AUTHENTICATE_HARD_BOUND: std::time::Duration = std::time::Duration::from_secs(15);

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
    /// user ID is required. The token is never persisted or logged.
    pub fn validate_shared_data_token(&self, token: &str) -> Result<String, String> {
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

        let resp = self
            .agent
            .get(&self.url("/Users/Me"))
            .set("Authorization", &auth_header)
            .set("X-Emby-Token", token)
            .call()
            .map_err(|e| format!("shared-data token validation failed: {e}"))?;

        let resp: serde_json::Value = resp
            .into_json()
            .map_err(|e| format!("shared-data token validation parse failed: {e}"))?;

        let user_id = resp["Id"].as_str().unwrap_or("").trim();
        if user_id.is_empty() {
            return Err(
                "shared-data token validation: /Users/Me returned empty user ID; \
                 API keys are not accepted for shared-data access"
                    .to_string(),
            );
        }

        Ok(user_id.to_string())
    }

    // ── Browse / fetch ───────────────────────────────────────────────────────
}
