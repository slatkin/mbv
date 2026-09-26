# Tasks

Each numbered group is one commit and leaves the workspace green on its own.
Groups run in order: each extraction depends on the crate below it.

**The gate for every group**, run from the repo root after its tasks:

```
cargo fmt
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

`cargo fmt` reflow is accepted, never reverted. No `allow`/`expect` attribute may
be added to silence a lint — if one cannot be fixed at the source, stop and ask.
No `pub use` re-export may be added in `mbv-core` or the TUI for any moved path;
the old path must not resolve.

**Every new crate's `Cargo.toml`** copies the shape of `crates/mbv-text/Cargo.toml`:
`name`, a one-line `description`, the seven `*.workspace = true` package fields,
`[lints] workspace = true`, externals as `<dep>.workspace = true`, and
path deps as `mbv-x = { path = "../mbv-x" }`. The crate is added to both
`[workspace] members` and `default-members` in the root `Cargo.toml`.

**Bulk path rewrites** use `sed -i` (or `ast-grep`) over `src/ crates/` for the
exact prefixes named in each task, then `cargo check` output drives the rest.
Grouped imports that mix moved and unmoved names (e.g.
`use crate::api::{EmbyItem, SessionInfo};`) are split by hand.

**Naming pitfalls:**

- `crates/mbv-core/src/config/` (app config) ≠ `crates/mbv-keybinds/src/config.rs`
  (keybind compiler). Only the former moves in group 6.
- `mbv_core::api` keeps `SessionInfo`, `PlaybackInfo`, `EmbyClient`,
  `parse_item`, `device_id`, the token-cache functions. Only the names listed in
  group 2 move.
- `player/types.rs` keeps its name through group 3; in group 5 the whole
  remaining file moves to `crates/mbv-ctrl/src/player.rs`.
  `player/tracks.rs` (created in group 3) stays in `mbv-core`.
- `config/types_feed.rs` holds both `FeedKind` (moves in group 4) and
  `FeedSubscription` (stays in config).
- `config/types_setup.rs` holds `ServiceKind` (moves in group 4) and the setup
  structs (stay).

## 0. Groundwork

- [ ] 0.1 Check sequencing: `enforce-audiobookshelf-both-shapes` must be merged
  to `main` before group 4 (it rewrites `QueueItem`). Run `openspec list` and
  `git log --oneline -20`. Verify: `rg 'AudiobookshelfBook\(' crates/mbv-core/src/playback/queue/items.rs`
  returns nothing (the variant is gone), or else stop before group 4 and ask.

## 1. `mbv-text` gains the HTML/entity helpers

- [ ] 1.1 Move `decode_entities`, `html_to_text` and their private helpers
  `is_block_tag`, `trim_blank_lines`, `extract_href` (the block from the
  `/// Decode common XML/HTML entities` doc comment through `extract_href`,
  `crates/mbv-core/src/api/types.rs` ~lines 37–163) into a new
  `crates/mbv-text/src/html.rs`, declared `pub mod html;` in
  `crates/mbv-text/src/lib.rs`. Move any `api/types.rs` inline test or
  `api/tests/parsing.rs` test whose subject is one of these two functions into
  `crates/mbv-text/src/html/tests.rs`. Verify: `cargo nextest run -p mbv-text`
  passes.
- [ ] 1.2 Rewrite callers (`api/types_parsing.rs`, `audiobookshelf/catalog.rs`,
  `src/app/infra/feed_parse.rs`, and any the compiler names) to
  `mbv_text::html::{decode_entities, html_to_text}`; add `mbv-text` to
  `mbv-core`'s `[dependencies]`. Verify: gate passes and
  `rg 'fn decode_entities' crates/mbv-core` is empty.

## 2. `mbv-emby-model`

- [ ] 2.1 Create `crates/mbv-emby-model` (deps: `serde`, `mbv-ids` only if a
  moved item names an id type — otherwise omit). Move from
  `crates/mbv-core/src/api/types.rs` into its `src/lib.rs`: `TICKS_PER_SECOND`,
  `RESUME_THRESHOLD_PERCENT`, `MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS`,
  `should_resume` (lines 1–35 minus `pub use crate::config::Config`), and
  `EmbyPerson`, `EmbyLink`, `EmbyArtistRef`, `EmbyItem`, `EmbyImageTags` with
  every `impl` block for them (from the `Task 5.3d` comment above `EmbyPerson`
  to the end of `impl EmbyItem`, before `SessionInfo`). Inline tests in
  `api/types.rs` and cases in `api/tests/` whose subject is a moved item move to
  `crates/mbv-emby-model/src/tests.rs`. Verify: `cargo nextest run -p mbv-emby-model`
  passes.
- [ ] 2.2 Add `mbv-emby-model` to `mbv-core` and the TUI `[dependencies]` and
  rewrite every reference to the nine moved names from
  `crate::api::` / `mbv_core::api::` / `super::types::` to `mbv_emby_model::`
  (~143 files for `EmbyItem`, ~55 for `TICKS_PER_SECOND`). Verify: gate passes;
  `rg 'pub struct EmbyItem|TICKS_PER_SECOND: i64' crates/mbv-core` is empty.

## 3. Cycle cuts inside `mbv-core`

- [ ] 3.1 `git mv crates/mbv-core/src/playback/transition.rs
  crates/mbv-core/src/player/transition.rs`; declare `pub mod transition;` in
  `player.rs` (no `pub use`); remove it from `playback.rs` and delete the
  `pub use playback::transition as playback_transition;` line in `lib.rs`.
  Rewrite `playback_transition::` and `playback::{…Transition…}` references
  (~22 files) to `player::transition::`. Verify: `cargo check -p mbv-core --all-targets`
  and `cargo check -p mbv --all-targets` succeed.
- [ ] 3.2 Split `crates/mbv-core/src/player/types.rs` at `const LANGS` (~line
  416): everything from `LANGS` to end of file (track parsing/selection,
  `lang_code_to_name`, `refresh_tracks`, their helpers and any tests of them)
  moves to a new `player/tracks.rs`, declared `mod tracks;` in `player.rs`, with
  its `pub(super)`/`pub(in crate::player)` visibilities unchanged. Remove
  `use libmpv2::Mpv;` and `use std::sync::{Arc, Mutex};` from `types.rs` if
  unused. Rewrite `types::{parse_tracks, …}` imports in `player/` to `tracks::`.
  Verify: `rg 'Mpv' crates/mbv-core/src/player/types.rs` shows only the
  `MpvQuit` variant; `cargo check -p mbv-core --all-targets` succeeds.
- [ ] 3.3 Move `resolve_library_route` (and its doc comment) from
  `config/types_paths.rs` to `remote_player/connect/endpoint.rs`, exported where
  `DaemonEndpoint` is (`pub use connect::{DaemonEndpoint, resolve_library_route}`
  in `remote_player.rs`). Move its test case from `config/tests/settings.rs` to
  `remote_player/connect/tests.rs`. Rewrite its callers (~4 files) to
  `remote_player::resolve_library_route`. Verify: `rg 'remote_player'
  crates/mbv-core/src/config` is empty; `cargo check -p mbv-core -p mbv
  --all-targets` succeeds.
- [ ] 3.4 Move `commit_audiobookshelf_candidate`,
  `repair_audiobookshelf_candidate`, `replace_audiobookshelf_candidate` from
  `config/audiobookshelf_lifecycle.rs` into `audiobookshelf.rs` (bodies
  unchanged; they now call `crate::config::persist_audiobookshelf_setup_and_secret`
  / `crate::config::replace_audiobookshelf_setup_and_secret`, which stay). Move
  the `config/tests/paths.rs` case that calls
  `AudiobookshelfClient::validate_setup_bounded` into `audiobookshelf/tests.rs`
  (or its `tests/` children). Rewrite callers (~2 files) to
  `audiobookshelf::…`. Verify: `rg 'crate::audiobookshelf'
  crates/mbv-core/src/config` is empty; `cargo check -p mbv-core -p mbv
  --all-targets` succeeds.
- [ ] 3.5 Delete `pub use crate::config::Config;` from `api/types.rs` (zero
  users). Verify: gate passes and `rg 'crate::(remote_player|audiobookshelf|api)'
  crates/mbv-core/src/config` is empty.

## 4. `mbv-queue`

- [ ] 4.1 Create `crates/mbv-queue` (deps: `mbv-emby-model`, `mbv-ids`,
  `serde`, `serde_json`/`log` only if the moved code uses them; dev-dep
  `rstest`). `git mv` `playback/queue.rs` → `src/lib.rs`,
  `playback/queue/items.rs` → `src/items.rs`,
  `playback/execution_sequence.rs` → `src/execution_sequence.rs`
  (`mod execution_sequence; pub use execution_sequence::*;` in `lib.rs`),
  `playback/tests.rs` + `playback/tests/` → `src/tests.rs` + `src/tests/`.
  Verify: the files exist under `crates/mbv-queue/src/` and not under
  `crates/mbv-core/src/playback/` except `playback.rs` (removed in 4.3).
- [ ] 4.2 Into the same crate move, unchanged: `FeedKind` + its `impl` (from
  `config/types_feed.rs`) → `src/kinds.rs`; `ServiceKind` + its `impl` (from
  `config/types_setup.rs`) → `src/kinds.rs`; all of
  `config/types_queue_state.rs` (`QueueSource`, `QueueState`, impls) →
  `src/state.rs` (delete the file and its `mod`/`pub use` lines in
  `config.rs`); `QueueLineage` (from `ctrl.rs`, with its derives/impls) →
  `src/state.rs`. `lib.rs` gets `mod kinds; pub use kinds::*; mod state; pub use state::*;`.
  Verify: `cargo nextest run -p mbv-queue` passes.
- [ ] 4.3 Delete `crates/mbv-core/src/playback.rs`, the empty `playback/`
  directory, and the `pub mod playback;` / `playback_queue` /
  `playback_execution_sequence` lines in `lib.rs`. Add `mbv-queue` to
  `mbv-core`, TUI and (if the compiler asks) `mbvd` `[dependencies]`. Rewrite
  `crate::playback_queue::`, `mbv_core::playback_queue::`,
  `crate::playback_execution_sequence::`, `mbv_core::playback_execution_sequence::`,
  and `crate::playback::`/`mbv_core::playback::` (non-transition names) to
  `mbv_queue::`; rewrite `config::{FeedKind, ServiceKind, QueueState,
  QueueSource}` and `ctrl::QueueLineage` references to `mbv_queue::`.
  Verify: gate passes; `rg 'playback_queue|playback_execution_sequence|mod playback\b' src crates`
  is empty; `rg 'crate::(config|ctrl|api|player)' crates/mbv-queue` is empty.

## 5. `mbv-ctrl`

- [ ] 5.1 Create `crates/mbv-ctrl` (deps: `mbv-queue`, `mbv-emby-model`,
  `mbv-ids`, `serde`, others only as the compiler asks; dev-deps `rstest`,
  `serde_json` if tests use it). `git mv crates/mbv-core/src/ctrl.rs
  crates/mbv-ctrl/src/lib.rs`, `ctrl/tests.rs` + `ctrl/tests/` →
  `src/tests.rs` + `src/tests/`, and `crates/mbv-core/src/player/types.rs` →
  `crates/mbv-ctrl/src/player.rs` (`pub mod player;` in `lib.rs`). Inside the
  new crate `crate::player::` paths stay as written;
  rewrite `crate::ctrl::` → `crate::`, queue/emby-model paths to their crates.
  Verify: `cargo check -p mbv-ctrl` succeeds.
- [ ] 5.2 Split the test `local_only_command_is_refused_without_delivery_or_termination`
  in `tests/tests_wire.rs`: the `WireCommand::try_from_player_command` assertion
  stays in `mbv-ctrl`; the `RemotePlayer::stub_with_command_rx` send-path half
  becomes its own test in `mbv-core`'s `remote_player` test module, keeping the
  comment that explains it. Replace `ServiceKind`/`FeedEntry`/`EmbyItem` test
  paths with the new crates. Verify: `cargo nextest run -p mbv-ctrl` passes.
- [ ] 5.3 Delete `pub mod ctrl;` from `mbv-core/src/lib.rs` and `mod types;
  pub use types::*;` from `player.rs`. Add `mbv-ctrl` to `mbv-core`, TUI and
  `mbvd` `[dependencies]` as needed. Rewrite `crate::ctrl::` /
  `mbv_core::ctrl::` → `mbv_ctrl::`, and `crate::player::` / `super::` /
  `mbv_core::player::` references to `PlayerCommand`, `PlayerEvent`,
  `PlayerStatus`, `SubtitlePrefs`, `SubtitleChoice`, `CONNECTION_LOST_MESSAGE`
  → `mbv_ctrl::player::`. `player/tracks.rs` imports `PlayerStatus` from
  `mbv_ctrl::player`. Verify: gate passes; `rg 'crate::(config|api|remote_player|daemon)|mbv_core' crates/mbv-ctrl`
  is empty.

## 6. `mbv-config`

- [ ] 6.1 Create `crates/mbv-config` (deps: `mbv-keybinds`, `mbv-queue`,
  `serde`, `toml`, `log`, others only as the compiler asks; `[features] test = []`;
  dev-deps `rstest` and whatever `config/tests/` uses). `git mv
  crates/mbv-core/src/config.rs crates/mbv-config/src/lib.rs` and
  `crates/mbv-core/src/config/*` → `crates/mbv-config/src/`. Inside the crate
  rewrite `crate::config::` → `crate::`. Keep `test_support` gated
  `#[cfg(any(test, feature = "test"))]` exactly as today. Verify:
  `cargo nextest run -p mbv-config` passes and `rg 'mbv_core|crate::(ctrl|api|player|remote_player|audiobookshelf)' crates/mbv-config`
  is empty.
- [ ] 6.2 Delete `pub mod config;` from `mbv-core/src/lib.rs`. Add `mbv-config`
  to `mbv-core`, `mbvd` and TUI `[dependencies]`; set `mbv-core`'s feature to
  `test = ["mbv-net/test", "mbv-config/test"]`; add
  `mbv-config = { path = …, features = ["test"] }` to the TUI and `mbvd`
  `[dev-dependencies]`. Bulk-rewrite `crate::config::` (inside `mbv-core`) and
  `mbv_core::config::` (TUI, `mbvd`) → `mbv_config::`; fix `use crate::config;`
  / `config::X` short forms the compiler reports. Drop any `mbv-core`
  dependency (e.g. `toml`, `mbv-keybinds`) that `cargo check` shows unused
  after the move — remove the line and re-check. Verify: gate passes;
  `rg 'mbv_core::config|crate::config' src crates` is empty (the TUI's own
  `src/config.rs` module, if referenced as `crate::config`, is the one
  exception — confirm it is the TUI's file, not a stale path).

## 7. `mbv-feed`

- [ ] 7.1 Re-run `openspec list`; if `decompose-app-god-type` or
  `prune-tui-test-suite` has unchecked tasks touching
  `src/app/state/types/feed.rs` or `src/app/infra/`, rebase onto their latest
  commit first. Verify: `git status` clean before starting.
- [ ] 7.2 Create `crates/mbv-feed` (deps: `mbv-config`, `mbv-queue`,
  `mbv-emby-model`, `mbv-net`, `mbv-text`, `ureq`, `time`, `serde`,
  `serde_json`, `log` as the compiler asks; dev-dep `mbv-config` with
  `features = ["test"]`). `git mv src/app/infra/feed_parse.rs` →
  `crates/mbv-feed/src/lib.rs`, `src/app/infra/feed_parse/{date,tests}.rs` →
  `crates/mbv-feed/src/`, `crates/mbv-core/src/feed_entry_state.rs` →
  `crates/mbv-feed/src/entry_state.rs` (`mod entry_state; pub use entry_state::*;`).
  Move `IdleFeedItem` from `src/app/state/types/feed.rs` into `lib.rs` with
  `pub` fields. Widen `pub(in crate::app)` items to `pub`; make `tls_agent`
  private. Verify: `cargo nextest run -p mbv-feed` passes.
- [ ] 7.3 Delete `mod feed_parse;` from `src/app/infra.rs` and `pub mod
  feed_entry_state;` from `mbv-core/src/lib.rs`. Add `mbv-feed` to the TUI
  `[dependencies]`. Rewrite `crate::app::infra::feed_parse::` →
  `mbv_feed::`, `mbv_core::feed_entry_state::` → `mbv_feed::`, and
  `IdleFeedItem` imports → `mbv_feed::IdleFeedItem`. Replace the two
  `feed_parse::tls_agent(…)` calls in `src/app/infra/images/protocol.rs` and
  `src/app/infra/images/fetch/level_warmup.rs` with
  `mbv_net::native_tls_agent(None, …)`. Move `time` to
  `[workspace.dependencies]`; remove it from the TUI and/or `mbv-core` if
  `cargo check` then passes without it. Verify: gate passes;
  `rg 'feed_parse|feed_entry_state' src crates --glob '!crates/mbv-feed/**'`
  returns only method names like `hydrate_feed_entry_state`/field names, no
  module paths.

## 8. Docs and pre-push

- [ ] 8.1 Add the five crates to `AGENTS.md`'s repository map (one line each,
  matching the Tier 1 entries' style) and update the `crates/mbv-core/` line
  so it no longer claims config/protocols/canonical queue. Verify: `rg
  'mbv-queue|mbv-ctrl|mbv-config|mbv-feed|mbv-emby-model' AGENTS.md` shows all
  five.
- [ ] 8.2 Run `make check-code-file-lines` before pushing; split any file it
  flags (e.g. `mbv-ctrl/src/lib.rs`, from the 837-line `ctrl.rs`) along a
  responsibility seam per the `splitting-files` skill. Verify: the check passes.
- [ ] 8.3 Comment on issue #814 summarising the Tier 2 crates and the final
  dependency graph (`mbv-emby-model → mbv-queue → {mbv-ctrl, mbv-config} →
  mbv-feed`), and noting the D2 `mbv-feed → mbv-emby-model` tick edge. Verify:
  `gh issue view 814 --comments` shows the comment.
