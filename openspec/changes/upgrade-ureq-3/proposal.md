## Why

Dependabot PR #551 bumps `ureq` from 2.12.1 to 3.4.0, a major version with breaking
API changes upstream. We don't want to keep deferring web-library upgrades — the web
is unreliable, and the crate releases majors for real reasons (TLS fixes, pooling
bugs, redirect bugs). This change migrates our usage to the ureq 3.x API and confirms
nothing broke in the process.

## What Changes

- **BREAKING (internal only, no user-visible change)**: `AgentBuilder::new()` →
  ureq 3.x's `Agent` config/builder API, at every call site (`feed_parse.rs`,
  `audiobookshelf.rs`, `api_client_auth.rs`, `images.rs`).
- **BREAKING**: `ureq::json!` macro (removed upstream) → `serde_json::json!` at all
  call sites (`api_client_reporting.rs`, `api_client_sessions.rs`,
  `api_client_playlists.rs`, `api_client_auth.rs`).
- **BREAKING**: `ureq::Error::Status(u16, Response)` shape change → update the
  status/error match arms in `api_client_auth.rs::service_failure` and
  `audiobookshelf_catalog.rs::map_error`, and in `audiobookshelf_playback.rs` /
  `audiobookshelf_contract_probe.rs`.
- **BREAKING**: native-tls wiring point (`AgentBuilder::tls_connector`) moves to
  whatever ureq 3.x's TLS config entry point is; must confirm native-tls is still
  actually in effect after the migration, not silently defaulting to something else.
- Add unit test coverage for `map_error` / `service_failure` status-code branching
  (401/403 → auth failure, >=500 → transient, other → protocol error), which has no
  test coverage today. This is the one spot a clean compile could hide a behavior
  regression.
- Bump `ureq = "2"` → `ureq = "3"` in the workspace `Cargo.toml`.

## Capabilities

No spec-level behavior changes — this preserves existing request/response and
error-classification behavior through a dependency major-version bump. See
`skip_specs: true` in `.openspec.yaml`.

## Impact

- Files: `Cargo.toml`, `src/app/images.rs`, `src/app/feed_parse.rs`,
  `crates/mbv-core/src/api_client_auth.rs`,
  `crates/mbv-core/src/api_client_playlists.rs`,
  `crates/mbv-core/src/api_client_sessions.rs`,
  `crates/mbv-core/src/api_client_reporting.rs`,
  `crates/mbv-core/src/api_types.rs`,
  `crates/mbv-core/src/audiobookshelf.rs`,
  `crates/mbv-core/src/audiobookshelf_catalog.rs`,
  `crates/mbv-core/src/audiobookshelf_playback.rs`,
  `crates/mbv-core/examples/audiobookshelf_contract_probe.rs`.
- Dependency: `ureq` 2.12.1 → 3.4.0 (already staged in dependabot PR #551's
  `Cargo.toml`/`Cargo.lock` diff).
- No API/protocol/UI surface changes; this is an internal HTTP-client migration.
