# Change recipes

Each recipe is an ordered list of the sites that one recurring kind of change
touches. Do the steps in this order. Each step names the check that proves it.
A change that skips a site can compile and partly work, but it silently drops
that site. The ADRs and specs stay the source of the rationale. This file only
lists the sites.

Every recipe starts with an OpenSpec change (delta spec plus `tasks.md`) and
ends with the gates in `AGENTS.md` (*Tooling*).

## Add a ctrl protocol variant

Use this recipe for a new `CtrlCmd`, `CtrlEvent`, or `PlayerEvent` variant,
or for a new field on an existing message.

1. **Decide: capability or version bump.** Read the rules on
   `CTRL_PROTOCOL_VERSION` in `crates/mbv-ctrl/src/protocol.rs`. A new variant
   or a `#[serde(default)]` field is additive: it gets a capability and no
   bump. A rename, a removal, or a change of meaning, units, nullability,
   handshake, or framing is a bump. A bump needs explicit user approval. Never
   bump as a ride-along.
   *Proof:* the delta spec under `openspec/specs/ctrl-protocol/` states the
   choice.
2. **Vocabulary.** Add the variant in `crates/mbv-ctrl/src/commands.rs`,
   `events.rs`, or `player.rs`. Pin the wire tag with `#[serde(rename = "...")]`.
   For a new `CtrlCmd`, add an arm to the exhaustive matches in
   `CtrlCmd::requires_owner` and `CtrlCmd::mutates_owner_queue`. `CtrlCmd` is
   the ctrl wire vocabulary; it never becomes a *UI command* in the terminal
   application. The *Transport command* (`CONTEXT.md`) is a different type,
   `TransportCommand`: the owner-resolved transport action (`Step`, `Player`,
   `Stop`).
   *Proof:* `cargo check -p mbv-ctrl`.
3. **Capability.** Add a `CTRL_CAP_*` constant with a doc comment in
   `protocol.rs`. Re-export it from `lib.rs`. Advertise it in `CtrlHello`
   (`hello.rs`: `current()` or a `with_*` builder) and add a `supports_*`
   accessor. If the Owner advertises it, set it in `DaemonHelloCaps`
   (`crates/mbv-daemon/src/core_ctrl_spawn.rs`).
   *Proof:* a case in `crates/mbv-ctrl/tests/ctrl/handshake.rs`.
4. **Wire tags.** Extend the pinned-tag and round-trip tests in
   `crates/mbv-ctrl/tests/ctrl/wire.rs` (and `queue.rs` for queue shapes).
   *Proof:* `cargo nextest run -p mbv-ctrl`.
5. **Owner handling.** Handle a `CtrlCmd` in `crates/mbv-daemon/src/control.rs`
   (queue commands in `control_queue.rs`). Record the peer capability in
   `HelloClassification` (`core_ctrl_spawn.rs`) and on the client record
   (`crates/mbv-daemon/src/ctrl.rs`). Send a gated `CtrlEvent` only to peers
   that advertised the capability.
   *Proof:* `cargo nextest run -p mbv-daemon`.
6. **Client handling.** Send a `CtrlCmd` through `RemotePlayer::send_ctrl_cmd`
   (`crates/mbv-remote-player/src/lib.rs`). Decode a `CtrlEvent` in
   `crates/mbv-remote-player/src/connect.rs` and map it to a `PlayerEvent`.
   Handle the `PlayerEvent` in the shell (`src/app/dispatch/session/player_event.rs`
   or `src/app/shell/run/drains.rs`). Give it an exhaustive arm or a
   documented no-op.
   *Proof:* `cargo check -p mbv`.
7. **Packaged `mbvd` parity.** `mbvd` runs the `mbv-daemon` library, so step 5
   covers its Owner path. Also check the `CtrlEvent` decode in
   `crates/mbvd/src/main.rs` (Service-setup reconcile and Hello). It must
   still match the new shape.
   *Proof:* `cargo nextest run -p mbvd`.
8. **Fixtures.** When the conformance fixtures of #951 exist, add or update
   the fixture for the new shape. A bump opens a new versioned directory.

## Add a keybind action

Use this recipe for a new configurable keyboard action. Read
`openspec/specs/configurable-keybinds/spec.md` first.

1. **Registry.** Add a `KeybindAction` to `KEYBIND_ACTIONS` in
   `crates/mbv-keybinds/src/registry.rs`: `id`, `section`, `default_chords`,
   `gate`, `policy`, `rebindable`, and `prefix_addressable`. The `id` is also
   the config key. Add the id to the identifier list and the default-chord
   table in `crates/mbv-keybinds/src/tests.rs`.
   *Proof:* `cargo nextest run -p mbv-keybinds`.
2. **Routing policy.** In `src/app/input/key_policy.rs`, add a
   `KeyPolicyBinding` variant, a `KeyPolicyEntry` whose `name` equals the
   registry `id`, and an arm in `command_for_policy`. Routing stays in
   `src/app/input/`. Never route keys anywhere else.
   *Proof:* `cargo check -p mbv`.
3. **Dispatch.** The UI command is nested. Add the variant to the intent enum
   of its domain in `src/app/dispatch/action.rs` (`VisualIntent`,
   `PlaybackIntent`, `NavigationIntent`, or `PanelIntent`) and give it an arm
   in the matching sub-dispatcher. A shell-owned action goes to
   `ShellCommand` instead. If a component also requests the action,
   add the typed request in `crates/mbv-ui-msg/` and map it to the UI command
   in the shell (for example `src/app/shell/playback.rs`).
   *Proof:* `cargo clippy --workspace --all-targets -- -D warnings`.
4. **Help label.** A Playback-section action needs a label in
   `playback_label` (`crates/mbv-render/src/components/help.rs`). A missing
   label panics when the help overlay opens.
   *Proof:* no unit test paints every label. Ask the user to open help (F1)
   in a live run.
5. **Routing tests.** Add the expected outcome to the routing matrix in
   `src/app/tests/routing_matrix/`. If the default chord needs a gate open,
   add the id to the snapshot setup in `defaults_reproduce_todays_resolution`
   (`src/app/input/key_policy/resolution_tests.rs`).
   *Proof:* `cargo nextest run -p mbv`.

To retire an action, remove its sites in the reverse order and add its id to
`RETIRED_ACTION_IDS` in `crates/mbv-keybinds/src/config.rs`. An old config
then still loads.

## Add a Service capability

Use this recipe when a Service gets a new playable media kind (for example
Audiobookshelf books after podcast episodes). Read ADR 0018 (exhaustive
dispatch by destination), ADR 0019 (Player owners resolve Service playback),
and `docs/invariants/06-queue-progress-application-sites.md` first.

1. **Probe the API.** Confirm the endpoints and response shapes before the
   design (`AGENTS.md`, *Start here*).
2. **Provider client.** Add the fetch, play, and progress calls in the
   provider crate (`crates/mbv-audiobookshelf/` or `crates/mbv-emby/`).
   *Proof:* a hermetic test in that crate's `tests/` binary.
3. **Queue identity.** Add the kind to `QueueItemKind` and `QueueItem` in
   `crates/mbv-queue/src/items.rs`. Give it a content identity and a
   `required_service`. Every match on `QueueItem` must name it.
   *Proof:* `cargo check --workspace`.
4. **Owner admission.** Teach `admissible_for_owner*`
   (`crates/mbv-queue/src/items.rs`) and `daemon_admits`
   (`crates/mbv-daemon/src/control_queue.rs`) about the kind. Components never
   admit. Check the output admissions too (Cast: `crates/mbv-daemon/src/cast.rs`;
   Emby Session: `crates/mbv-daemon/src/emby_session/load.rs`).
   *Proof:* `cargo nextest run -p mbv-queue -p mbv-daemon`.
5. **Ctrl negotiation.** Follow the ctrl recipe above. A new queue kind needs
   a queue capability (like `abs-book-queue`). A new progress event needs its
   own capability (like `abs-book-progress`). Filter slots per peer in
   `unified_queue_state_for_peer` (`control_queue.rs`) and in
   `broadcast_state_gated` (`crates/mbv-daemon/src/ctrl.rs`).
6. **Progress and lifecycle.** Resolve the source and report progress in the
   Owner (`crates/mbv-daemon/src/` Service module). Apply acknowledged
   progress only at the sites that invariant 06 lists. Finalize the playback
   session on completion, stop, skip, queue replace, and teardown.
7. **Browse destination.** Add the destination to the exhaustive browse
   dispatch (input, rendering, refresh, help, hit testing). Do not inherit
   another destination's behavior through a default branch. Follow
   `docs/architecture/tui-frontend.md` for the component and painter.
8. **Vocabulary.** Add the new terms to `CONTEXT.md`.
