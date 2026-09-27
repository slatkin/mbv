# Design

## Context

Edges between the moving modules, counted as `crate::<module>` references on
`main` at `c9331f37c`. Each is marked with what it becomes once Tier 2 has
landed:

- **`audiobookshelf → api` (4 refs):** `html_to_text` and `TICKS_PER_SECOND`
  only. Tier 2 moves these to `mbv-text::html` and `mbv-emby-model`, so the
  edge goes away.
- **`api → audiobookshelf` (6 refs):** all in `api/tests/failure.rs`, which
  holds the Audiobookshelf client's HTTP-boundary tests next to the Emby ones.
  No production code is involved. So the "cycle" #814 names for Tier 3 is one
  misplaced test file.
- **`api ↔ service_runtime`:** `api` constructs and returns `EmbyFailure`,
  `EmbyFailureClass`, and `EmbyBootstrap` (15 refs). `service_runtime` holds
  `EmbyClient` in `EmbyRuntime`, `EmbyItem` in `EmbyBootstrap`, and
  `AudiobookshelfUser` in `AudiobookshelfRuntime`.
- **`api → cast` (10 refs):** `EmbyClient::get_playback_info_for_cast` takes a
  `cast::dispatch::CastDeviceProfile` and returns a `cast::client::CastMediaItem`.
  It needs `EmbyClient`'s private `post`, `user_id`, `token`, and `config`.
- **`cast → audiobookshelf` (1):** `dispatch.rs` resolves
  `AudiobookshelfAudioSource` for episodes. `cast → config` is `FeedKind` only,
  which Tier 2 moves to `mbv-queue`.
- **`api → player` (2):** `SubtitlePrefs`, which Tier 2 moves to
  `mbv_ctrl::player`.
- **`remote_player → player` (11):** `PlayerCommand`, `PlayerEvent`,
  `PlayerStatus`, `SubtitlePrefs`, and `CONNECTION_LOST_MESSAGE`. All of these
  move to `mbv-ctrl` in Tier 2, which leaves only `player → remote_player`
  (`PlayerProxy::Remote(RemotePlayer)`, 5 refs).
- **`player → service_runtime` (11):** all `SetupGeneration`.
- **`daemon`** references everything below it except `remote_player` and
  `cast`. Nothing below it references it. `mbvd`'s `main.rs` and the TUI's
  `src/local_daemon.rs` are its only external callers, so it has to stay a
  library.
- Test-only items are already gated `#[cfg(any(test, feature = "test"))]` in
  `player/proxy.rs`, `player/controller.rs`, `remote_player.rs`, and
  `remote_player/connect.rs`. The TUI enables them through `mbv-core`'s
  `test` feature (root `Cargo.toml` dev-dependency). A few test helpers are
  still plain `#[cfg(test)] pub(crate)`, for example
  `Player::spy_on_commands` (controller) called from `daemon/tests`.

## Goals / Non-Goals

**Goals:**

- The crate graph is acyclic and reads, bottom-up over the Tier 2 crates:
  `mbv-audiobookshelf → mbv-cast → mbv-emby → mbv-core → mbv-remote-player
  → mbv-player → mbv-daemon`. Each crate may also use any lower Tier 1/2
  crate. The TUI and `mbvd` sit on top.
- No provider crate depends on `mbv-core`, `mbv-player`, or `mbv-daemon`.
  `mbv-audiobookshelf` does not depend on `mbv-emby`.
- Old module paths do not resolve, and no re-export shims are added.
- Every cycle cut moves an existing item to the module that owns it. No item
  changes signature or behaviour.

**Non-Goals:**

- Removing edges that are unused but harmless. The main one is
  `mbv-remote-player → mbv-emby`, which exists only for the ignored
  `_client: Arc<EmbyClient>` parameters (dropping them would be a signature
  change across the TUI).
- Making `mbv-emby` independent of `mbv-ctrl` (`SubtitlePrefs`) or of
  `mbv-cast` (the cast `PlaybackInfo` request).
- Splitting any file just because it moved. The pre-push line check governs
  that.

## Decisions

### D1: Cut inside `mbv-core` first, then extract with pure moves

This is the same approach as Tier 2 D1. Group 1 does the two in-core cuts (the
misplaced Audiobookshelf tests, and the `EmbyFailure*`/`EmbyBootstrap` move
into `api`) while everything is still one crate, so a single
`cargo check -p mbv-core` proves them. Every later group then follows the
Tier 1 shape: new manifest, `git mv`, path rewrite, delete the old `mod`
line. The extraction order runs bottom-up, so each new crate depends only on
crates that already exist.

### D2: `cast` is its own crate below `mbv-emby`

#814 suggests folding `cast` into the Emby crate. But `cast/dispatch.rs`
decides castability for all three providers (Emby profile, feed enclosure,
Audiobookshelf episode token, book exclusion), and it needs
`AudiobookshelfAudioSource`. If it were folded in, `mbv-emby` would depend on
`mbv-audiobookshelf`, putting one provider on top of another. As a separate
`mbv-cast`, the real edges show: cast knows the Audiobookshelf source shape,
and the Emby client builds a cast request.

The alternative was to move `get_playback_info_for_cast` into `mbv-cast`, but
that function needs `EmbyClient` internals and would force widening them.
The Emby client stays the only code that speaks Emby HTTP, so
`mbv-emby → mbv-cast` it is.

### D3: `EmbyFailure`, `EmbyFailureClass`, `EmbyBootstrap` go to `mbv-emby`

These are what the Emby client returns, and `EmbyBootstrap` is two
`Vec<EmbyItem>`. They move into `api` as a new `api/failure.rs` (declared
`mod failure; pub use failure::*;`, the same pattern as `api.rs`'s other
submodules), so after extraction they are `mbv_emby::EmbyFailure`. The rest of
`service_runtime` stays where it is: `ServiceState`, `SetupGeneration`,
`EmbyRuntime`, and `AudiobookshelfRuntime` hold both providers' handles and
exist to be shared by `mbv-player`, `mbv-daemon`, and the TUI.

The alternative was to move `service_runtime` into `mbv-emby` whole, which
would make the Emby crate own Audiobookshelf runtime state.

### D4: `mbv-core` survives as the Service runtime + logging crate

After every other move, `mbv-core` holds only `applog` and `service_runtime`.
Keeping the crate means the 39 TUI files that use `mbv_core::service_runtime`
and the three that use `mbv_core::applog` need no rewrite. Its position
(above the providers, below `mbv-player`) is correct for what it now contains.

Alternatives rejected:
- Deleting `mbv-core` and adding `mbv-services` + `mbv-applog`: two renames
  with no boundary gain.
- Moving `SetupGeneration` into `mbv-player`: `EmbyRuntime` and
  `AudiobookshelfRuntime` use it too, so `mbv-core` would then depend on
  `mbv-player`.

### D5: `mbv-daemon`, not `mbv-daemon-core`

The daemon library runs both inside `mbvd` and in-process from the TUI's
`src/local_daemon.rs`, so it cannot move into the `mbvd` binary crate. #814's
working name `mbv-daemon-core` reads too much like `mbv-core`. The crate is
`mbv-daemon` and its path is `mbv_daemon::…`.

### D6: Crate roots and in-crate paths

Each module file becomes its crate's `src/lib.rs`, and its directory becomes
`src/`. Examples: `api.rs` → `crates/mbv-emby/src/lib.rs` and `api/*` →
`crates/mbv-emby/src/*`; `player.rs` → `crates/mbv-player/src/lib.rs`. Inside
each crate, `crate::<module>::` rewrites to `crate::`. Outside, the paths
become `mbv_emby::`, `mbv_audiobookshelf::`, `mbv_cast::`,
`mbv_remote_player::`, `mbv_player::`, and `mbv_daemon::`. The
`mbv_core::player_owner_state` alias is deleted, and its 2 TUI users switch to
`mbv_player::owner_state::`.

### D7: Test gating follows Tier 1 D3 / Tier 2 D9

Every new crate whose code has `#[cfg(any(test, feature = "test"))]` items gets
`[features] test = […]`, which forwards to the `test` feature of each lower
crate it has test items from (at minimum `mbv-net/test` wherever `mock_http`
is used outside `#[cfg(test)]`). A `#[cfg(test)] pub(crate)` helper that a
test in a different crate calls becomes `#[cfg(any(test, feature = "test"))]
pub`. The calling crate enables the feature in `[dev-dependencies]`. Consumers
(TUI, `mbvd`, the higher crates) add
`<crate> = { path = …, features = ["test"] }` to their dev-dependencies in
place of relying on `mbv-core/test`. Once group 8 is done, `mbv-core` keeps a
`test` feature only if one of its own items still needs it.

### D8: One change, commit-sized groups

The reasoning is the same as Tier 1 D4 and Tier 2 D10. Each group leaves the
workspace green and can ship as its own PR. Groups 6 and 7 (`mbv-player`,
`mbv-daemon`) are the largest moves (~11k lines each) and stay one group each,
because the path rewrite only compiles once the whole module has moved.

## Risks / Trade-offs

- **Tier 2 drift.** This plan names the paths Tier 2's tasks create, and Tier 2
  has not started yet (it is queued behind `decompose-app-god-type` and
  `enforce-audiobookshelf-both-shapes`). → Group 0 verifies each assumed path. If one
  differs, it stops and reports rather than guessing a mapping.
- **Inherent impls left behind.** An `impl EmbyClient { … }` or
  `impl Player { … }` block in a file that does not move stops compiling across
  a crate boundary. → Measured: the only such blocks are inside the moving
  modules. If the compiler finds one, it moves with its type, or the group
  stops and asks.
- **`pub(crate)` widening.** Roughly 125 items cross the new boundaries and
  become `pub` (the same trade as Tier 1 D5). Newly `pub` items can trip
  `missing_panics_doc`/`must_use_candidate`. Fix at the source; never with
  `allow`.
- **Rewrite volume.** `mbv_core::api::` is used in 123 TUI files. → Bulk
  `sed` over the exact prefixes, then `cargo check` output drives the rest.
  Grouped imports such as `use mbv_core::{api, player, service_runtime}` are
  split by hand.
- **Build graph overhead.** There are six more crates. The gain is that an edit
  to `daemon` no longer recompiles the providers or the player, and the
  provider → runtime direction becomes a compile error. This is not gated on a
  benchmark.

## Migration Plan

Nothing persisted or on the wire changes: `serde` derives move with their
types, and the ctrl protocol version is unchanged. To roll back a group,
`git revert` its commit.
