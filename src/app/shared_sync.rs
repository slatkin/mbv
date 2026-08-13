use super::notify_actions::ToastSeverity;
use super::App;
use mbv_core::config::Config;
use mbv_core::shared_client::{SharedClient, SharedClientState, SharedWriteResult};
use mbv_core::shared_state::{
    load_roaming_settings_mirror, mirror_shared_document, RoamingSettings, SharedDocumentKind,
    SharedRecord, SharedSnapshotResponse,
};

impl App {
    pub(super) fn initialize_shared_state(&mut self) {
        let config = self.config.lock().unwrap().clone();
        let mut shared = SharedClient::new();
        shared.initialize(&config);
        if matches!(shared.state(), SharedClientState::Disabled) {
            return;
        }
        if let Some(settings) = load_roaming_settings_mirror() {
            self.apply_roaming_settings(settings);
        }

        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let snapshot = match shared.connect(&config, &client) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                shared.enter_fallback();
                self.shared_client = Some(shared);
                log::warn!(target: "shared_data", "shared-data connect failed: {error}");
                self.flash(
                    format!("Shared data unavailable; using local state ({error})"),
                    ToastSeverity::Warning,
                );
                return;
            }
        };

        let local = local_snapshot(&config);
        let snapshot = match shared.initialize_missing_documents(snapshot, &local) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                shared.enter_fallback();
                self.shared_client = Some(shared);
                log::warn!(target: "shared_data", "shared-data initialization failed: {error}");
                self.flash(
                    format!("Shared data restore failed; using local state ({error})"),
                    ToastSeverity::Warning,
                );
                return;
            }
        };

        if let Err(error) = self.apply_shared_snapshot(&snapshot) {
            shared.enter_fallback();
            self.shared_client = Some(shared);
            self.flash(
                format!("Shared data mirror failed; using local state ({error})"),
                ToastSeverity::Warning,
            );
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
        if self.is_local_daemon() {
            // The daemon's queue is live playback state. A shared snapshot is
            // only a persistence mirror here, not an authority that can
            // replace the queue we just attached to.
            return Ok(());
        }
        let state: crate::config::QueueState = serde_json::from_value(record.value.clone())
            .map_err(|e| format!("parse shared queue state: {e}"))?;
        mirror_shared_document(SharedDocumentKind::QueueState, &record.value)?;
        let queue_items = state.items;
        let cursor = super::actions::queue_restore_cursor(
            &queue_items,
            state.cursor,
            state.last_played_content_id.as_ref(),
            state.last_played_item_id.as_deref(),
            state.last_played_completed,
        );
        self.last_played_item_id = state.last_played_item_id;
        self.last_played_completed = state.last_played_completed;
        self.queue_source = state.source;
        // A shared replacement may recycle QueueSlotIds. Retire the old
        // projection before installing it so late consume outcomes cannot
        // alias a slot in this new queue lineage.
        self.retire_remote_tracking(true);
        self.player_tab.set_queue_items(queue_items, cursor);
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
        let mut config = self.config.lock().unwrap();
        config.auto_reconnect = settings.auto_reconnect;
        config.library_routes = settings.library_routes;
    }

    pub(super) fn persist_shared_document(
        &mut self,
        kind: SharedDocumentKind,
        value: serde_json::Value,
    ) -> Result<(), String> {
        if self.shared_client.is_none() {
            log::debug!(target: "shared_data", "shared-data persistence skipped: no client");
            return Ok(());
        }
        mirror_shared_document(kind, &value)?;

        let result = {
            let Some(shared) = self.shared_client.as_mut() else {
                log::debug!(target: "shared_data", "shared-data persistence skipped: client missing");
                return Ok(());
            };
            if !matches!(shared.state(), SharedClientState::Shared) {
                log::debug!(
                    target: "shared_data",
                    "shared-data persistence skipped: client state={:?}",
                    shared.state()
                );
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
                self.flash(
                    format!(
                        "Shared {} changed elsewhere; adopted the newer state",
                        kind.as_str()
                    ),
                    ToastSeverity::Warning,
                );
                Ok(())
            }
            Err(error) => {
                if let Some(shared) = self.shared_client.as_mut() {
                    shared.enter_fallback();
                }
                self.flash(
                    format!("Shared data unavailable; using local state ({error})"),
                    ToastSeverity::Warning,
                );
                Err(error)
            }
        }
    }

    pub(super) fn persist_roaming_settings(&mut self) {
        let settings = {
            let config = self.config.lock().unwrap();
            RoamingSettings {
                auto_reconnect: config.auto_reconnect,
                library_routes: config.library_routes.clone(),
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
                    self.flash(
                        "Shared data connection closed; using local state".to_string(),
                        ToastSeverity::Warning,
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
                let mut snapshot = snapshot;
                if snapshot.queue_state.as_ref().is_some_and(|record| {
                    record.value["items"].as_array().is_some_and(Vec::is_empty)
                }) && !self.player_tab.queue.is_empty()
                {
                    log::warn!(
                        target: "shared_data",
                        "preserving non-empty local queue over empty reconnect snapshot"
                    );
                    snapshot.queue_state = None;
                }
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
                self.flash(
                    "Shared data reconnected".to_string(),
                    ToastSeverity::Success,
                );
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

        let config = self.config.lock().unwrap().clone();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
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

    /// Read stored feed-entry state from the shared daemon and copy the
    /// returned position/played values into the entry. Returns the entry
    /// (possibly mutated). Missing identity, absent state, unsupported
    /// capability, disconnection, or read failure all leave the entry
    /// stateless without rejecting playback or contacting Emby.
    pub(super) fn hydrate_feed_entry_state(
        &mut self,
        mut entry: mbv_core::playback_queue::FeedEntry,
    ) -> mbv_core::playback_queue::FeedEntry {
        let (Some(feed_id), Some(shared)) = (&entry.feed_id, self.shared_client.as_mut()) else {
            return entry;
        };
        if !matches!(shared.state(), SharedClientState::Shared) {
            return entry;
        }
        match shared.get_feed_entry(feed_id.clone(), entry.guid.clone()) {
            Ok(Some(state)) => {
                entry.position_ticks = state.position_ticks;
                entry.played = state.played;
                log::info!(
                    target: "feed_state",
                    "hydrated feed entry guid={} feed_id={} pos={}s played={}",
                    entry.guid,
                    feed_id,
                    state.position_ticks / mbv_core::api::TICKS_PER_SECOND,
                    state.played,
                );
            }
            Ok(None) => {
                log::debug!(
                    target: "feed_state",
                    "no stored state for feed entry guid={} feed_id={}",
                    entry.guid,
                    feed_id,
                );
            }
            Err(e) => {
                log::warn!(
                    target: "feed_state",
                    "feed state read failed for guid={} feed_id={}: {e}; playing statelessly",
                    entry.guid,
                    feed_id,
                );
            }
        }
        entry
    }

    /// Bulk-hydrate a subscription's fetched entries by scanning the shared
    /// store once for the given `feed_id` and merging matching rows by entry
    /// GUID. Unknown stored GUIDs are ignored. Missing capability,
    /// disconnection, timeout, or scan failure leaves entries unchanged
    /// (zero position, unplayed) without blocking the refresh.
    pub(super) fn hydrate_feed_entries_for_subscription(
        &mut self,
        feed_id: &str,
        entries: &mut [mbv_core::playback_queue::FeedEntry],
    ) {
        let Some(shared) = self.shared_client.as_mut() else {
            return;
        };
        if !matches!(shared.state(), SharedClientState::Shared) {
            return;
        }
        let scanned = match shared.scan_feed_entries(feed_id.to_string()) {
            Ok(Some(rows)) => rows,
            Ok(None) => {
                log::debug!(
                    target: "feed_state",
                    "feed-entry scan unsupported; leaving entries unplayed feed_id={feed_id}",
                );
                return;
            }
            Err(e) => {
                log::warn!(
                    target: "feed_state",
                    "feed-entry scan failed for feed_id={feed_id}: {e}; leaving entries unplayed",
                );
                return;
            }
        };
        if scanned.is_empty() {
            return;
        }
        let lookup: std::collections::HashMap<&str, &mbv_core::shared_store::FeedEntryState> =
            scanned
                .iter()
                .map(|(guid, state)| (guid.as_str(), state))
                .collect();
        let mut hydrated = 0usize;
        for entry in entries.iter_mut() {
            if let Some(state) = lookup.get(entry.guid.as_str()) {
                entry.position_ticks = state.position_ticks;
                entry.played = state.played;
                hydrated += 1;
            }
        }
        if hydrated > 0 {
            log::info!(
                target: "feed_state",
                "bulk-hydrated {hydrated} entries for feed_id={feed_id}",
            );
        }
    }

    /// Write feed-entry state to the shared daemon. Failures are logged and
    /// discarded — they never stop playback.
    pub(super) fn write_feed_entry_state(
        &mut self,
        feed_id: &str,
        entry_guid: &str,
        position_ticks: i64,
        played: bool,
    ) {
        let Some(shared) = self.shared_client.as_mut() else {
            return;
        };
        if !matches!(shared.state(), SharedClientState::Shared) {
            return;
        }
        let state = mbv_core::shared_store::FeedEntryState {
            position_ticks,
            played,
        };
        match shared.put_feed_entry(feed_id.to_string(), entry_guid.to_string(), state) {
            Ok(true) => {
                log::info!(
                    target: "feed_state",
                    "wrote feed entry state guid={} feed_id={} pos={}s played={}",
                    entry_guid,
                    feed_id,
                    position_ticks / mbv_core::api::TICKS_PER_SECOND,
                    played,
                );
            }
            Ok(false) => {
                log::debug!(
                    target: "feed_state",
                    "feed state write skipped (unsupported) guid={} feed_id={}",
                    entry_guid,
                    feed_id,
                );
            }
            Err(e) => {
                log::warn!(
                    target: "feed_state",
                    "feed state write failed guid={} feed_id={}: {e}",
                    entry_guid,
                    feed_id,
                );
            }
        }
    }

    /// Resolve an addressable Feed queue slot, derive its lifecycle state,
    /// update queue progress, and write `FeedEntryState` through #492
    /// without invoking Emby progress reporting. `completed` is true for
    /// known-runtime EOF or stop at/above 95% — played entries store
    /// position zero. Unknown-runtime EOF keeps `played` false.
    pub(super) fn persist_feed_slot_lifecycle(
        &mut self,
        slot_id: mbv_core::playback_queue::QueueSlotId,
        position_ticks: i64,
        completed: bool,
    ) {
        // Extract identity from the slot before any mutable borrow.
        let (feed_id, entry_guid) = {
            let queue = self.playback_queue();
            let Some(slot) = queue.queue.slot(slot_id) else {
                return;
            };
            let mbv_core::playback_queue::QueueItem::Feed(ref entry) = slot.item else {
                return;
            };
            let Some(ref feed_id) = entry.feed_id else {
                return;
            };
            (feed_id.clone(), entry.guid.clone())
        };
        let runtime = {
            let queue = self.playback_queue();
            queue
                .queue
                .slot(slot_id)
                .map(|s| s.item.runtime_ticks())
                .unwrap_or(0)
        };
        let (store_position, store_played) = if completed && runtime > 0 {
            (0, true)
        } else {
            (position_ticks, false)
        };
        let queue_mut = self.playback_queue_mut();
        let _ = queue_mut
            .queue
            .apply_progress(slot_id, store_position, store_played);
        self.write_feed_entry_state(&feed_id, &entry_guid, store_position, store_played);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_daemon_keeps_live_queue_when_shared_snapshot_arrives() {
        let live_items = crate::app::tests::make_items(2);
        let mut app = crate::app::tests::make_local_daemon_app_stub(live_items.clone());
        let shared_state = crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items: crate::app::tests::make_items(5)
                .into_iter()
                .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor: 4,
            last_played_content_id: None,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        };
        let record = SharedRecord {
            revision: 1,
            value: serde_json::to_value(shared_state).unwrap(),
        };

        app.apply_shared_record(SharedDocumentKind::QueueState, &record)
            .unwrap();

        assert_eq!(
            app.player_tab
                .emby_items()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            live_items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>()
        );
    }
}
