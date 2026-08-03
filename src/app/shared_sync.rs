use super::App;
use mbv_core::config::Config;
use mbv_core::shared_client::{SharedClient, SharedClientState, SharedWriteResult};
use mbv_core::shared_state::{
    load_roaming_settings_mirror, mirror_shared_document, RoamingSettings, SharedDocumentKind,
    SharedRecord, SharedSnapshotResponse,
};

impl App {
    pub(super) fn initialize_shared_state(&mut self) {
        let config = self.client.lock().unwrap().config.clone();
        let mut shared = SharedClient::new();
        shared.initialize(&config);
        if matches!(shared.state(), SharedClientState::Disabled) {
            return;
        }
        if let Some(settings) = load_roaming_settings_mirror() {
            self.apply_roaming_settings(settings);
        }

        let client = self.client.lock().unwrap().clone();
        let snapshot = match shared.connect(&config, &client) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                shared.enter_fallback();
                self.shared_client = Some(shared);
                self.flash_status_high(format!(
                    "Shared data unavailable; using local state ({error})"
                ));
                return;
            }
        };

        let local = local_snapshot(&config);
        let snapshot = match shared.initialize_missing_documents(snapshot, &local) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                shared.enter_fallback();
                self.shared_client = Some(shared);
                self.flash_status_high(format!(
                    "Shared data restore failed; using local state ({error})"
                ));
                return;
            }
        };

        if let Err(error) = self.apply_shared_snapshot(&snapshot) {
            shared.enter_fallback();
            self.shared_client = Some(shared);
            self.flash_status_high(format!(
                "Shared data mirror failed; using local state ({error})"
            ));
            return;
        }
        if let Some(record) = &snapshot.roaming_settings {
            if let Ok(settings) = serde_json::from_value::<RoamingSettings>(record.value.clone()) {
                log_roaming_mismatches(&settings);
            }
        }

        self.shared_client = Some(shared);
    }

    fn apply_shared_snapshot(&mut self, snapshot: &SharedSnapshotResponse) -> Result<(), String> {
        if let Some(record) = &snapshot.queue_state {
            self.apply_shared_queue(record)?;
        }
        if let Some(record) = &snapshot.library_position_state {
            self.apply_shared_library_position(record)?;
        }
        if let Some(record) = &snapshot.last_remote_connection {
            mirror_shared_document(SharedDocumentKind::LastRemoteConnection, &record.value)?;
        }
        if let Some(record) = &snapshot.roaming_settings {
            self.apply_shared_roaming_settings(record)?;
        }
        Ok(())
    }

    fn apply_shared_queue(&mut self, record: &SharedRecord) -> Result<(), String> {
        let state: crate::config::QueueState = serde_json::from_value(record.value.clone())
            .map_err(|e| format!("parse shared queue state: {e}"))?;
        mirror_shared_document(SharedDocumentKind::QueueState, &record.value)?;
        let cursor = super::actions::queue_restore_cursor(
            &state.items,
            state.cursor,
            state.last_played_item_id.as_deref(),
            state.last_played_completed,
        );
        self.last_played_item_id = state.last_played_item_id;
        self.last_played_completed = state.last_played_completed;
        self.queue_source = state.source;
        self.player_tab.set_items(state.items, cursor);
        self.queue_dirty = false;
        self.spawn_enrich_queue_state(state.positions);
        Ok(())
    }

    fn apply_shared_library_position(&mut self, record: &SharedRecord) -> Result<(), String> {
        let state: crate::config::LibraryPositionState =
            serde_json::from_value(record.value.clone())
                .map_err(|e| format!("parse shared library position state: {e}"))?;
        mirror_shared_document(SharedDocumentKind::LibraryPositionState, &record.value)?;
        self.library_position_state = state;
        Ok(())
    }

    fn apply_shared_roaming_settings(&mut self, record: &SharedRecord) -> Result<(), String> {
        let settings: RoamingSettings = serde_json::from_value(record.value.clone())
            .map_err(|e| format!("parse shared roaming settings: {e}"))?;
        mirror_shared_document(SharedDocumentKind::RoamingSettings, &record.value)?;
        self.apply_roaming_settings(settings);
        Ok(())
    }

    fn apply_roaming_settings(&mut self, settings: RoamingSettings) {
        self.library_routes = settings.library_routes.clone();
        let mut client = self.client.lock().unwrap();
        client.config.auto_reconnect = settings.auto_reconnect;
        client.config.library_routes = settings.library_routes;
    }

    pub(super) fn persist_shared_document(
        &mut self,
        kind: SharedDocumentKind,
        value: serde_json::Value,
    ) -> Result<(), String> {
        if self.shared_client.is_none() {
            return Ok(());
        }
        mirror_shared_document(kind, &value)?;

        let result = {
            let Some(shared) = self.shared_client.as_mut() else {
                return Ok(());
            };
            if !matches!(shared.state(), SharedClientState::Shared) {
                return Ok(());
            }
            let expected_revision = match kind {
                SharedDocumentKind::QueueState => shared.revisions().queue_state,
                SharedDocumentKind::LibraryPositionState => {
                    shared.revisions().library_position_state
                }
                SharedDocumentKind::LastRemoteConnection => {
                    shared.revisions().last_remote_connection
                }
                SharedDocumentKind::RoamingSettings => shared.revisions().roaming_settings,
            }
            .unwrap_or(0);
            shared.update_document_and_wait(kind, expected_revision, value)
        };
        match result {
            Ok(SharedWriteResult::Committed(record)) => {
                mirror_shared_document(kind, &record.value)?;
                Ok(())
            }
            Ok(SharedWriteResult::Stale(record)) => {
                self.apply_shared_record(kind, &record)?;
                self.flash_status_high(format!(
                    "Shared {} changed elsewhere; adopted the newer state",
                    kind.as_str()
                ));
                Ok(())
            }
            Err(error) => {
                if let Some(shared) = self.shared_client.as_mut() {
                    shared.enter_fallback();
                }
                self.flash_status_high(format!(
                    "Shared data unavailable; using local state ({error})"
                ));
                Err(error)
            }
        }
    }

    pub(super) fn persist_roaming_settings(&mut self) {
        let settings = {
            let client = self.client.lock().unwrap();
            RoamingSettings {
                auto_reconnect: client.config.auto_reconnect,
                library_routes: client.config.library_routes.clone(),
            }
        };
        if let Ok(value) = serde_json::to_value(settings) {
            let _ = self.persist_shared_document(SharedDocumentKind::RoamingSettings, value);
        }
    }

    pub(super) fn drain_shared_events(&mut self) -> bool {
        let mut changed = self.drain_shared_reconnect();
        let events = self
            .shared_client
            .as_mut()
            .map(|shared| shared.drain_events())
            .unwrap_or_default();
        for event in events {
            if matches!(
                event,
                mbv_core::shared_protocol::SharedDataEvent::ConnectionClosed
            ) {
                let was_shared = self
                    .shared_client
                    .as_ref()
                    .is_some_and(|shared| matches!(shared.state(), SharedClientState::Shared));
                if was_shared {
                    if let Some(shared) = self.shared_client.as_mut() {
                        shared.enter_fallback();
                    }
                    self.flash_status_high(
                        "Shared data connection closed; using local state".to_string(),
                    );
                    changed = true;
                }
                continue;
            }
            let mbv_core::shared_protocol::SharedDataEvent::DocumentNotification { kind, record } =
                event
            else {
                continue;
            };
            let newer = self
                .shared_client
                .as_ref()
                .and_then(|shared| revision_for(shared, kind))
                .map(|revision| record.revision > revision)
                .unwrap_or(true);
            if !newer {
                continue;
            }
            if let Some(shared) = self.shared_client.as_mut() {
                shared.apply_notification(kind, &record);
            }
            if let Err(error) = self.apply_shared_record(kind, &record) {
                log::warn!(target: "shared_data", "failed to apply shared notification: {error}");
                continue;
            }
            changed = true;
        }
        self.start_shared_reconnect_if_needed();
        changed
    }

    fn drain_shared_reconnect(&mut self) -> bool {
        let Some(rx) = &self.shared_reconnect_rx else {
            return false;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.shared_reconnect_rx = None;
                if let Some(shared) = self.shared_client.as_mut() {
                    shared.record_retry_failure();
                }
                return false;
            }
        };
        self.shared_reconnect_rx = None;

        match result {
            Ok((mut shared, snapshot)) => {
                if let Err(error) = self.apply_shared_snapshot(&snapshot) {
                    shared.enter_fallback();
                    self.shared_client = Some(shared);
                    log::warn!(target: "shared_data", "reconnected shared-data mirror failed: {error}");
                    return false;
                }
                if let Some(record) = &snapshot.roaming_settings {
                    if let Ok(settings) =
                        serde_json::from_value::<RoamingSettings>(record.value.clone())
                    {
                        log_roaming_mismatches(&settings);
                    }
                }
                self.shared_client = Some(shared);
                self.flash_status("Shared data reconnected".to_string());
                true
            }
            Err(error) => {
                if let Some(shared) = self.shared_client.as_mut() {
                    shared.record_retry_failure();
                }
                log::debug!(target: "shared_data", "shared-data reconnect failed: {error}");
                false
            }
        }
    }

    fn start_shared_reconnect_if_needed(&mut self) {
        if self.shared_reconnect_rx.is_some() {
            return;
        }
        let should_retry = self
            .shared_client
            .as_ref()
            .is_some_and(|shared| shared.should_retry());
        if !should_retry {
            return;
        }

        let config = self.client.lock().unwrap().config.clone();
        let client = self.client.lock().unwrap().clone();
        let local = local_snapshot(&config);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut shared = SharedClient::new();
            shared.initialize(&config);
            let result = shared
                .connect(&config, &client)
                .and_then(|snapshot| shared.initialize_missing_documents(snapshot, &local));
            let _ = tx.send(result.map(|snapshot| (shared, snapshot)));
        });
        self.shared_reconnect_rx = Some(rx);
    }

    fn apply_shared_record(
        &mut self,
        kind: SharedDocumentKind,
        record: &SharedRecord,
    ) -> Result<(), String> {
        match kind {
            SharedDocumentKind::QueueState => self.apply_shared_queue(record),
            SharedDocumentKind::LibraryPositionState => self.apply_shared_library_position(record),
            SharedDocumentKind::LastRemoteConnection => mirror_shared_document(kind, &record.value),
            SharedDocumentKind::RoamingSettings => self.apply_shared_roaming_settings(record),
        }
    }
}

fn revision_for(shared: &SharedClient, kind: SharedDocumentKind) -> Option<u64> {
    match kind {
        SharedDocumentKind::QueueState => shared.revisions().queue_state,
        SharedDocumentKind::LibraryPositionState => shared.revisions().library_position_state,
        SharedDocumentKind::LastRemoteConnection => shared.revisions().last_remote_connection,
        SharedDocumentKind::RoamingSettings => shared.revisions().roaming_settings,
    }
}

fn local_snapshot(config: &Config) -> SharedSnapshotResponse {
    SharedSnapshotResponse {
        queue_state: mbv_core::config::load_queue_state().map(record),
        library_position_state: Some(record(mbv_core::config::load_library_position_state())),
        last_remote_connection: mbv_core::config::load_last_remote_connection()
            .ok()
            .flatten()
            .map(record),
        roaming_settings: Some(record(load_roaming_settings_mirror().unwrap_or_else(
            || RoamingSettings {
                auto_reconnect: config.auto_reconnect,
                library_routes: config.library_routes.clone(),
            },
        ))),
    }
}

fn record<T: serde::Serialize>(value: T) -> SharedRecord {
    SharedRecord {
        revision: 0,
        value: serde_json::to_value(value).unwrap_or(serde_json::Value::Null),
    }
}

fn log_roaming_mismatches(settings: &RoamingSettings) {
    let path = mbv_core::config::config_path();
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(doc) = text.parse::<toml::Value>() else {
        return;
    };
    if let Some(local) = doc
        .get("session")
        .and_then(|session| session.get("auto_reconnect"))
        .and_then(toml::Value::as_bool)
    {
        if local != settings.auto_reconnect {
            log::warn!(
                target: "shared_data",
                "shared roaming setting mismatch: auto_reconnect local={local} shared={}",
                settings.auto_reconnect
            );
        }
    }
    let Some(routes) = doc.get("library_routes").and_then(toml::Value::as_table) else {
        return;
    };
    for (library, value) in routes {
        let Some(local) = value.as_str() else {
            continue;
        };
        if settings
            .library_routes
            .get(&library.to_lowercase())
            .map(String::as_str)
            != Some(local)
        {
            log::warn!(
                target: "shared_data",
                "shared roaming setting mismatch: library_routes[{library:?}] local={local:?} shared={:?}",
                settings.library_routes.get(&library.to_lowercase())
            );
        }
    }
}
