# Proposal

## Why

Issue #814 Tiers 3 and 4. After Tier 2 (`extract-tier-2-domain-crates`),
`mbv-core` still holds the three providers (`api`, `audiobookshelf`, `cast`)
and the heavy runtime (`player`, `remote_player`, `daemon`), roughly 32k
lines in one crate. Any edit to any of them recompiles all of them, and the
compiler does not enforce which of them may depend on which. Tier 2 already
removes most of the cross-edges, so what is left is a handful of misplaced
items followed by pure moves. Planning both tiers together gives one
dependency graph for the whole split instead of two that could each assume
something different about the other.

Measured on `main` at `c9331f37c`, projected onto the layout Tier 2 leaves.

## What Changes

Six new crates, in dependency order. There is no change to behaviour, the
wire protocol, persistence, the config format, or key bindings.

| New crate | Moved from (`crates/mbv-core/src/`) | Workspace deps |
|---|---|---|
| `mbv-audiobookshelf` | `audiobookshelf.rs` + `audiobookshelf/` | `mbv-config`, `mbv-queue`, `mbv-emby-model`, `mbv-text`, `mbv-net` |
| `mbv-cast` | `cast.rs` + `cast/` (Google Cast client, discovery, per-provider dispatch) | `mbv-audiobookshelf`, `mbv-queue` |
| `mbv-emby` | `api.rs` + `api/`, plus `EmbyFailure`, `EmbyFailureClass`, `EmbyBootstrap` from `service_runtime.rs` | `mbv-cast`, `mbv-config`, `mbv-ctrl`, `mbv-emby-model`, `mbv-text`, `mbv-net`, `mbv-ws`, `mbv-ids` |
| `mbv-remote-player` | `remote_player.rs` + `remote_player/` | `mbv-ctrl`, `mbv-config`, `mbv-queue`, `mbv-emby`, `mbv-emby-model`, `mbv-net` |
| `mbv-player` | `player.rs` + `player/` (controller, proxy, runtime, reporting, `transition`, `tracks`, `owner_state`, …) | `mbv-remote-player`, `mbv-core`, `mbv-emby`, `mbv-audiobookshelf`, `mbv-ctrl`, `mbv-config`, `mbv-queue`, `mbv-ws`, `libmpv2` |
| `mbv-daemon` | `daemon.rs` + `daemon/` | `mbv-player`, `mbv-core`, `mbv-emby`, `mbv-audiobookshelf`, `mbv-ctrl`, `mbv-config`, `mbv-queue`, `mbv-ws` |

After the split, `mbv-core` keeps only `applog` and `service_runtime` (the
Service runtime state machines and `SetupGeneration`). It depends on
`mbv-emby` and `mbv-audiobookshelf`, and `mbv-player`/`mbv-daemon` depend on it.

Cycle cuts inside `mbv-core`, done before any extraction:

- The two Audiobookshelf client tests in `api/tests/failure.rs` and their
  `audiobookshelf_response` helper (`audiobookshelf_me_http_boundary_…`,
  `dead_audiobookshelf_endpoint_is_connectivity`) move to the
  `audiobookshelf` test module. They are the only `api → audiobookshelf` edge.
- `EmbyFailure`, `EmbyFailureClass`, `EmbyBootstrap` move from
  `service_runtime.rs` into `api`. The Emby client returns them, and that edge
  is `api → service_runtime` (15 refs) inside a cycle with
  `service_runtime → api`.

Changes that follow from the split:

- **BREAKING (inside the workspace only):** every moved path changes, and no
  re-export shim is left behind. `mbv_core::{api, audiobookshelf, cast, player,
  remote_player, daemon}` stop resolving, and so does the
  `mbv_core::player_owner_state` compatibility alias (2 TUI files).
- Test-only helpers gated `#[cfg(test)]` that a crate on the far side of a new
  boundary calls (e.g. `Player::spy_on_commands`, used by daemon tests) become
  `#[cfg(any(test, feature = "test"))] pub`. Each crate that has such items gets
  a `test` feature.
- The orphaned `crates/mbv-core/src/id_types.rs` (not declared in `lib.rs`;
  Tier 1 moved its contents to `mbv-ids`) is deleted.
- The `mbv-core` package description is updated to match its reduced contents.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. This is a code-organisation refactor with no externally observable
behaviour change, so `skip_specs: true`.

## Impact

- **Prerequisite:** `extract-tier-2-domain-crates` is merged. Every path and
  crate named here (`mbv-ctrl`, `mbv-config`, `mbv-queue`, `mbv-emby-model`,
  `mbv-text::html`, `player::transition`, `player/tracks.rs`) is what Tier 2
  leaves behind. Group 0 checks this and stops if it is not merged.
- **New:** six `crates/mbv-*/` directories. Workspace `members` and
  `default-members` each gain six entries.
- **Call sites** (files per module path today, TUI + `mbvd` + in-core):
  `api` 123+1+46, `service_runtime` 39+0+15 (only the 8 files that name
  `EmbyFailure*`/`EmbyBootstrap` change), `player` 36+1+46, `audiobookshelf`
  35+2+10, `remote_player` 27+0+7, `daemon` 1+1+20, `cast` 7+0+3. All of these
  edits are forced by the compiler.
- **Visibility widening:** `pub(crate)` items that a new boundary now has to
  cross become `pub`: ~55 in `player`, ~55 in `daemon`, ~14 in
  `remote_player`. The compiler decides which ones.
- **Docs:** the repository map in `AGENTS.md` gains the six crates, and its
  `mbv-core` line shrinks.
- **Sequencing:** Tier 2 is scheduled after `decompose-app-god-type` and
  `enforce-audiobookshelf-both-shapes`, and this change comes after Tier 2.
  Both of those will therefore have landed by the time this starts.
  `prune-tui-test-suite` edits `src/app/`, which every group touches only
  through path rewrites.
- **Out of scope:** Tier 5 (TUI crates); moving `daemon` into the `mbvd`
  binary (the TUI's `src/local_daemon.rs` runs it in-process as well);
  removing the unused `_client: Arc<EmbyClient>` parameters on
  `RemotePlayer::{play, play_queue}`, which is the only reason
  `mbv-remote-player` depends on `mbv-emby`; renaming `mbv-core`.
- Umbrella: issue #814, Tiers 3–4.
