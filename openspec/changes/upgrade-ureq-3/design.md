## Context

ureq 2.x is used behind ~10 call sites (`AgentBuilder` + native-tls, `ureq::json!`,
`Error::Status(u16, Response)` matched for 401/403/>=500 classification). See
proposal.md for the full file list and why now.

ureq 3.x is a real rewrite, not a compatible major bump. Two changes matter beyond
renamed types:

1. **Error no longer carries the response.** In 2.x, a non-2xx response arrives as
   `Err(Error::Status(code, Response))`, and the `Response` is still readable (used
   in `audiobookshelf_contract_probe.rs::read_response` to log error bodies). In 3.x,
   the default behavior turns non-2xx into `Err(Error::StatusCode(code))` with no
   body attached — the body is only reachable if you disable "status as error" and
   get the response back as `Ok`.
2. **TLS provider is now explicit config, not a builder method call on the agent
   directly** — `AgentBuilder::tls_connector(...)` is gone; native-tls is selected
   through the new `Config`/`TlsConfig` provider setting.

## Goals / Non-Goals

**Goals:**
- Preserve current error-classification behavior (401/403 → auth failure, >=500 →
  transient, else → protocol error) exactly, including for the debug body logging in
  the contract-probe example.
- Preserve native-tls as the actual TLS backend (not silently fall back to rustls).
- Land as one mechanical PR driven by `cargo check`, not a redesign of the HTTP
  client layer.

**Non-Goals:**
- Do not introduce retries, connection pooling tuning, or timeout changes beyond
  whatever ureq 3.x defaults to unless a default change breaks a test.
- Do not touch `tungstenite` (separate crate, unaffected by this bump).

**Revised during implementation** — see below for what was actually built.

**Keep `http_status_as_error(true)` (the 3.x default); rename
`Error::Status(u16, Response)` matches to `Error::StatusCode(u16)`.**

The original decision here was to disable `http_status_as_error` and check
`response.status()` explicitly at every call site, to preserve
`audiobookshelf_contract_probe.rs`'s ability to log error response bodies.
That decision undercounted the blast radius: `http_status_as_error` is a
single agent-wide setting, not per-call. Disabling it turns every existing
`.call()?.into_json()?` chain in the codebase (dozens of them, across
`api_client_library.rs`, `api_client_playlists.rs`, `api_client_sessions.rs`,
`audiobookshelf_catalog.rs`, `audiobookshelf_playback.rs`) from "the `?`
already bails on a bad status" into "the `?` no longer fires on a bad status,
and the code proceeds to parse whatever body came back" — a correctness risk
introduced by the migration, not present before it, and far outside what a
dependency-bump PR should be taking on.

Kept the default instead: `Error::Status(u16, Response)` becomes
`Error::StatusCode(u16)` (no response attached) at every match site — a
one-line rename, same control flow, every existing `?`-chain keeps working
unchanged. Cost, found while implementing (bigger than first scoped): this
also touches `api_client_playlists.rs::create_playlist` and `rename_playlist`,
which built a `"HTTP {code}: {body}"` error string from the failed response
body — that body text is no longer available, so those error strings drop to
`"HTTP {code}"` equivalent (whatever `ureq::Error`'s `Display` produces).
`audiobookshelf_contract_probe.rs` (a dev-only example, not shipped) loses
the same thing for its error-body debug logging.

**Keep native-tls, wire it through ureq 3.x's TLS provider config.**

The proposal to swap to rustls was not raised and is out of scope — native-tls is
already a workspace dependency shared with `tungstenite`. Migration is: move the
`native_tls::TlsConnector` construction from `AgentBuilder::tls_connector(...)` to
whatever the 3.x `Config`/`TlsConfig` entry point is for a custom native-tls
connector (confirm exact method name against the installed 3.4.0 API during
implementation — the shape is a config decision, not the connector itself).

**Add missing unit coverage for `map_error` / `service_failure` before touching
their bodies.**

These functions currently have zero tests. Write status-code-in → classification-out
tests against the *current* (2.x) code first, so the PR's diff proves the
classification didn't change, rather than trusting a manual read of a match
expression that changed shape.

## Risks / Trade-offs

- [`audiobookshelf_contract_probe.rs` loses error-body logging] → it's a dev-only
  example binary (not part of the shipped `mbv`/`mbvd` binaries), and still logs
  the status code — just not the body text. Accepted as the cost of keeping the
  rest of the migration mechanical; revisit only if the example's diagnostic value
  is actually missed in practice.
- [TLS provider config API is unconfirmed at design time] → if native-tls wiring
  compiles but silently no-ops (falls back to a default provider), it wouldn't be
  caught by `cargo check`. Mitigate with a runtime smoke test (real HTTPS request
  against an Emby/ABS instance from the existing manual test flow) before merging,
  not just unit tests.

## Migration Plan

1. Bump `ureq` to 3.4.0 in `Cargo.toml` (already staged by PR #551's lockfile diff).
2. Add `map_error`/`service_failure` unit tests against current code (must pass
   before any ureq API changes).
3. Fix compile errors file by file: `Error::Status` → `Error::StatusCode` renames
   and the TLS config move.
4. Re-run the new unit tests — must still pass unchanged.
5. Manual smoke test against a real Emby and Audiobookshelf server to confirm TLS
   and auth-failure handling still work end-to-end.

No rollback complexity beyond reverting the dependency bump and the call-site diff;
no data migration or persisted state involved.
