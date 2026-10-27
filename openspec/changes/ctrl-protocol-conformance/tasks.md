# Tasks

## 1. Schema derivations (schemars reaches every wire type)

- [x] 1.1 Add `schemars` (with its derive feature) to the workspace and to `mbv-ids`, `mbv-emby-model`, `mbv-queue`, `mbv-ctrl`; add `jsonschema` as a `dev-dependency` of `mbv-ctrl`. Verify: `cargo check -p mbv-ctrl` succeeds.
- [x] 1.2 Derive `JsonSchema` on the wire-reachable types in `mbv-ids` (`ItemId`) and `mbv-emby-model` (`EmbyItem`, `EmbyPerson`, `EmbyLink`, `EmbyArtistRef`, `EmbyImageTags`). Verify: `cargo check -p mbv-ids` and `cargo check -p mbv-emby-model`.
- [x] 1.3 Derive `JsonSchema` on the wire-reachable `mbv-queue` types (`FeedEntry`, `AudiobookshelfQueueItem`, `AudiobookshelfBookQueueItem`, `AudiobookshelfItem`, `QueueItemContentId`, `QueueSource`, `QueueLineage`, `ServiceKind`, `FeedKind`, `QueueSlotId`) and hand-write `impl JsonSchema for QueueItem` delegating to a schema-mirror of the `kind`-tagged proxy (design D3). Verify: `cargo check -p mbv-queue` plus a unit test asserting the emitted `QueueItem` schema offers the four `kind` tags.
- [x] 1.4 Derive `JsonSchema` on every wire-reachable `mbv-ctrl` type (`CtrlCmd`, `CtrlEvent`, `CtrlHello`, `WireCommand`, `PlaybackIntent*`, `OwnerAction`, `Output*`, `SessionNowPlaying`, `DisconnectReason`, `QueueOp*`, `ServiceSetupRejection`, `PipePlayback*`, `Audiobookshelf*ProgressEvent`, `UnifiedQueue*`, `TransitionSummary`, `ProgressUpdate`, `PlayerEvent`, `PlayerStatus`) and override `run_identity` with `#[schemars(with = "...")]` for the emitted `[request_id, generation]` pair (design D3). Verify: `cargo check -p mbv-ctrl` and `cargo clippy -p mbv-ctrl --all-targets -- -D warnings`.

## 2. Protocol directory and conformance harness

- [x] 2.1 Create `protocol/ctrl/v10/README.md` documenting the layout, the additive-in-place / bump-opens-`v11` rules, the user-approval requirement for bumps, and the bless command (design D1, D4). Verify: file exists and its rules match `protocol.rs`'s comment.
- [x] 2.2 Add `tests/ctrl/conformance.rs` to the existing `tests/ctrl.rs` binary: the `FIXTURES` table (path, round-trip vs decode-only, canonical-value builder), the exhaustive per-enum shape-name matches, fixture-dir↔table reconciliation, schema regeneration + byte-compare + `CTRL_PROTOCOL_VERSION` binding, per-fixture `jsonschema` validation, and `MBV_PROTOCOL_BLESS=1` rewrite mode (design D5). Seed it with the `hello/` shapes (base, control-credential, credential-free packaged, service-setup-admin, pin-swap, pinned-surface, owner-action, audio-only daemon). Verify: after a bless run, `cargo nextest run -p mbv-ctrl conformance` is green and a hand-edited fixture makes it fail.
- [ ] 2.3 Fill the `commands/` corpus: every `CtrlCmd` variant, all twelve `WireCommand` tags as `PlayerCmd.<tag>` rows, all five `PlaybackIntentAction`s, all three `OutputSelection`s, both `OwnerAction`s, and `ApplyServiceSetup` for both `ServiceKind`s. Verify: bless regenerates, suite green, and a seeded wire change (e.g. rename a field locally) fails conformance.
- [ ] 2.4 Fill the `events/` corpus: every `CtrlEvent` variant, every `PlayerEvent` variant as `Player.<variant>` rows, all three `OutputState`s, all four `DisconnectReason`s, all four `ServiceSetupRejection`s, the `PlaybackIntentOutcome`/`PlaybackIntentRejection` cases, and `QueueOpOutcome` applied/rejected. Verify: suite green; removing a fixture file fails coverage reconciliation.
- [ ] 2.5 Fill `shared/` plus decode-only rows: `QueueItem` for all four kinds, a legacy bare-`EmbyItem` decode-only fixture, a bare-scalar `run_identity` decode-only fixture, `PlayerStatus`, and a `UnifiedQueueStateData` carrying in-flight and queued transitions. Verify: suite green and the decode-only fixtures deserialize.

## 3. Owner decode and docs

- [ ] 3.1 Add a `crates/mbv-daemon` test that decodes every `fixtures/commands/*.json` through `serde_json::from_str::<CtrlCmd>` (the same call `core_ctrl_spawn.rs` uses) and asserts the corpus is non-empty (design D6). Verify: `cargo nextest run -p mbv-daemon` passes.
- [ ] 3.2 Update `docs/recipes.md` "Add a ctrl protocol variant" step 8 to name the bless command and the fixture directory, and point the `protocol.rs` bump-rules comment at `protocol/ctrl/v10/` (design D7). Verify: both files name `protocol/ctrl/` and `MBV_PROTOCOL_BLESS`.

## 4. Final verification

- [ ] 4.1 Run the full gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-ctrl -p mbv-daemon`, and `openspec validate ctrl-protocol-conformance --strict`. Verify: all green.
