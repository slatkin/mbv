# Design

## Context

The ctrl wire vocabulary lives in `crates/mbv-ctrl/` as serde types
(`CtrlCmd`, `CtrlEvent`, `CtrlHello`, plus payload types from `mbv-queue`,
`mbv-emby-model`, `mbv-ids`). Encoding is one JSON object per line. Both ends
decode through the same crate: the daemon at
`crates/mbv-daemon/src/core_ctrl_spawn.rs` (`serde_json::from_str::<CtrlCmd>`),
the Client in `crates/mbv-remote-player`. Packaged `mbvd` runs `mbv-daemon`,
so one corpus covers both. The repo has no `schemars`, `insta`, or schema
tooling today; CI's `cargo nextest run --release` step already runs every
workspace test, so a test-embedded gate needs no workflow changes.
`docs/recipes.md` step 8 of "Add a ctrl protocol variant" is a placeholder
waiting on this work.

## Goals / Non-Goals

**Goals:**

- `protocol/ctrl/v10/` committed to the repo: JSON Schema per wire root, a
  golden fixture per wire shape, a README with the update/bump rules.
- Conformance in both directions and full coverage (every variant either has
  a fixture or the suite fails), enforced by the existing test job.
- One deliberate regeneration path; no scripts.
- Packaged-owner decode exercised against the same corpus.

**Non-Goals:**

- No handshake-order or framing changes; the gate pins what exists (issue
  #951's stated scope). No `CTRL_PROTOCOL_VERSION` bump.
- `TransportCommand` and `PlayerCommand` are not ctrl wire lines (the former
  is resolved to wire commands client-side, the latter encodes through
  `WireCommand`) — no fixtures for them.
- No fuzzing/property testing, no cross-version compat matrix (only the
  current version gets a directory), no schema publishing beyond the repo.
- The persisted `queue_state.json` shares some types but is not the ctrl
  wire; it is out of scope (it inherits schema coverage for free).

## Decisions

### D1: Directory name is the wire version — `protocol/ctrl/v10/`

The issue's `protocol/ctrl/v1/` is an example ("e.g."), not a mandate. Naming
the directory after the `CTRL_PROTOCOL_VERSION` it pins is self-describing:
a bump to 11 opens `protocol/ctrl/v11/` and the old directory stays as the
record of v10 — no side table mapping schema revisions to wire versions.
*Alternative:* an independent schema revision (`v1`, `v2`, …) divorces the
directory name from the protocol version and invites skew.

### D2: Schema is generated with `schemars`, committed, drift-checked

`schemars` `JsonSchema` derives go on every wire-reachable type (additive
derives; no serde behavior changes). A generator emits
`schema/ctrl-cmd.schema.json`, `schema/ctrl-event.schema.json`,
`schema/ctrl-hello.schema.json` (nested types land in `$defs`), embedding
`CTRL_PROTOCOL_VERSION` in the schema `$id`/title. The conformance test
regenerates and byte-compares against the committed files — type-level drift
fails even before fixtures do.
*Alternatives:* a hand-written schema duplicates the truth and drifts
unchecked; a manifest catalog is weaker than the issue's "machine-checkable
schema" and cannot validate fixtures. Cost: `schemars` joins `mbv-ctrl`,
`mbv-queue`, `mbv-emby-model`, `mbv-ids`; `jsonschema` (validator) is a
dev-dependency of `mbv-ctrl`.

### D3: Custom-serde types get explicit schema overrides

Two spots serialize through custom impls schemars cannot see through:

- `QueueItem` — a manual `JsonSchema` impl delegates to a schema-mirror of
  the `kind`-tagged proxy (`Emby`/`Feed`/`Audiobookshelf`/
  `AudiobookshelfBook`). The schema describes the emitted shape; the
  permissive decode path (legacy bare `EmbyItem`, no `kind`) is documented in
  the README and covered by a decode-only fixture.
- `run_identity` (`#[serde(with = "run_identity_wire")]`) — the field's Rust
  type is `u64` but the wire shape is the `[0, generation]` pair. A
  `#[schemars(with = "...")]` override names the pair shape; a decode-only
  fixture pins the tolerated bare scalar.

Newtype ids (`QueueSlotId`, `ItemId`) get `JsonSchema` derives so the schema
stays honest wherever they appear.

### D4: Fixture corpus layout

```text
protocol/ctrl/v10/
  README.md
  schema/{ctrl-cmd,ctrl-event,ctrl-hello}.schema.json
  fixtures/
    commands/<Tag>[.<case>].json   # e.g. UnifiedQueueReplace.json,
                                  # PlayerCmd.SeekAbsolute.json,
                                  # SelectPlaybackOutput.Cast.json
    events/<Tag>[.<case>].json     # e.g. Player.Stopped.json,
                                  # PlaybackOutput.EmbySession.json,
                                  # ServiceSetupRejected.RevisionMismatch.json
    hello/<shape>.json             # bare CtrlHello per negotiated shape
    shared/<Type>.<case>.json      # nested-only shapes: QueueItem x4 kinds,
                                  # legacy bare EmbyItem (decode-only),
                                  # PlayerStatus, UnifiedQueueStateData
```

One fixture per variant, plus per-case fixtures where the payload enum is
itself contract (`WireCommand`, `PlayerEvent`, `OutputSelection`,
`OutputState`, `OwnerAction`, `DisconnectReason`, `ServiceSetupRejection`,
`PlaybackIntentOutcome`/`Rejection`, `QueueItem` kinds, `QueueOpOutcome`).
Handshake fixtures cover the Local control-credential hello, the
credential-free packaged hello (#523's boundary), service-setup-admin,
pin-swap (pinned + swap token), owner-action, and an audio-only daemon hello.
Files are pretty-printed JSON so diffs stay reviewable; roughly a hundred
files, all generated.

### D5: Conformance harness lives in `crates/mbv-ctrl/tests/ctrl/conformance.rs`

A module of the existing `tests/ctrl.rs` binary (the crate's one integration
binary, per `M-INTEGRATION-TESTS`), driven by a `FIXTURES` table: each row
names a path, a direction class (round-trip or decode-only), and a builder
for the canonical value.

- **Round-trip rows:** encode the value, compare `serde_json::Value` against
  the file (semantic equality — field order is not wire contract); decode the
  file, re-encode, compare again. This checks both directions without needing
  `PartialEq` on `CtrlCmd`/`CtrlEvent`.
- **Decode-only rows** (legacy bare `EmbyItem`, bare-scalar `run_identity`):
  decoding must yield exactly the canonical value the same shape encodes to
  (the legacy bytes themselves are not re-emitted or rewritten, but a
  silently dropped or altered field fails).
- **Coverage, compile-forced:** the test owns an exhaustive `match` per wire
  enum mapping every variant to its shape name — a new variant breaks the
  build until the arm (and the fixture) exists. A runtime reconciliation
  asserts table rows and on-disk files match exactly, catching orphaned or
  missing fixtures.
- **Schema freshness:** regenerate all three schemas and byte-compare.
- **Fixture-vs-schema validation:** every fixture validates against its root
  schema via the `jsonschema` dev-dependency, so a wrong `#[schemars(with)]`
  override cannot pass silently.
- **Bless mode:** `MBV_PROTOCOL_BLESS=1` makes the harness rewrite fixtures
  and schemas instead of failing — the single deliberate-update path (same
  shape as insta's update mode, no new tooling or scripts).

### D6: Owner-side decode check in `crates/mbv-daemon`

One test iterates `fixtures/commands/*.json` and decodes each through
`serde_json::from_str::<CtrlCmd>` — the entry `core_ctrl_spawn.rs` uses —
asserting every file decodes and the corpus is non-empty. `mbvd` runs
`mbv-daemon`, so the packaged-owner direction is exercised without launching
a daemon.

### D7: Recipe and comment updates, not duplicated pins

`docs/recipes.md` step 8 names the bless command and the fixture site;
`protocol.rs`'s bump-rules comment points at `protocol/ctrl/v10/`. The
existing `wire.rs` pin tests stay: they pin at unit level, cost nothing, and
a wire change forcing updates in two places is the intended friction. Pure
tag-pin assertions that a fixture now duplicates may be trimmed
opportunistically during implementation, but removing them is not a goal.

## Risks / Trade-offs

- **A wire-reachable type misses `JsonSchema`** → the derive is required by
  compilation wherever a schema root reaches the type, so coverage is
  compile-forced, not convention.
- **`#[schemars(with)]` mirrors drift from the real serde shape** →
  fixture-vs-schema validation plus round-trip fixtures catch a mirror that
  no longer matches what serde emits.
- **Bless mode rubber-stamps unintended drift** → same exposure as snapshot
  testing everywhere: regenerated JSON diffs are small and reviewable, and
  the spec makes regeneration a deliberate, reviewed act.
- **~100 generated files add diff noise** → grouped directories, one shape
  per file, README explains the layout; changes to a wire shape show as a
  one-file fixture diff plus schema diff.
- **schemars major-version churn regenerates all schemas** → pin the major
  version; a format bump lands as one explicit regen commit, not a side
  effect.
