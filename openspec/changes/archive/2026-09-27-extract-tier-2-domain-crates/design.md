# Design

## Context

Facts that shape the approach, measured on `main` at `922802192`:

- **Queue → providers/config/ctrl/player.** `QueueItem::Emby(Box<EmbyItem>)`
  embeds the Emby DTO from `api/types.rs`; `queue/items.rs` also reads
  `api::{should_resume, TICKS_PER_SECOND}` and `config::{FeedKind,
  ServiceKind}`. `transition.rs` builds a `player::PlayerCommand::JumpTo` and a
  `ctrl::TransitionSummary`. `execution_sequence.rs` is queue-only.
- **ctrl ↔ player types is intrinsic.** `ctrl.rs` wraps `PlayerCommand`,
  `PlayerEvent`, `PlayerStatus`; `player/types.rs` lines 1–413 (`PlayerEvent`,
  `PlayerCommand`) carry `ctrl::{PlaybackGeneration, PlaybackRequestId,
  UnifiedQueueStateData, QueueLoadResult, PlaybackIntentEvent, …}`. That half of
  `player/types.rs` has no `Mpv` use; the `use libmpv2::Mpv` serves only the
  track parsing/selection tail from `const LANGS` (line 416) onward.
- **Config's outward edges are few and misplaced.** Besides the queue types it
  already hosts (`types_queue_state.rs`, pure data — persistence lives in
  `config/state.rs`), config reaches out through exactly:
  `ctrl::QueueLineage` (`state.rs`), `remote_player::DaemonEndpoint`
  (`resolve_library_route` in `types_paths.rs`), and
  `audiobookshelf::{AudiobookshelfValidatedSetup, AudiobookshelfUser}` (three
  thin candidate wrappers in `audiobookshelf_lifecycle.rs`). Two tests follow
  the last two (`tests/settings.rs` route case, `tests/paths.rs` calling
  `AudiobookshelfClient::validate_setup_bounded`).
- **`api/types.rs` is half DTO, half Emby client helpers.** The DTO part
  (`EmbyPerson`, `EmbyLink`, `EmbyArtistRef`, `EmbyItem`, `EmbyImageTags` and
  their impls) plus the tick/resume constants and `should_resume` depend only on
  `serde` and `std`. `decode_entities`/`html_to_text` are pure string helpers
  used by Emby parsing, `audiobookshelf/catalog.rs`, and the TUI feed parser.
  `pub use crate::config::Config` at its top has zero users.
- **Feeds straddle two crates.** `src/app/infra/feed_parse/` (TUI, 388 lines +
  `date.rs`) depends on the TUI only through `IdleFeedItem { title, link }`
  (`src/app/state/types/feed.rs`, used in 2 files). Its `tls_agent` is a
  one-line wrapper over `mbv_net::native_tls_agent` that two image-fetch sites
  also call. `mbv-core/src/feed_entry_state.rs` depends only on
  `config::state_dir` (and `TestStateDirGuard` in tests).
- `config/test_support.rs` (`TestStateDirGuard`, `TestTempDir`) is gated
  `#[cfg(any(test, feature = "test"))]` and used by 55 files across `mbv-core`,
  `mbvd`, and the TUI.

## Goals / Non-Goals

**Goals:**

- The crate graph is acyclic and reads, bottom-up:
  `mbv-emby-model → mbv-queue → {mbv-ctrl, mbv-config} → mbv-feed`, with
  `mbv-core` above all but `mbv-feed`, and the TUI above everything.
- `mbv-queue` and `mbv-ctrl` have no dependency on `mbv-config`; `mbv-config`
  has no dependency on `mbv-ctrl`, any provider, or `mbv-core`.
- Old module paths do not resolve (no re-export shims).
- Every cycle cut is a move of an existing item to the module that owns it; no
  item changes signature or behaviour.

**Non-Goals:**

- Making `QueueItem` provider-agnostic (generic over an item trait or with a
  queue-owned Emby projection). It would remove `mbv-queue → mbv-emby-model`,
  but it is a redesign of the canonical queue, not a split.
- Tier 3: the `api ↔ audiobookshelf` cycle and the provider crates. The rest of
  `api/types.rs` (sessions, device id, token cache) stays in `mbv-core`.
- Splitting `ctrl.rs` (837 lines) — the pre-push line check governs it, and it
  loses `QueueLineage` here.
- Removing the `mbv_core::player_owner_state` alias.

## Decisions

### D1: Invert inside `mbv-core` first, then extract as pure moves

Group 3 performs every cut that relocates an item *within* `mbv-core`
(transition, the `player/types.rs` split, the route and candidate functions, the
dead `api::Config` alias) while everything is still one crate, where a single
`cargo check -p mbv-core` proves it. Items bound for a new crate
(`ServiceKind`, `FeedKind`, `QueueState`, `QueueSource`, `QueueLineage`) move
once, directly into that crate in its extraction group, so their callers are
rewritten once rather than twice. Each extraction group is then the Tier 1
shape: new manifest, `git mv`, path rewrite, delete old `mod` line.
Alternative rejected: cutting each in-core edge in the same commit as an
extraction — mixes a design move with a 100+-file mechanical rewrite.

### D2: `mbv-emby-model` is its own crate

`QueueItem` must name `EmbyItem`, so `EmbyItem` must sit below `mbv-queue`.
Putting the DTO into `mbv-queue` would make the queue crate own an Emby wire
type; putting it in the future Tier 3 `mbv-emby` would make the queue depend on
an HTTP client. A model-only crate states the real edge — the queue knows the
Emby item *shape*, not the Emby *service* — and Tier 3's `mbv-emby` depends on
it too.

The tick unit (`TICKS_PER_SECOND`, `RESUME_THRESHOLD_PERCENT`,
`MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS`, `should_resume`) goes with it,
because `EmbyItem::{resume_seconds, runtime_seconds, should_resume}` use it and
`mbv-emby-model` is the lowest crate that does. Consequence: `mbv-feed` depends
on `mbv-emby-model` for `TICKS_PER_SECOND` alone. Accepted — ticks are mbv's
position unit everywhere, and a crate for one constant is noise.

### D3: `decode_entities` and `html_to_text` go to `mbv-text`

Three providers use them; none owns them. `mbv-text` is already the pure
text-predicate crate, and this keeps `mbv-feed` and (later) the Audiobookshelf
crate from depending on Emby for string cleanup.

### D4: `ServiceKind`, `FeedKind`, `QueueState`, `QueueSource`, `QueueLineage` go to `mbv-queue`

Each is consumed by the queue or ctrl and by config, and `mbv-queue` is the
lowest crate that needs it: `QueueItem::required_service` returns `ServiceKind`,
`FeedEntry::feed_kind` is a `FeedKind`, `QueueState` is `Vec<QueueItem>` plus
source/lineage metadata. `FeedSubscription` stays in config (only `FeedKind`
moves out of `types_feed.rs`); `ServiceKind::secret_name` moves with its enum.
Alternatives rejected: `mbv-ids` (it holds identifier newtypes; widening it
dilutes the one thing it means) and a `mbv-domain` crate (a grab-bag name that
accretes, same reasoning as Tier 1 D2).

### D5: The Player vocabulary lives in `mbv-ctrl`

`ctrl` and `PlayerCommand`/`PlayerEvent`/`PlayerStatus` reference each other,
so they share a crate. The vocabulary (plus `SubtitlePrefs`, `SubtitleChoice`,
`CONNECTION_LOST_MESSAGE`) is exactly what crosses the Player-owner boundary,
which is `mbv-ctrl`'s purpose. Group 3 first splits `player/types.rs` so the
mpv track tail becomes `player/tracks.rs`; the remaining `player/types.rs` then
moves whole into `mbv-ctrl` as `src/player.rs`, reached as
`mbv_ctrl::player::{PlayerCommand, …}` (mirrors today's `player::` path, so the
rewrite is a crate-name swap).

### D6: `transition.rs` stays in `mbv-core` as `player::transition`

`OwnerTransitionState` is owner-side playback-lifecycle state whose only outputs
are a `PlayerCommand` and a `TransitionSummary`. It is not queue data (#814 lists
it under `mbv-queue`; that would force `mbv-queue → mbv-ctrl` and a cycle), and
it is not wire protocol. `player::transition` is where Tier 4's `mbv-player`
will take it. With transition gone, the `playback` module is empty and is
deleted along with the three `playback_*` aliases in `lib.rs`.

### D7: Config's outward calls move to their owners

- `resolve_library_route` → `remote_player` (next to `DaemonEndpoint::parse`,
  which it wraps); its test case moves with it.
- `commit_audiobookshelf_candidate`, `repair_audiobookshelf_candidate`,
  `replace_audiobookshelf_candidate` → `audiobookshelf` (they destructure the
  provider's validated candidate and call config's
  `persist_audiobookshelf_setup_and_secret` /
  `replace_audiobookshelf_setup_and_secret`, both of which stay). The
  `tests/paths.rs` case that calls `AudiobookshelfClient::validate_setup_bounded`
  moves to the audiobookshelf test module.

This is #814's "providers own their config sections" direction applied only as
far as needed to make config a leaf over `mbv-keybinds` + `mbv-queue`.

### D8: `mbv-feed` layout and TUI edges

`feed_parse.rs` becomes `crates/mbv-feed/src/lib.rs` (with `date.rs`, `tests.rs`);
`feed_entry_state.rs` becomes `src/entry_state.rs` with `pub use
entry_state::*`, so callers write `mbv_feed::FeedEntryStore`. `IdleFeedItem`
moves into `mbv-feed` unchanged (its fields become `pub`). `tls_agent` becomes
private to `mbv-feed`; the two image-fetch call sites in `src/app/infra/images/`
call `mbv_net::native_tls_agent(None, …)` directly — identical behaviour, since
that is the wrapper's whole body. `time` moves from the TUI's dependencies to
`mbv-feed` (and `[workspace.dependencies]`) if nothing else in the TUI uses it.

### D9: Test-support gating follows Tier 1 D3

`mbv-config` gets `[features] test = []` and keeps `test_support` gated as today.
`mbv-core`'s `test` feature becomes `["mbv-net/test", "mbv-config/test"]`; `mbv`
and `mbvd` dev-dependencies add `mbv-config = { …, features = ["test"] }` where
they name `TestStateDirGuard`/`TestTempDir` directly; `mbv-feed`'s own tests use
`mbv-config` with `test` via its dev-dependency.

### D10: One change, commit-sized groups

Same reasoning as Tier 1 D4: shared shape, one manifest edit site, one gate. Each
group leaves the workspace green and can ship as its own PR.

## Risks / Trade-offs

- **Rewrite volume.** `crate::config::`/`mbv_core::config::` alone is ~950
  references in ~244 files. → Bulk path rewrites are done with `sed`/`ast-grep`
  over the listed prefixes, then fixed up by `cargo check` output; grouped
  imports (`use crate::api::{EmbyItem, SessionInfo}`) split by hand where only
  some names moved. The compiler is the completeness check; nothing is counted
  by grep.
- **Orphan-rule and inherent-impl breakage.** An `impl Foo { … }` for a moved
  type that lives in a file that stays behind (e.g. a `QueueItem` method in
  `mbv-core`) stops compiling across a crate boundary. → Such impls move with
  the type if they use only lower crates; otherwise they become free functions
  or an extension trait in `mbv-core`. Each case is listed in the group's
  commit message. Stop and ask if one needs more than that.
- **`pub(crate)` widening.** Items reached across the new boundaries become
  `pub` (same trade as Tier 1 D5). Newly-`pub` items can trip
  `missing_panics_doc`/`must_use_candidate`; fix at source, no `allow`.
- **`enforce-audiobookshelf-both-shapes` rewrites `QueueItem`.** → Group 4
  (`mbv-queue`) runs after that change lands, or on a rebase over it.
- **In-flight `src/app/` changes.** `decompose-app-god-type` and
  `prune-tui-test-suite` edit files every group path-rewrites. Rewrites are
  mechanical and rebase cleanly; group 7 (`mbv-feed`) edits
  `state/types/feed.rs` and `infra/`, so it re-checks `openspec list` first.
- **Build-time.** Five more crates add graph overhead; the win is incremental
  rebuilds (an edit to the `mbv-core` runtime no longer recompiles
  config/queue/ctrl) and compiler-enforced boundaries. Not gated on a
  benchmark.

## Migration Plan

Nothing persisted or on the wire changes (`serde` derives move with their types
unchanged; the ctrl protocol version stays 11). Rollback per group is
`git revert` of that group's commit.
