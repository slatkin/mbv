# Tasks

Each group is one commit that leaves the workspace green. Run the groups in
order.

**The gate** (run from the repo root after each group):

```
cargo fmt
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

- Accept whatever `cargo fmt` reflows, and never revert it.
- Never add an `allow`/`expect` attribute. If an item goes dead after it
  moves, delete the item and its tests (design D1). If a lint can't be fixed
  at the source, stop and ask.
- Never add a `pub use` re-export for a moved path. The old path must not
  resolve.
- No painter output, key handling, mouse handling, `Msg` payload, log text,
  or persisted value may change. Existing tests pass unchanged apart from
  their `use` paths, except where a task says otherwise.
- Moved items in `src/app/state/` use `pub(in crate::app)` visibility.
- Bulk path rewrites: use `sed -i` over `src/ crates/` for the exact prefixes
  named in the task, then let `cargo check` output drive the rest. Split by
  hand any grouped `use mbv_ui_model::x::{…}` import that mixes moved and
  unmoved names.

## 1. Service runtime structs leave `mbv-core` (design D4)

- [x] 1.1 Move `EmbyRuntime` and `AudiobookshelfRuntime` (with their `impl`s,
  and the tests in `crates/mbv-core/src/service_runtime.rs` that exercise
  them) to a new `src/app/state/service_runtime.rs`, declared from
  `src/app/state.rs`. Keep `ServiceState` and `SetupGeneration` in `mbv-core`,
  and make `SetupGeneration::next` `#[must_use] pub const fn`. Rewrite
  `mbv_core::service_runtime::{EmbyRuntime,AudiobookshelfRuntime}` imports in
  `src/` to `crate::app::state::service_runtime::…`. Verify with
  `rg "service_runtime::(EmbyRuntime|AudiobookshelfRuntime)" crates` (no
  hits).
- [x] 1.2 Remove `mbv-emby` and `mbv-audiobookshelf` from
  `crates/mbv-core/Cargo.toml` `[dependencies]`. Remove each
  `[dev-dependencies]` entry that no remaining `mbv-core` source names. Verify
  that `cargo tree -p mbv-core -e normal --depth 1` lists no `mbv-*` crate,
  then run the gate.

## 2. Move the shell-only event and queue-owner types (design D1)

- [x] 2.1 Move `crates/mbv-ui-model/src/events.rs` whole to
  `src/app/state/events.rs`. Rewrite `mbv_ui_model::events::` to
  `crate::app::state::events::` and delete `pub mod events;` from
  `crates/mbv-ui-model/src/lib.rs`. Its `use crate::playback::…` /
  `crate::queue_owner::…` imports now point at `mbv_ui_model::playback::…`
  until 2.3 and 3.1 move those types. Verify with
  `rg "mbv_ui_model::events" src crates` (no hits).
- [x] 2.2 Move `crates/mbv-ui-model/src/player_tab.rs` whole to
  `src/app/state/player_tab.rs`, the same way as 2.1 (rewrite
  `mbv_ui_model::player_tab::`, delete the `mod` line). Verify with
  `rg "mbv_ui_model::player_tab" src crates` (no hits).
- [x] 2.3 Merge `crates/mbv-ui-model/src/queue_owner.rs` (`QueueEpoch`,
  `QueueOrigin`) into the existing `src/app/state/queue_owner.rs`, then delete
  the ui-model file and its `mod` line. Rewrite
  `mbv_ui_model::queue_owner::` to `crate::app::state::queue_owner::`. Verify
  with `rg "mbv_ui_model::queue_owner" src crates` (no hits), then run the
  gate.

## 3. Split `playback` and `home_latest` (design D1)

- [x] 3.1 Create `src/app/state/playback.rs` holding every item of
  `crates/mbv-ui-model/src/playback.rs` except `PlaybackState` and
  `QueueScope` (`QueueScopeResolution`, `UndoEntry`, `RemoteSlotState`,
  `DestinationLatestSource`, `DestinationLatestSnapshot`, `HomeContent`,
  `SuspendedLocalSession`, `PendingQueueAction`, `ReplacementExecutor`,
  `RoutedReplacementPrep`, `PlaylistMutation`, `PlaylistMutationState`).
  Leave only `PlaybackState` and `QueueScope` in the ui-model file. Rewrite
  imports of the moved names to `crate::app::state::playback::…`. Verify that
  `crates/mbv-ui-model/src/playback.rs` no longer names `mbv_player`,
  `mbv_ws`, `mbv_ctrl`, `mbv_core` or `mpsc`.
- [x] 3.2 Move `capture_launch_window` and `current_launch_secs` from
  `crates/mbv-ui-model/src/home_latest.rs` to a new
  `src/app/state/home_latest.rs`, and update the callers `src/main.rs` and
  `src/app/shell.rs`. (`main.rs` sits outside `crate::app`, so make the two
  functions `pub(crate)`.) `HomeLatestLaunchWindow`,
  `is_new_in_launch_window`, `provider_timestamp_secs` and their tests stay.
  Verify with `rg "mbv_config::(load|save)_home_latest" crates` (no hits),
  then run the gate.

## 4. Sessions sidebar paints `SessionTargetRow` (design D2)

- [x] 4.1 Add `SessionTargetRow` (fields exactly as in design D2) and
  `SessionTargetRow::key()` to `crates/mbv-ui-model/src/panel_targets.rs`.
  Move `PanelTarget` and `resolve_session_target` into the existing
  `src/app/state/panel_targets.rs`, and add `PanelTarget::row(&self) ->
  SessionTargetRow` there.
- [x] 4.2 Change `SessionsComponent` (`crates/mbv-components/src/sessions.rs`)
  to store `Vec<SessionTargetRow>` and take `&[SessionTargetRow]` in
  `set_content`. `project_targets` reads the row fields in place of
  `session.*` / `receiver.*`. Its painted text must not change. Map
  `PanelTarget::row` at the call site in `src/app/shell/overlays/sidebars.rs`.
  Rewrite the component's tests to build `SessionTargetRow`s instead of
  `make_session` / `CastReceiver`, then remove `mbv-emby` from
  `crates/mbv-components/Cargo.toml` `[dev-dependencies]` if nothing else uses
  it. Verify with `rg "mbv_emby|mbv_cast" crates/mbv-components crates/mbv-ui-model`
  (no hits), then run the gate.

## 5. Device picker carries endpoint strings (design D3)

- [x] 5.1 Change `LibraryRouteStage::PickDevice.devices` in
  `crates/mbv-ui-model/src/context_menu.rs` to `Vec<(String, Option<String>)>`.
  In `enter_device_stage` (`src/app/shell/overlays/menus.rs`), keep the typed
  list, the eligibility logging and the parsed-endpoint cursor comparison.
  Convert each endpoint with `to_string()` only when building the stage. In
  `commit_device_selection`, insert the string as is. The flash and log text
  stay byte-identical. Verify with
  `rg "mbv_remote_player" crates/mbv-ui-model crates/mbv-render crates/mbv-components`
  (no hits), then run the gate.

## 6. Drop the edges and prove the boundary

- [ ] 6.1 Remove `mbv-player`, `mbv-emby`, `mbv-remote-player`, `mbv-ws`,
  `mbv-cast` and `mbv-ctrl` from `crates/mbv-ui-model/Cargo.toml`. Run the
  gate.
- [ ] 6.2 For each of `mbv-ui-model`, `mbv-ui-msg`, `mbv-render` and
  `mbv-components`, run `cargo tree -p <crate> -e normal --prefix none` and
  confirm that none of `mbv-player`, `mbv-ctrl`, `mbv-emby`, `mbv-ws`,
  `mbv-remote-player`, `mbv-cast` or `mbv-daemon` appears. Record the four
  results in the commit message.
- [ ] 6.3 Update the `mbv-core` line in `AGENTS.md`'s repository map to "app
  logging + Service state/setup-generation types". Verify with
  `rg "mbv-core" AGENTS.md`.
