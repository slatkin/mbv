## Why

`shared-mbv-state` is an opt-in, off-by-default capability that hosts per-Emby-user state
in the packaged daemon's `redb` database and roams it over its own authenticated TCP/Unix
protocol — roughly 3.6k lines across core, the TUI, `mbvd`, configuration, and five specs.
Its only distinct value over the existing local files is cross-machine roaming, and it buys
that by maintaining a second, revisioned authority for state (queue, library position, last
remote connection, roaming settings) that every other path already persists locally first.

The cost surfaced while investigating #687: the store's test harness created a `redb`
database per call under `/tmp` with no cleanup, and the accumulation filled the tmpfs mid-run.
Removing the capability deletes its primary leaking test site as a side effect, and settles
whether the roaming tier is worth carrying at all.

## What Changes

- **BREAKING** Remove the `shared-mbv-state` capability: `mbvd` shared-data hosting, the
  shared-data protocol (TCP/Unix + optional TLS + Emby-token identity), per-document
  compare-and-swap revisions, propagation to connected clients, and local-mirror
  fallback/reconnect with its toasts.
- **BREAKING** Remove the `[shared_data]` configuration surface (`enabled`, `listen`,
  `endpoint`, `tls_cert_path`, `tls_key_path`), the `mbvd --export-shared-data` action, the
  `mbv-shared-data-tcp-port` Emby session advertisement, and the shared-data status glyph.
- Drop the `redb` dependency — it is used nowhere else in the workspace.
- Keep feed-entry state, locally: `state_dir()/feed_entry_state.json` storing rows keyed by
  `(user_id, feed_id, entry_guid)` with last-write-wins writes and prefix scan by
  `(user_id, feed_id)`. The three user-visible behaviors are unchanged — the All / Played /
  Unplayed filter, the "Watched" row marker, and resume-on-play — but the state no longer
  roams between machines.
- The four roamed documents revert to their existing local files and `config.toml`
  (`auto_reconnect`, `[library_routes]`) as sole authority. Nothing is migrated:
  `shared.mbvd` and `roaming_settings.json` are left on disk unread, and a leftover
  `[shared_data]` section is dropped from `config.toml` on the next settings save.
- Amend the `feed-subscriptions` Purpose, which still claims the client remembers no feed
  playback state — true of the pre-store design, contradicted by every requirement added
  since, and now true again only in the machine-local sense.
- #687's leak was fixed separately (47926ae0) before this plan landed; this change still deletes
  the store and its `redb` test harness outright. The issue's remaining item — a repo-wide sweep
  of the other `temp_dir()` sites — stays open and is test hygiene, independent of this
  capability.

## Capabilities

### New Capabilities

None. Feed-entry state keeps its existing capability; only its storage changes.

### Modified Capabilities

- `shared-mbv-state`: capability removed in full (14 requirements).
- `feed-entry-state`: the daemon-service transport requirements are replaced by local-store
  persistence requirements; keying, last-write-wins, and prefix-scan semantics are retained.
- `feed-subscriptions`: roaming-state clauses drop from three requirements; the Purpose and
  the "No playback state is remembered" scenario are corrected to local-state wording.
- `packaged-daemon-service-runtime`: the clause and scenario asserting that shared-data
  enablement and identity are untouched are removed.
- `service-independent-startup`: "Optional shared state cannot gate local operation" is
  restated as local feed state that cannot be gated.

## Impact

- **Code removed**: `crates/mbv-core/src/shared_{client,client_transport,client_tests,service,protocol,state,store,worker}.rs`,
  `src/app/shared_sync.rs`, the `daemon_run.rs` hosting block, `mbvd`'s export action, the
  `shared_data_*` config fields and their validation, and the shared-data glyph.
- **Code changed**: feed-entry state moves to a local store read and written by the TUI
  (same single writer as today); the surviving feed-state functions relocate from
  `shared_sync.rs` into `src/app/feed_tab_actions.rs`.
- **Dependencies**: `redb` removed from the workspace.
- **Specs**: five capabilities as listed above; `ctrl-protocol` is untouched (its capability
  list never covered shared data, and `CTRL_CAP_SHARED_MBV_STATE` is an unused constant).
- **Docs**: `CONTEXT.md` (the six "Shared data and roaming" terms plus `FeedEntry`'s roaming
  sentence), `docs/architecture/interactive-tui-component-map.md` (three mentions).
- **Issues**: motivates a partial update to #687.
