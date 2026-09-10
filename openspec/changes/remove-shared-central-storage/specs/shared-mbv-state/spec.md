## REMOVED Requirements

### Requirement: Shared-data hosting and use are explicit opt-ins

**Reason**: The shared-data facility is removed. There is no daemon hosting, no endpoint to configure, and no opt-in to express; mbv-owned state is single-machine state with no roaming tier above it.

**Migration**: No configuration is required. The four state documents continue to be read and written from their existing local files under `state_dir()`, which is the behavior a client without a shared-data endpoint already had.

### Requirement: Shared documents are isolated per Emby user

**Reason**: There are no shared documents and no authenticated shared-data session to scope them to. The four remaining state files are machine-local, not per-user.

**Migration**: Feed entry state retains its per-user key locally so switching Emby users does not mix played state. Queue state, library position state, last remote connection, and `auto_reconnect` / `[library_routes]` are machine-local and unchanged.

### Requirement: Shared identity is verified and fail-closed

**Reason**: Shared-data access was the only consumer of this check. Removing the transport removes the identity requirement with it; there is no connection left to authorize or reject.

**Migration**: None. Emby credentials continue to be used only for Emby API and playback behavior.

### Requirement: Shared-data transport is local or private-network only

**Reason**: The transport is removed along with the listeners and endpoint validation that enforced this rule.

**Migration**: None.

### Requirement: Remote shared-data transport is encrypted

**Reason**: Removed with the remote transport it protected.

**Migration**: None.

### Requirement: First writer initializes an absent document

**Reason**: A single-machine client cannot race another client for an absent document, so conditional creation has no meaning.

**Migration**: The ordinary local-state behavior stands — a missing file means the state is absent, and the next write creates it.

### Requirement: Shared roaming settings override local configuration

**Reason**: There is no roaming tier to override from. Silent shared-over-local precedence is withdrawn.

**Migration**: `auto_reconnect` and `[library_routes]` are read from `config.toml` only, and the F2 library-route picker keeps writing them there. `roaming_settings.json` is left unread on disk; a value that existed only in that mirror is not adopted.

### Requirement: Documents use independent optimistic revisions

**Reason**: Revisions existed to reject stale cross-client writes. With one writer and one machine there is no concurrent writer to reject.

**Migration**: Local persistence continues to use atomic temporary-file replacement, which preserves the previous file when a write or rename fails.

### Requirement: Committed updates propagate to connected clients

**Reason**: There are no shared-data connections and therefore nothing to notify.

**Migration**: None.

### Requirement: Connected state is mirrored locally

**Reason**: Inverted by this change — the local file is the primary store rather than a mirror of a remote document.

**Migration**: None; the local files already carry the complete document schema.

### Requirement: Shared failure falls back locally and retries

**Reason**: There is no remote dependency left to fail, fall back from, or retry, and no fallback toast to show.

**Migration**: Local read and write failures are logged and non-fatal, matching the existing local-state behavior. Browsing and playback never depend on state persistence succeeding.

### Requirement: Shared state regains authority after reconnection

**Reason**: There is no reconnection path and no second copy of the state that could diverge.

**Migration**: None.

### Requirement: Storage failure is isolated from playback

**Reason**: The daemon no longer stores user state, so there is no daemon-side commit to isolate.

**Migration**: The local-store equivalent of this guarantee is stated in `feed-entry-state`; a failed local write never blocks playback and never damages the previously written state.

### Requirement: Shared documents are exportable as JSON

**Reason**: There is no database to export from and no `mbvd --export-shared-data` action. Nothing in the remaining state is opaque.

**Migration**: Every remaining state document is a plain JSON file under `state_dir()` (`queue_state.json`, `library_position_state.json`, `last_remote_connection.json`, `feed_entry_state.json`) and can be read directly. A leftover `shared.mbvd` is unread by any current code path.
