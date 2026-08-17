## 1. Baseline safety net

- [x] 1.1 Add unit tests for `audiobookshelf_catalog.rs::map_error` covering
      401, 403, 500, 503, and one non-error status code, asserting the
      resulting `AudiobookshelfError` variant for each.
      (Deviation: `api_failure_tests.rs::audiobookshelf_me_http_boundary_uses_bearer_and_redacts_failures`
      already covers 401/500/404/200-malformed end-to-end through a real
      TCP listener — this is the pre-existing safety net; no new test added.)
- [x] 1.2 Add unit tests for `api_client_auth.rs::service_failure` covering
      401, 403, and a non-auth status code, asserting the resulting
      `EmbyFailure` variant for each.
      (Deviation: `api_failure_tests.rs::persisted_token_http_401_and_403_are_authentication_rejections`
      and `..._5xx_transport_and_malformed_responses_are_unavailable` already
      cover this; no new test added.)
- [x] 1.3 Run these new tests against current (2.x) code and confirm they pass
      before touching the dependency.
      (Ran the 3 existing tests above via
      `rtk cargo nextest run -p mbv-core -E 'test(audiobookshelf_me_http_boundary) or test(persisted_token_http)'` — 3 passed.)

## 2. Bump the dependency

- [x] 2.1 Bump `ureq` to `"3"` in `Cargo.toml`. Also re-added the `json`
      feature (needed for `send_json`/`read_json`, dropped by mistake
      initially) and ran `cargo update -p ureq --precise 3.4.0`.
- [x] 2.2 Ran `rtk cargo check --workspace --all-targets` to enumerate every
      compile error (46 initially, across `mbv-core`, `mbv`, and the
      `audiobookshelf_contract_probe` example).

## 3. Migrate agent construction and TLS wiring

- [x] 3.1 `src/app/feed_parse.rs`: `AgentBuilder`+`tls_connector` →
      `Agent::config_builder().tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls))`.
      Made `tls_agent()` `pub(super)` and infallible (native-tls connector
      construction moved inside ureq itself, lazy, no longer fallible at
      agent-build time).
- [x] 3.2 Same in `crates/mbv-core/src/audiobookshelf.rs`.
- [x] 3.3 Same in `crates/mbv-core/src/api_client_auth.rs` — factored into a
      shared `emby_agent(connect_timeout, total_timeout)` fn since both the
      constructor and `with_request_timeout` needed it.
- [x] 3.4 Same in `src/app/images.rs`, at both agent-construction sites
      (the `ureq::get(&url)` free-function call and the `fetch_url` closure's
      own `AgentBuilder`).
- [x] 3.5 Confirmed via ureq 3.4.0 source
      (`tls/mod.rs`: "The setting is never picked up automatically") that
      `TlsProvider::NativeTls` must be set explicitly per-agent — unlike 2.x,
      where it was implicit. Two sites (`audiobookshelf.rs`,
      `api_client_auth.rs`) had never set it explicitly and were relying on
      2.x's implicit behavior; now fixed everywhere.

## 4. Migrate error matching (kept `http_status_as_error` at its 3.x default
      of `true` — see design.md "Decisions" for why the original
      disable-it plan was reverted, and for the two shipped-code call sites
      that lose error-body detail as a result)

- [x] 4.1 Renamed `ureq::Error::Status(code, _)` → `ureq::Error::StatusCode(code)`
      in `api_client_auth.rs::service_failure` and its other match site.
- [x] 4.2 Same rename in `audiobookshelf_catalog.rs::map_error`.
- [x] 4.3 Same in `audiobookshelf_playback.rs::wait_for_hls_ready` — folded
      `Error::Status`/`Error::Transport` into a single `Err(_)` catch-all arm.
- [x] 4.4 Updated `audiobookshelf_contract_probe.rs`: renamed the
      `Error::StatusCode` matches; `post`'s error branch now returns a status
      code with a placeholder body string (see design.md risk note) instead
      of the real error body.

## 5. Migrate remaining API surface

- [x] 5.1 Replaced `ureq::json!` with `serde_json::json!` in
      `api_client_reporting.rs`, `api_client_sessions.rs`,
      `api_client_playlists.rs`, `api_client_auth.rs`.
- [x] 5.2 Replaced the bare `ureq::get(&url)` call in `src/app/images.rs`
      with `super::feed_parse::tls_agent().get(&url)`.
- [x] 5.3 Fixed remaining surface: `.set()` → `.header()` everywhere;
      `.into_json()`/`.into_string()`/`.into_reader()` → `.body_mut().read_json()`
      / `.body_mut().read_to_string()` / `.into_body().into_reader()`;
      `ureq::Response`/`ureq::Request` → `ureq::http::Response<ureq::Body>` /
      `ureq::RequestBuilder<ureq::typestate::WithBody|WithoutBody>`; bodyless
      `.call()` on a POST builder → `.send_empty()`; `.send_string(s)` →
      `.send(s)`. Also found and fixed two real (not just example) error-body
      losses in `api_client_playlists.rs::create_playlist`/`rename_playlist`
      (see design.md).

## 6. Defensive URL-encoding (added during implementation — see below)

Testing surfaced a real, confirmed behavior change beyond the mechanical
migration: ureq 2.x silently percent-encoded invalid URL characters when
building a request; ureq 3.x builds requests through the strict `http::Uri`
parser and rejects them outright (surfaces as a generic connectivity error,
not a distinguishable one). Real Emby/Audiobookshelf IDs are opaque
server-generated tokens so this was low real-world risk, but the user chose
to harden it rather than just patch the tests that exposed it.

- [x] 6.1 Added `percent-encoding` as a direct `mbv-core` dependency (already
      present transitively) and a `crate::encode_path_segment` helper in
      `lib.rs` (percent-encodes everything outside RFC 3986 unreserved:
      `ALPHA / DIGIT / "-" / "." / "_" / "~"`).
- [x] 6.2 Wrapped every dynamic URL *path segment* (not query values, which
      ureq already encodes) built via `format!()` across
      `api_client_library.rs`, `api_client_playlists.rs`,
      `api_client_auth.rs`, `api_client_sessions.rs`,
      `audiobookshelf_playback.rs`, `audiobookshelf_catalog.rs` with
      `crate::encode_path_segment(...)`.
- [x] 6.3 Fixed two more real behavior changes the encoding fix's tests
      surfaced: ureq 3.x lowercases header names on the wire (RFC
      7230-legal, but broke exact-case test assertions in `api_tests.rs`,
      `api_failure_tests.rs`, `audiobookshelf_playback_tests.rs`), and
      `send_json` now pretty-prints instead of compact-prints the JSON body
      (broke exact-substring assertions in `audiobookshelf_playback_tests.rs`,
      `player_sources.rs`). Both are cosmetic wire-format changes with no
      functional impact; tests updated to be case/whitespace-tolerant rather
      than asserting exact formatting.

## 7. Verify

- [x] 7.1 `rtk cargo check --workspace --all-targets` clean (mbv, mbv-core,
      mbvd, and the contract-probe example).
- [x] 7.2 `rtk cargo clippy --workspace --all-targets` clean (3 remaining
      warnings are pre-existing and unrelated to this change).
- [x] 7.3 `rtk cargo nextest run --workspace` — 1364 passed, 1 skipped.
- [ ] 7.4 Manual smoke test: connect to a real Emby server and a real
      Audiobookshelf server, exercise one auth-failure path (bad token) and
      one normal request, confirm behavior matches pre-upgrade.
- [x] 7.5 `rtk make check-code-file-lines` clean.
