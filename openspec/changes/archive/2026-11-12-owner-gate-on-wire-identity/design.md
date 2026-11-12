# Design

## Context

See `proposal.md` (Why) and the two delta specs for behavior. This section gives only the current state that shapes the approach.

- **The gate.** `read_server_hello` (`crates/mbv-remote-player/src/connect/handshake.rs`) refuses under `PeerBuild::MustMatch` when `app_version` or `build_id` differs from this binary. It runs before `send_client_hello`, which invariant 15 requires. `PeerBuild::MustMatch` applies to the local Client attach and to the two restricted roles (service-setup admin and owner action). Explicit endpoints and packaged mbvd use `PeerBuild::Any`.
- **The build identity.** `crates/mbv-ctrl/build.rs` embeds `BUILD_ID` (git short SHA, `-dirty`, or `unknown`). `CtrlHello::current()` puts it in every hello. This change keeps it, and uses it for display only.
- **The wire schemas.** `CtrlCmd`, `CtrlEvent`, and `CtrlHello` derive `schemars::JsonSchema`, and `schemars` (1.2.3) is already a normal dependency of `mbv-ctrl`. The conformance test byte-compares `schema_for!` output with `protocol/ctrl/v10/schema/*.json`. The schemas contain about 170 `description` annotations from doc comments. They also contain properties that are named `title` and `description` (for example `AudiobookshelfQueueItem`, `FeedEntry`, `PlayerStatus`).
- **The refusal error.** `RemotePlayerErrorKind::OwnerBuildMismatch(Box<OwnerBuildMismatchData>)` carries the Owner's version and build. Its `Display` is the one text that the TUI prompt (`src/owner_restart.rs`) and the GUI mismatch state (`crates/mbv-gui/src/attach.rs`) both show.
- **The handshake result is discarded.** `read_server_hello` returns only `CtrlCompatibility`. Nothing keeps the Owner's version or build after the handshake.
- **Settings rows.** `SETTING_SECTIONS` (`crates/mbv-ui-model/src/settings.rs`, 722 lines) lists the rows, and `setting_value(key, cfg, ui)` gives values from configuration only. `mbv-ui-model` does not depend on `mbv-ctrl`. The TUI shell builds rows in `Model::main_settings_rows` (`src/app/shell/settings.rs`). `setting_row_kind` (`src/app/dispatch/settings.rs`) classifies every key with an exhaustive match. `handle_settings_activate` schedules a save for every row that is not a destination, also a `Passive` one.
- **The GUI Settings screen** is planned in `add-gui-settings-screen`. Its F2 rows come from `SETTING_SECTIONS`, filtered by `SettingKey::scope`, and its frame carries each row's key, kind, and value (that change's design D3, D4, D9). This change's GUI part needs that change to be merged first.
- **File sizes.** `crates/mbv-remote-player/src/connect/tests.rs` has 782 lines, `crates/mbv-remote-player/src/lib.rs` 748, and `crates/mbv-remote-player/src/connect.rs` 722.

## Goals / Non-Goals

**Goals:**

- A rebuilt Client attaches to a running Owner of the same version when no ctrl message shape changed.
- A wire change or a version change still refuses before the credential is sent.
- Both Clients show the version information of both sides in Settings, read-only.

**Non-Goals:**

- No change to `CTRL_PROTOCOL_VERSION`, capability negotiation, or `validate_peer`.
- No change to explicit-endpoint or mbvd behavior beyond the new Settings row.
- No detection of behavior-only Owner changes. The Settings row is the only signal (decision with the user).
- No change to `build.rs` or to how `BUILD_ID` is computed.

## Decisions

### D1. The wire identity is an FNV-1a digest of the normalized runtime schemas

`crates/mbv-ctrl/src/wire_identity.rs` gives `pub fn wire_id() -> &'static str`. The value is computed one time through `std::sync::LazyLock`:

1. Generate `schemars::schema_for!` for `CtrlCmd`, `CtrlEvent`, and `CtrlHello`, in this order.
2. Convert each to `serde_json::Value` and normalize it: remove the annotation keywords `description`, `title`, `examples`, `$comment`, `$id`, and `$schema` from every schema object. Do not treat the keys of `properties`, `patternProperties`, `$defs`, or `definitions` maps as keywords: those keys are property and definition names, and a property named `title` is wire shape.
3. Serialize each normalized value with `serde_json::to_vec`. `serde_json` is built without `preserve_order`, so object keys are sorted and the bytes are deterministic.
4. Feed the three byte strings, each followed by a `0x00` separator, into 64-bit FNV-1a. Format the result as 16 lowercase hex digits.

FNV-1a is about ten lines, has a fixed specification, and does not depend on the toolchain. `std::hash::DefaultHasher` is rejected, because its algorithm is not stable across Rust releases, and an Owner and a Client can come from different toolchains. A cryptographic hash crate (for example `sha2`) is rejected, because collision resistance has no value here and it would add a dependency to `mbv-ctrl`. A digest of the committed `protocol/ctrl/v10/schema/*.json` files in `build.rs` is rejected: it adds path coupling between the crate and the repository layout, and it trusts the committed copy instead of the types. The conformance test keeps the committed copy equal to the types, so the two would give the same value.

`CTRL_PROTOCOL_VERSION` is not in the digest. The protocol version has its own check, and the delta spec keeps "only the protocol version differs" as a non-prompt refusal.

### D2. `BuildInfo` holds the three values, and the hello carries `wire_id`

`mbv-ctrl` gets `pub struct BuildInfo { pub app_version: String, pub build_id: String, pub wire_id: String }` with `BuildInfo::current()` (from `CARGO_PKG_VERSION`, `BUILD_ID`, and `wire_id()`) and `BuildInfo::differs_for_gate(&self, other: &BuildInfo) -> bool`. That method is true when `app_version` differs or `wire_id` differs, and it compares nothing else. An empty `wire_id` never equals the non-empty local value, so an Owner from before this change is refused.

`CtrlHello` gets `#[serde(default)] pub wire_id: String`, set in `current()`, and `CtrlHello::build_info(&self) -> BuildInfo`. The field is additive, as `build_id` was. No capability is added, because the field value itself is the signal, and the gate reads it before any capability. The committed `ctrl-hello.schema.json` and the hello fixtures are regenerated in the same commit, as the `ctrl-protocol-conformance` spec requires.

### D3. The gate compares through `BuildInfo`, and the error carries both sides

`read_server_hello` returns `ServerHello { compatibility: CtrlCompatibility, owner_build: BuildInfo }`. Under `PeerBuild::MustMatch` it refuses when `BuildInfo::current().differs_for_gate(&owner_build)`, still before `validate_peer` and `send_client_hello` (invariant 15). `perform_handshake_with_role` and the bounded `HandshakeOutcome` carry `owner_build` through, and `connect_stream` stores it in a new `RemotePlayer::owner_build: BuildInfo` field. `RemotePlayer::owner_build(&self) -> &BuildInfo` reads it. `RemotePlayer::set_owner_build_for_test` follows the existing `set_ctrl_compatibility_for_test` seam, for the App tests.

`RemotePlayerErrorKind::OwnerBuildMismatch` becomes `OwnerMismatch(Box<BuildInfo>)`, and it holds the Owner's values. `is_owner_build_mismatch` becomes `is_owner_mismatch`, `owner_build_mismatch_for_test` becomes `owner_mismatch_for_test(BuildInfo)`, and the error code becomes `remote-player.owner_mismatch`. The `Display` text lists version, build, and wire identity for the Owner and for this Client. An empty value shows as unknown. The TUI prompt and the GUI mismatch state already show this `Display`, so they need no other change. The rename is in scope because the old name would say "build" for a refusal that no longer depends on the build.

### D4. Two read-only `Info` rows in the Session section

`mbv-ui-model` gets `SettingKey::ClientVersion` and `SettingKey::OwnerVersion`, put after `ShowSysTrayIcon` in the Session section of `SETTING_SECTIONS`. A new `SettingValueKind::Info` is their kind. Their labels are "This client" and "Owner". `setting_value` returns an empty string for `Info` keys, because the values do not come from configuration. `mbv-ui-model` stays independent of `mbv-ctrl`: the row values are formatted strings that `App` gives.

`App::version_row_value(key) -> String` (a new small module `src/app/version_rows.rs`) formats `BuildInfo::current()` for `ClientVersion` and `self.player.remote().owner_build()` for `OwnerVersion`, as `version · build · wire`, with an empty value shown as unknown. `Model::main_settings_rows` and `key_row` take the two strings and use them for the `Info` rows. Both Clients use this one `App` method, so the TUI and the GUI show the same text.

`setting_row_kind` puts both keys in a new `SettingRowKind::ReadOnly`. `handle_settings_activate` returns at once for `ReadOnly`, before it schedules a save, so the spec's "saves nothing" holds. `Passive` is not reused, because a `Passive` activation still schedules a save. In `pinned_setting_request`, the two keys join the exhaustive `None` arm. `SettingKey::scope` (from `add-gui-settings-screen`) returns `AllClients` for both.

A plain line below the grid was considered and rejected by the user in favor of rows in the Session section.

### D5. The GUI shows the rows read-only

The GUI F2 row frame (from `add-gui-settings-screen` D9) gets the row kind `info`, and its value is `App::version_row_value`. The Svelte Session section shows an `info` row as a label and a value with no control. A keyboard or pointer activation of it sends no `settings` command. Because the rows are `AllClients`, they appear in the GUI Session section through the existing scope filter and need no new section code.

## Risks / Trade-offs

- [A behavior-only Owner change attaches without a prompt, so a test can run against old Owner logic] → The Settings rows show both build identities. The user accepted this trade-off.
- [A `schemars` upgrade can change the generated schema without a wire change] → The wire identity changes one time, and the next attach shows the restart prompt one time. The conformance byte-compare fails in the same upgrade, so the change is visible.
- [The normalization misses an annotation keyword that a future `schemars` version adds] → The result is a too-strict gate (a false refusal), not a false attach. The unit test in task 1.2 holds the normalization rules.
- [A new ctrl type gets a property map under a keyword that is not in the list of D1] → The same direction: a too-strict gate. No wire change can make two wire identities equal through normalization, because only annotation keywords are removed.
- [The first attach from this change to an older running Owner prompts one time] → This is the expected result for an Owner that sends no wire identity. The proposal marks it as BREAKING.
- [`connect/tests.rs` is close to 800 lines] → The new gate tests go in a new child module `connect/tests/owner_gate.rs`, and the build-id cases move there.

## Migration Plan

No data migration. On the first launch after the update, a running Owner from before this change gets the existing restart prompt one time. Rollback is a revert. An Owner from this change and a Client from before it are refused by the old Client's build-identity check, as today.

## Open Questions

None.
