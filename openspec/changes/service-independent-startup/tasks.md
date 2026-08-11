## 1. Configuration And Credential Foundation

- [ ] 1.1 Separate general application configuration from Emby runtime ownership and make every non-Emby config section parse and save correctly when `[server]` is absent.
- [ ] 1.2 Add singleton Service setup records, per-Service mode-`0600` secret paths, and a per-Local-daemon Control credential path using existing atomic persistence utilities.
- [ ] 1.3 Implement one-time legacy Emby server/token/user migration with write-new-before-remove-old ordering and actionable failure reporting.
- [ ] 1.4 Extend the nearest config/auth persistence tests to protect legacy migration, failed-write retention, secret permissions, and config parsing without Emby.

## 2. Service-Independent Runtime

- [ ] 2.1 Introduce explicit runtime Service state and setup-generation tracking while keeping Emby as a concrete optional runtime.
- [ ] 2.2 Move general configuration and feed management out of mandatory `EmbyClient` ownership throughout App construction and state transitions.
- [ ] 2.3 Remove the global Emby authentication gate, start the TUI and selected Player-owner role first, and initialize configured Emby in a bounded background worker.
- [ ] 2.4 Route empty first-launch state to Services settings, preserve ordinary navigation for configured content, and keep feed-only/local-fallback operation functional through every Emby state.

## 3. Services Settings And Emby Migration

- [ ] 3.1 Add the Services Settings destination with singleton Emby, Audiobookshelf, and always-present Feeds entries and their applicable runtime states/actions.
- [ ] 3.2 Recast the Emby login form as transactional Service setup/repair that retains username/password only through token generation and commits only validated setup.
- [ ] 3.3 Implement Emby connectivity/authentication state classification so connectivity preserves the secret while rejection clears only the secret and exposes Needs authentication.
- [ ] 3.4 Implement confirmed Emby replacement/removal and clear all Emby-owned queue items, positions, routes, caches, setup, and credentials without affecting Feeds.

## 4. Local Daemon Control Trust

- [ ] 4.1 Generate and load the stable per-owner Control credential independently of every Service credential.
- [ ] 4.2 Add capability-gated ctrl hello support for the Control credential while preserving the existing Emby field semantics for deferred legacy peers.
- [ ] 4.3 Make bare and Local daemon Player-owner construction, attachment, and feed playback work with no Emby runtime; return a targeted compatibility error for feed-only attachment to legacy peers.
- [ ] 4.4 Extend existing real-handshake/process-boundary tests to prove valid and invalid Control authentication plus new-client fallback to an Emby-authenticated legacy peer.

## 5. Reconciliation And Verification

- [ ] 5.1 Reconcile startup documentation and the completed `graceful-failure-emby-unavailable` change so no current artifact still specifies exiting when Emby is unavailable.
- [ ] 5.2 Manually verify fresh empty startup, migrated Emby startup, invalid Emby credentials, unreachable Emby, bare feed-only playback, stay-alive feed-only playback, Emby setup/repair, and confirmed replacement/removal.
- [ ] 5.3 Run `cargo check -p mbv-core`, relevant focused tests, `cargo clippy --workspace --all-targets`, and `make check-code-file-lines`; resolve all introduced failures.
