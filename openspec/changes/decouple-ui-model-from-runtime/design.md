# Design

## Context

Measured on `main` at 13e0304d8. Each runtime dependency of `mbv-ui-model` is
there because of one of the items below. The "UI use" column shows who, among
`mbv-components`, `mbv-render` and `mbv-ui-msg`, names the item.

| Item (in `mbv-ui-model`) | Runtime crate it pulls in | UI use |
|---|---|---|
| `events::{LibEvent, SessionEvent, NavigateLanding, PendingSeries*}` | `mbv-player`, `mbv-emby`, `mbv-core` | none |
| `playback::SuspendedLocalSession` | `mbv-player`, `mbv-ws`, `mbv-ctrl`, `mbv-core` | none |
| other shell-only `playback` types (`PendingQueueAction`, `ReplacementExecutor`, `RoutedReplacementPrep`, `UndoEntry`, `RemoteSlotState`, `QueueScopeResolution`, `DestinationLatest*`, `HomeContent`, `PlaylistMutation*`) | — | none (`HomeContent` in components is a different type) |
| `playback::{PlaybackState, QueueScope}` | — | components, ui-msg |
| `player_tab::PlayerTab` (`from_unified_state`, `set_unified_state`) | `mbv-ctrl` | none |
| `queue_owner::{QueueEpoch, QueueOrigin}` | — | none |
| `panel_targets::PanelTarget` (`Emby(Box<SessionInfo>)`, `Cast(CastReceiver)`) | `mbv-emby`, `mbv-cast` | `SessionsComponent` stores and paints it |
| `panel_targets::SessionTargetKey` | — | components, ui-msg |
| `context_menu::LibraryRouteStage::PickDevice { devices: Vec<(String, Option<DaemonEndpoint>)> }` | `mbv-remote-player` | the component stores it, the painter reads `len()`/names; the shell reads it back on commit |
| `confirm::ConfirmAction::{ReplaceEmby, ReplaceAudiobookshelf}(SetupGeneration)` | `mbv-core` → `mbv-emby`, `mbv-audiobookshelf` | components match variants only |
| `home_latest::{capture_launch_window, current_launch_secs}` (config disk I/O) | — | none (`main.rs`, `shell.rs`) |

`mbv-render` depends on `mbv-core` only for `service_runtime::ServiceState`.
`mbv-core`'s `EmbyRuntime`/`AudiobookshelfRuntime` (which hold an
`EmbyClient` / `AudiobookshelfUser`) are the only reason `mbv-core` depends on
the provider crates, and only `src/app` uses them. `mbv-player` and
`mbv-daemon` use only `SetupGeneration`.

## Goals / Non-Goals

**Goals:**

- `cargo tree -p <crate> -e normal` for each of `mbv-ui-model`, `mbv-ui-msg`,
  `mbv-render` and `mbv-components` lists none of `mbv-player`, `mbv-ctrl`,
  `mbv-emby`, `mbv-ws`, `mbv-remote-player`, `mbv-cast`, `mbv-daemon`.
- `mbv-core` has no internal (`mbv-*`) dependencies.
- Every moved item has exactly one home; old paths do not resolve; no
  re-export shims.

**Non-Goals:**

- Removing `mbv-config`, `mbv-feed` or `mbv-audiobookshelf` from the UI
  crates. Components use those types directly (`TuiLaunchState`, row DTOs).
- Moving `settings::setting_value(&Config, …)` out of `mbv-ui-model`. It is
  shell-only, but it pulls in no runtime crate.
- Moving the pure `home_latest` types (`HomeLatestLaunchWindow`,
  `is_new_in_launch_window`, `provider_timestamp_secs`). They pull in no
  runtime crate, and components call `provider_timestamp_secs`.
- Renaming `mbv-core`.

## Decisions

### D1: Shell-only types move to `src/app/state/`, merging with existing owners

| Moves from `mbv-ui-model` | To |
|---|---|
| `events.rs` (whole) | `src/app/state/events.rs` (new) |
| `player_tab.rs` (whole) | `src/app/state/player_tab.rs` (new) |
| `queue_owner.rs` (whole) | merged into the existing `src/app/state/queue_owner.rs`, which already imports `QueueOrigin` from it |
| `playback.rs` minus `PlaybackState` and `QueueScope` | `src/app/state/playback.rs` (new) |
| `home_latest::{capture_launch_window, current_launch_secs}` | `src/app/state/home_latest.rs` (new) |
| `panel_targets::{PanelTarget, resolve_session_target}` | merged into the existing `src/app/state/panel_targets.rs`, which already builds `PanelTarget`s |

Moved items take the visibility used in `src/app/state` (`pub(in crate::app)`).
Moving items from a library crate into the binary exposes any that are dead to
the `dead_code` lint. Delete any item that goes dead, along with its tests;
don't suppress the lint.

Alternative rejected: a new `mbv-shell-state` crate. Nothing outside the TUI
crate uses these types, so a crate would only add a manifest and
cross-crate `pub`.

### D2: Sessions sidebar paints a `SessionTargetRow` presentation model

In `mbv-ui-model/src/panel_targets.rs`, next to `SessionTargetKey`:

```rust
pub enum SessionTargetRow {
    Emby { id: String, device_name: String, client: String, user_name: String,
           host: String, now_playing: Option<String>, is_paused: bool,
           position_s: i64, runtime_s: i64 },
    Cast { id: String, friendly_name: String },
}
impl SessionTargetRow { pub fn key(&self) -> SessionTargetKey }
```

These are exactly the fields `SessionsComponent::project_targets` reads today.
`SessionsComponent` stores `Vec<SessionTargetRow>`, and `set_content` takes
`&[SessionTargetRow]`. The shell adds `PanelTarget::row(&self) ->
SessionTargetRow` in `src/app/state/panel_targets.rs` and maps at the one
`set_content` call site (`src/app/shell/overlays/sidebars.rs`). Activation already sends `SessionTargetKey`, which
the shell resolves against its own `Vec<PanelTarget>` via
`resolve_session_target`, so that path is unchanged.

Alternatives rejected:
- Moving `SessionInfo` to `mbv-emby-model` and `CastReceiver` somewhere
  lower. `CastReceiver` has no natural lower home, and the component would
  still receive provider records (supported commands, media info) it never
  paints.
- Depending on `mbv-cast` from `mbv-ui-model`. It is a provider client crate
  (`rust_cast`, mDNS), so this would keep a runtime edge.

### D3: `PickDevice` carries the endpoint as its persisted string

`devices: Vec<(String, Option<String>)>`. The `Some` value is
`DaemonEndpoint::to_string()`, the exact string `commit_device_selection`
writes into `library_routes` today. In `enter_device_stage` (`src/app/shell/overlays/menus.rs`), the shell still
builds the typed `Vec<(String, Option<DaemonEndpoint>)>`, still logs
eligibility, and still computes the initial cursor by comparing parsed
`DaemonEndpoint` values, as it does today. It converts to strings only when it
builds the stage. `commit_device_selection` inserts the string directly, and
its log line prints the same text. The `(name, None)` "not routable" branch is
unchanged.

The endpoint doesn't get a newtype. The config map it is written into is
already `BTreeMap<String, String>`, so a newtype here would convert at both
ends and guard nothing.

Alternative rejected: have the shell keep the typed device list in a field and
push only names. The shell would then store the same fact that the stage
already carries.

### D4: The Service runtime structs leave `mbv-core`

`EmbyRuntime` and `AudiobookshelfRuntime`, along with their tests, move to
`src/app/state/service_runtime.rs`. `mbv-core::service_runtime` keeps
`ServiceState` and `SetupGeneration`, and `SetupGeneration::next` becomes
`pub const fn` because the moved runtimes call it. `mbv-core` drops `mbv-emby`
and `mbv-audiobookshelf`, plus any dev-dependencies left unused (today:
`mbv-config`, `mbv-queue`, `ureq`, `serde_json`, `libmpv2`, `rust_cast`;
remove each one that no remaining `mbv-core` test uses). `mbv-render` and
`mbv-ui-model` keep their `mbv-core` dependency, which is now a leaf.

Alternative rejected: moving `ServiceState`/`SetupGeneration` into
`mbv-ui-model` or `mbv-ids`. `mbv-player` and `mbv-daemon` use
`SetupGeneration`. Neither crate is a UI crate, and `SetupGeneration` isn't a
media identifier.

### D5: Order: cut the dependency edges, then drop them from the manifests

Each group leaves `cargo check --workspace` green. Groups 1–4 touch disjoint
items: D4, D1, D2 and D3. Group 5 removes the manifest dependencies and runs
the `cargo tree` gate. Removing the dependencies is the proof: a missed use
fails to compile.

## Risks / Trade-offs

- [`dead_code` fires on moved items that were only "used" by being `pub` in a
  library] → delete them per D1. The group's clippy gate catches every one.
- [`SessionTargetRow` duplicates the display fields of `SessionInfo`] → this
  is intended: it is a presentation projection, built once per push at the
  shell boundary.
- [The device-picker cursor could drift if the initial-cursor comparison were
  switched to comparing strings] → D3 keeps the typed comparison in the
  shell.
- [Merge conflicts with in-flight work importing `mbv_ui_model::playback::*`]
  → a pure path rewrite that the compiler guides. No behaviour moves.

## Migration Plan

This is a pure refactor on one branch, with no runtime migration. To roll
back, revert the commit.
