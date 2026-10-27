# Proposal

## Why

The Client-to-Player-owner vocabulary exists only as Rust types in
`crates/mbv-ctrl/`. Wire tags are pinned by unit tests
(`crates/mbv-ctrl/tests/ctrl/wire.rs`), but there is no versioned wire schema
and no fixture corpus, and nothing in CI detects wire drift. Skew between a
Client and a separately packaged `mbvd` therefore surfaces only at runtime and
quietly: unknown commands are skipped, unknown events are logged and ignored,
and undeserializable ctrl lines are dropped without a log. The v9→v10
`EmbyItem` drift (a v9 peer silently dropping every `UnifiedQueue` command
from a v10 Client) is the documented example (issue #951).

## What Changes

- Add a versioned public protocol directory `protocol/ctrl/v10/` holding a
  machine-checkable JSON Schema for each wire root (`CtrlCmd`, `CtrlEvent`,
  `CtrlHello`) plus a golden fixture corpus covering every command, event,
  handshake shape, queue slot/snapshot shape, and rejection shape.
- Add a conformance test harness in `crates/mbv-ctrl` that checks both
  directions against the corpus: encode produces the fixture bytes and decode
  round-trips them; schema regeneration must match the committed schema
  byte-for-byte; fixture files and code-level wire variants must cover each
  other exactly. A wire change fails CI until fixtures and schema are
  deliberately regenerated.
- Add a thin owner-side decode check in `crates/mbv-daemon` so the packaged
  `mbvd` path (which runs `mbv-daemon`) is exercised against the same corpus.
- Update `docs/recipes.md` (the "Add a ctrl protocol variant" recipe's
  placeholder step 8) and the `protocol.rs` bump-rules comment to name the
  directory and the deliberate-regeneration workflow.
- No protocol-version bump, no handshake or framing change: the gate pins the
  wire as it exists at version 10. Additive changes keep landing in
  `protocol/ctrl/v10/`; a bump opens `protocol/ctrl/v11/` and still requires
  explicit user approval.

## Capabilities

### New Capabilities

- `ctrl-protocol-conformance`: the versioned public protocol directory and the
  CI-enforced conformance gate — schema freshness, fixture corpus coverage,
  encode/decode round-trip in both directions, and the deliberate-update rule
  that keeps fixture changes a reviewed act rather than silent drift.

### Modified Capabilities

None. `ctrl-protocol` runtime behavior is unchanged (still version 10, same
handshake and framing).

## Impact

- `protocol/ctrl/v10/` (new): `README.md`, `schema/*.schema.json`,
  `fixtures/{commands,events,hello,shared}/*.json`.
- `crates/mbv-ctrl`: `schemars` dependency and `JsonSchema` derives (or
  explicit `#[schemars(with)]` overrides) on every wire-reachable type; new
  conformance test module under `tests/ctrl/`; `protocol.rs` comment updated.
- `crates/mbv-queue`, `crates/mbv-emby-model`, `crates/mbv-ids`: `schemars`
  derives on the wire-reachable types those crates own (`QueueItem` needs a
  manual schema via a mirror type because its serde impl is custom).
- `crates/mbv-daemon`: one corpus-decode test.
- Dev-tooling: `jsonschema` crate as a dev-dependency for fixture validation;
  a bless mode (`MBV_PROTOCOL_BLESS=1 cargo nextest run -p mbv-ctrl ...`) for
  deliberate fixture/schema regeneration. No new scripts; CI needs no new
  step — the gate rides the existing `cargo nextest run --release` job.
- `docs/recipes.md`: recipe step 8 filled in with the real regeneration path.
- Fixes #951.
