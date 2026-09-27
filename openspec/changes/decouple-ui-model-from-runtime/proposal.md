# Proposal

## Why

Issue #830, a follow-up to #814 Tier 5. `mbv-ui-model` is meant to hold plain
presentation models, but it depends on `mbv-player`, `mbv-emby`,
`mbv-remote-player`, `mbv-ws`, `mbv-cast`, `mbv-ctrl` and `mbv-core`, and
`mbv-core` in turn depends on `mbv-emby` and `mbv-audiobookshelf`. The cause
is that Tier 5 moved whole state files into the crate, including types only
the shell uses. As a result:

- **Build time:** an edit to the player, Emby, websocket, remote-player or
  cast crate rechecks `mbv-ui-model`, `mbv-ui-msg`, `mbv-render` and
  `mbv-components` (about 40k lines). Avoiding exactly that rebuild was the
  reason for Tier 5.
- **Boundary:** a component can reach a `PlayerProxy`, a `WsSender` or a
  worker `Receiver` through the public fields of
  `mbv_ui_model::playback::SuspendedLocalSession`. So "components never
  receive PlayerProxy / channels" is still a review rule, not a compile error.

## What Changes

- Types only the shell uses move from `mbv-ui-model` into
  `src/app/state/`, with no re-export shims:
  - all of `events` (`LibEvent`, `SessionEvent`, `NavigateLanding`,
    `PendingSeries*`)
  - all of `player_tab` (`PlayerTab`, which the ctrl protocol constructs)
  - all of `queue_owner` (`QueueOrigin`)
  - the shell-only part of `playback`: `SuspendedLocalSession`,
    `PendingQueueAction`, `ReplacementExecutor`, `RoutedReplacementPrep`,
    `UndoEntry`, `RemoteSlotState`, `QueueScopeResolution`,
    `DestinationLatest*`, the shell's `HomeContent`, and `PlaylistMutation*`
  - the part of `home_latest` that captures the launch window and persists
    it: `HomeLatestLaunchWindow`, `capture_launch_window`,
    `current_launch_secs`, `is_new_in_launch_window`
  - `PanelTarget` and `resolve_session_target`
- The Sessions sidebar gets a presentation row, `SessionTargetRow`, in
  `mbv-ui-model`. The shell keeps `Vec<PanelTarget>` (which wraps the Emby
  `SessionInfo` and cast `CastReceiver`) and pushes the rows the component
  paints. `SessionTargetKey` stays where it is.
- `LibraryRouteStage::PickDevice` carries each device's endpoint as the
  display string that `library_routes` persists, instead of an
  `mbv_remote_player::DaemonEndpoint`.
- `EmbyRuntime` and `AudiobookshelfRuntime` move from `mbv-core` into
  `src/app/state/`, since only the TUI uses them. `mbv-core` keeps `applog`,
  `ServiceState` and `SetupGeneration`, and has no internal dependencies.
  `SetupGeneration::next` becomes public.
- Dependencies removed from `mbv-ui-model`: `mbv-player`, `mbv-emby`,
  `mbv-remote-player`, `mbv-ws`, `mbv-cast` and `mbv-ctrl`. Dependencies
  removed from `mbv-core`: `mbv-emby`, `mbv-audiobookshelf`, and its unused
  dev-dependencies.
- No change to user-visible behaviour, the wire protocol, the persistence
  format or the config format.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `interactive-component-framework`: the requirement that the UI crates
  can't reach shell state is extended to the Player and provider runtime.
  The UI crates must not depend on the Player, ctrl, websocket,
  remote-player, cast or Emby client crates, even transitively.

## Impact

- Crates: `mbv-ui-model`, `mbv-core`, `mbv-components` (`sessions.rs`),
  `mbv-render` (`library_routes.rs` pattern only), and the TUI crate
  (`src/app/state`, plus the import rewrites that follow).
- Callers: roughly 60 `use mbv_ui_model::{events,playback,player_tab,…}`
  sites and 20 `mbv_core::service_runtime::{EmbyRuntime,AudiobookshelfRuntime}`
  sites in `src/app` are rewritten to the new paths.
- Specs: `interactive-component-framework` (delta). The mentions of
  `LibEvent` in `playlist-management` name the type, not its crate, so they
  stay valid.
- Not in scope: `mbv-config` and the `mbv-audiobookshelf` row types stay as
  `mbv-ui-model` dependencies, because the components use them directly.
