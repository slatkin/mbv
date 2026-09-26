# Proposal

## Why

`App` (`src/app/state/app_struct.rs`) is a ~170-field struct with `impl App` blocks in 85 files. Startup/setup, runtime channels, remote-session tracking and the image cache all live as flat sibling fields, so any change can read or write any of them and nothing shows which code owns which state (issue #808). Its two main event dispatchers also route with semantics no type states: `handle_player_event` returns a bare `bool`, its stages route with `Result<bool, PlayerEvent>` (`Err` = "not mine"), and `handle_lib_event` chains `Option<LibEvent>` passthroughs that end in `unreachable!()` arms.

## What Changes

- Group four field sets out of `App` into owned sub-structs, each in its own file with its own constructor:
  - `ImageCache` (`app.images`): card image states, LRU, fetch queue, image worker channels, resize channels, pickers, protocol flags, card size.
  - `ServiceSetup` (`app.setup`): Emby/Audiobookshelf startup, test, setup and catalog receivers, startup requests, setup forms, pending replacements.
  - `RemoteTracking` (`app.remote`): remote position estimate, poll/stall/seek timers, session miss count, direct-remote connected/label/session id.
  - `RuntimeChannels` (`app.channels`): the App-created `lib`, `search`, `sessions`, `cast` and `notif_action` sender/receiver pairs.
- Methods that touch only one sub-struct's state move from `impl App` to `impl` of that sub-struct.
- `handle_player_event` becomes one exhaustive `match` over `PlayerEvent` and returns a named `PlayerEventFlow` enum instead of `bool`. The `Result<bool, PlayerEvent>` stage functions go away.
- `handle_lib_event` becomes one exhaustive `match` over `LibEvent`. The `Option<LibEvent>` family dispatchers (`handle_audiobookshelf_event`, `handle_browse_event`, `handle_music_event`, `handle_playlist_event`) and their `unreachable!()` arms go away. Inline arm bodies become named per-event handler methods.
- No user-visible behaviour changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a pure internal refactor (`skip_specs: true`).

## Impact

- `src/app/state/app_struct.rs`, `src/app/state/construct.rs`, `src/app/state/app_init.rs`: field removal, sub-struct construction.
- Every file that reads the moved fields, tests included: compile-forced path edits (`self.card_image_states` → `self.images.card_image_states`, and so on).
- `src/app/dispatch/session/player_event.rs`, `src/app/shell/run/drains.rs`, `src/app/dispatch/library/event.rs` and `event/*.rs`: dispatch reshaping.
- Shell `Model` is out of scope (see design.md, Non-Goals).
