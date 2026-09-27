# Proposal

## Why

Issue #814 Tier 2: the queue, ctrl protocol, config, and feed modules are the
domain foundation every provider and runtime split (Tiers 3–4) sits on. Today
they are tangled into `mbv-core` through cycles (`queue ↔ ctrl ↔ player types`,
`config ↔ queue`, `config ↔ remote_player`, `config ↔ audiobookshelf`), so none
of the later tiers can be extracted until these edges are cut and the modules
become crates the compiler keeps acyclic.

Unlike Tier 1, this is not a pure move: each cycle is broken first by moving a
small number of items to the module that owns them, then the modules are
extracted. Measured on `main` at `922802192`.

## What Changes

Five new crates plus one widened crate, in dependency order. No behaviour,
wire-protocol, persistence, config-format, or key-binding change.

| New crate | Moved from | Deps (workspace) |
|---|---|---|
| `mbv-emby-model` | `mbv-core/src/api/types.rs`: `EmbyItem`, `EmbyImageTags`, `EmbyPerson`, `EmbyLink`, `EmbyArtistRef`, the tick/resume constants and `should_resume` | `mbv-ids` |
| `mbv-queue` | `mbv-core/src/playback/{queue,queue/items,execution_sequence}.rs` + tests; `QueueState`/`QueueSource` (`config/types_queue_state.rs`), `FeedKind` (`config/types_feed.rs`), `ServiceKind` (`config/types_setup.rs`), `QueueLineage` (`ctrl.rs`) | `mbv-emby-model`, `mbv-ids` |
| `mbv-ctrl` | `mbv-core/src/ctrl.rs` + tests; the Player vocabulary from `player/types.rs` (`PlayerCommand`, `PlayerEvent`, `PlayerStatus`, `SubtitlePrefs`, `SubtitleChoice`, `CONNECTION_LOST_MESSAGE`) | `mbv-queue`, `mbv-emby-model`, `mbv-ids` |
| `mbv-config` | `mbv-core/src/config/` (minus the items above and below) | `mbv-keybinds`, `mbv-queue` |
| `mbv-feed` | TUI `src/app/infra/feed_parse/` + `mbv-core/src/feed_entry_state.rs`; `IdleFeedItem` from `src/app/state/types/feed.rs` | `mbv-config`, `mbv-queue`, `mbv-emby-model`, `mbv-net`, `mbv-text` |

`mbv-text` gains `decode_entities` and `html_to_text` from `api/types.rs` (pure
text helpers used by Emby, Audiobookshelf, and Feeds alike).

Cycle cuts done inside `mbv-core` before any extraction:

- `playback/transition.rs` → `mbv-core/src/player/transition.rs`. It is
  owner-side Player state that builds a `PlayerCommand`, not queue data.
- `player/types.rs` splits: the vocabulary (lines 1–413) stays for `mbv-ctrl`;
  the mpv track parsing/selection tail (`LANGS` onward) becomes
  `player/tracks.rs` and stays in `mbv-core`.
- `config::resolve_library_route` → `remote_player` (it returns a
  `DaemonEndpoint`; config only stores the raw string).
- `config::{commit,repair,replace}_audiobookshelf_candidate` → `audiobookshelf`
  (the provider consumes its own validated candidate; config keeps the
  transactional `persist_*`/`replace_*_setup_and_secret` seams they call).
- `QueueLineage`, `FeedKind`, `ServiceKind`, `QueueState`, `QueueSource` move
  into `playback::queue` so the queue module stops importing config and ctrl.

- **BREAKING (workspace-internal only):** every moved path changes and no
  re-export shim is left behind. The `mbv_core::{playback_queue,
  playback_execution_sequence, playback_transition}` aliases, the `playback`
  module, and the unused `api::Config` re-export are removed.
- `mbv-core`'s `test` feature forwards to `mbv-config/test` (which gates
  `TestStateDirGuard`/`TestTempDir`, used by 55 files).
- The TUI stops calling `feed_parse::tls_agent` for image fetches; those two
  sites call `mbv_net::native_tls_agent` directly.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. Code-organisation refactor with no externally observable behaviour
change, so `skip_specs: true`.

## Impact

- **New:** five `crates/mbv-*/` directories; workspace `members` and
  `default-members` grow by five.
- **Call sites** (files referencing the symbol today): `EmbyItem` 143,
  `TICKS_PER_SECOND` 55, `playback_queue` 128, `QueueSource` 69, `ServiceKind`
  60, `PlayerEvent` 56, `PlayerCommand` 50, `PlayerStatus` 32, `FeedKind` 31,
  `QueueLineage` 21, `ctrl` paths ~59, `config` paths ~244. All compiler-forced.
- **Visibility widening:** `pub(in crate::app)` / `pub(crate)` items in
  `feed_parse`, `IdleFeedItem`, and any `pub(crate)` config/queue/ctrl item
  reached across the new boundary become `pub`.
- **Docs:** `AGENTS.md` repository map gains the five crates.
- **Conflicts:** `enforce-audiobookshelf-both-shapes` rewrites `QueueItem`;
  land it before the `mbv-queue` extraction group or rebase onto it.
  `decompose-app-god-type` and `prune-tui-test-suite` edit `src/app/`, which
  every group touches only by path rewrites; the `mbv-feed` group also edits
  `src/app/state/types/feed.rs` and `src/app/infra/`.
- **Out of scope:** Tier 3 (`api ↔ audiobookshelf` cycle, provider crates),
  making `QueueItem` provider-agnostic, the `player_owner_state` alias.
- Umbrella: issue #814, Tier 2.
