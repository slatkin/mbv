# Tasks

Each numbered group is one commit and leaves the workspace green on its own.
Group 4 must follow group 3. Groups 5 and 6 must be last (see `design.md` —
Risks: in-flight `src/app/` changes).

**The gate for every group**, run from the repo root after its tasks:

```
cargo fmt
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run -p <new-crate> -p mbv-core -p mbv -p mbvd
```

`cargo fmt` reflow is accepted, never reverted. No `allow`/`expect` attribute may
be added to silence a lint — if one cannot be fixed at the source, stop and ask.

**Naming pitfalls before you start:**

- `crates/mbv-core/src/keybinds/config.rs` (the keybind compiler) is a different
  module from `crates/mbv-core/src/config/` (app config). Group 2 moves only the
  former.
- `mbv_core::config::parse` and `mbv_core::config::save` are the *callers* of
  `keybinds::load`; they stay in `mbv-core`.
- `src/app/infra/visualizer.rs` stays in the `mbv` crate. Only
  `src/app/infra/visualizer_worker.rs` moves.
- Do not add a `pub use` re-export in `mbv-core` or `src/app/infra.rs` for any
  moved path. The old path must not resolve.

## 0. Groundwork

- [x] 0.1 Confirm no other in-flight change is mid-edit in `crates/mbv-core/src/`:
  run `openspec list` and check `decompose-app-god-type` and
  `prune-tui-test-suite` are still confined to `src/app/`. Verify: neither
  change's `tasks.md` lists a `crates/mbv-core/` path with an unchecked box.
- [x] 0.2 Move `fuzzy-matcher = "0.3"` from `[dependencies]` in the root
  `Cargo.toml` to `[workspace.dependencies]`, leaving `fuzzy-matcher.workspace =
  true` in the `mbv` package. Verify: `cargo check -p mbv` succeeds.

## 1. `mbv-ids`

- [x] 1.1 Create `crates/mbv-ids/Cargo.toml`: `name = "mbv-ids"`, a `description`
  ("Type-safe media identifier newtypes for mbv."), all seven
  `*.workspace = true` package fields, `[lints] workspace = true`, and
  `serde.workspace = true`. Add `"crates/mbv-ids"` to `[workspace] members` and
  `default-members`. Verify: `cargo check -p mbv-ids` succeeds on the empty crate.
- [x] 1.2 Move `crates/mbv-core/src/id_types.rs` verbatim to
  `crates/mbv-ids/src/lib.rs`, keeping the file's leading comment block and the
  `string_id!` macro. Verify: `cargo check -p mbv-ids` succeeds and
  `ItemId`, `MediaSourceId`, `EmbySessionId` are `pub`.
- [x] 1.3 Delete `pub mod id_types;` and `pub use id_types::{EmbySessionId,
  ItemId, MediaSourceId};` from `crates/mbv-core/src/lib.rs`; add
  `mbv-ids = { path = "../mbv-ids" }` to `mbv-core`'s `[dependencies]`. Verify:
  `rg 'id_types' crates/ src/` returns only the new crate's own file, if any.
- [x] 1.4 Rewrite the 8 `crate::id_types::…` / `super::{… ItemId …}` import
  sites inside `mbv-core` to `mbv_ids::…`. Edit sites by role: the Emby client
  root and its types/reporting modules, the player root and its types /
  report-worker / run-decisions modules, and the daemon event-loop player-events
  module. Verify: `cargo check -p mbv-core --all-targets` succeeds.
- [x] 1.5 Add `mbv-ids = { path = "crates/mbv-ids" }` to the `mbv` package's
  `[dependencies]` and rewrite its `use mbv_core::{ItemId, …}` /
  `use mbv_core::ItemId` sites (the cast state-types module and the dispatch
  actions module) to `mbv_ids::…`. Verify: `cargo check -p mbv --all-targets`
  succeeds.
- [x] 1.6 Run the group gate for `mbv-ids`.

## 2. `mbv-keybinds`

- [x] 2.1 Create `crates/mbv-keybinds/Cargo.toml`: `name = "mbv-keybinds"`,
  `description` ("Configurable keybinding registry, chord grammar, and
  validation for mbv."), the seven `*.workspace = true` package fields, and
  `[lints] workspace = true`. It needs **no** dependencies — the module is pure
  `std`. Register it in `members` and `default-members`. Verify:
  `cargo check -p mbv-keybinds` succeeds on the empty crate.
- [x] 2.2 Move `crates/mbv-core/src/keybinds.rs` to
  `crates/mbv-keybinds/src/lib.rs` (keep its `//!` doc comment, its three `mod`
  declarations, its three `pub use` blocks, and its `#[cfg(test)] mod tests;`),
  and move `crates/mbv-core/src/keybinds/{chord,config,registry,tests}.rs` to
  `crates/mbv-keybinds/src/`. Verify: `cargo nextest run -p mbv-keybinds` runs
  the moved test module and passes.
- [x] 2.3 Delete `pub mod keybinds;` from `crates/mbv-core/src/lib.rs` and add
  `mbv-keybinds = { path = "../mbv-keybinds" }` to `mbv-core`'s `[dependencies]`.
  Rewrite the `crate::keybinds::…` references in `mbv-core` — the app-config
  parse, save, types-paths and config-test modules — to `mbv_keybinds::…`.
  Verify: `cargo check -p mbv-core --all-targets` succeeds.
- [x] 2.4 Add `mbv-keybinds = { path = "crates/mbv-keybinds" }` to the `mbv`
  package's `[dependencies]` and rewrite its 12+ `use mbv_core::keybinds::…`
  lines to `use mbv_keybinds::…`. Edit sites by role: the three input-routing
  modules and their test-support/resolution/prefix test modules, the shell
  settings module and its tests, the sidebars overlay module, the help component
  and the help painter, the settings state-types module, and the
  tick-integration and routing-matrix test support modules. Verify:
  `rg 'mbv_core::keybinds' src/ crates/` returns nothing.
- [x] 2.5 Run the group gate for `mbv-keybinds`.

## 3. `mbv-net`

- [x] 3.1 Create `crates/mbv-net/Cargo.toml`: `name = "mbv-net"`, `description`
  ("Shared HTTP, TLS-agent, socket and retry primitives for mbv."), the seven
  `*.workspace = true` package fields, `[lints] workspace = true`,
  `[features] test = []`, and `ureq.workspace = true`,
  `percent-encoding.workspace = true`, `rand.workspace = true`,
  `log.workspace = true`. Register it in `members` and `default-members`.
  Verify: `cargo check -p mbv-net` succeeds on the empty crate.
- [x] 3.2 Move `crates/mbv-core/src/{bounded,stream,mock_http}.rs` to
  `crates/mbv-net/src/`, declare them in `crates/mbv-net/src/lib.rs` as
  `pub mod bounded; pub mod stream;` and
  `#[cfg(any(test, feature = "test"))] pub mod mock_http;`, and widen
  `bounded::run_with_hard_bound`, `bounded::run_with_hard_bound_or_cleanup` and
  `stream::SocketStream` (plus its `try_clone` / `shutdown` methods) from
  `pub(crate)` to `pub`. Verify: `cargo nextest run -p mbv-net` passes the
  moved `bounded` tests.
- [x] 3.3 Move `PATH_SEGMENT`, `encode_path_segment`, `reconnect_backoff_sleep`
  and `native_tls_agent` out of `crates/mbv-core/src/lib.rs` into
  `crates/mbv-net/src/lib.rs`, keeping every doc comment and the existing
  `#[expect(clippy::cast_precision_loss, …)]` on the backoff cast verbatim.
  `encode_path_segment` and `reconnect_backoff_sleep` become `pub`. Verify:
  `cargo check -p mbv-net` succeeds and `crates/mbv-core/src/lib.rs` contains no
  function definitions.
- [x] 3.4 Change `mbv-core`'s `[features]` to `test = ["mbv-net/test"]`, add
  `mbv-net = { path = "../mbv-net" }` to its `[dependencies]`, and remove
  `percent-encoding.workspace = true` from it. Verify:
  `rg 'percent_encoding' crates/mbv-core/` returns nothing.
- [x] 3.5 Rewrite every `mbv-core` reference to the moved items:
  `crate::encode_path_segment` → `mbv_net::encode_path_segment` (~30 call sites
  across the Emby client library/auth/reporting modules and the Audiobookshelf
  catalog, catalog-books and playback modules), `crate::bounded::` →
  `mbv_net::bounded::`, `crate::stream::` → `mbv_net::stream::`,
  `crate::reconnect_backoff_sleep` → `mbv_net::reconnect_backoff_sleep` (the
  Audiobookshelf socket module; `ws.rs` is handled in group 4),
  `crate::native_tls_agent` → `mbv_net::native_tls_agent`, and
  `crate::mock_http` → `mbv_net::mock_http` (the Emby auth/test modules, the
  Audiobookshelf root and its playback/catalog tests, the player tests, the
  daemon ws and daemon tests). Verify: `cargo check -p mbv-core --all-targets`
  and `cargo check -p mbvd --all-targets` both succeed.
- [x] 3.6 Add `mbv-net = { path = "crates/mbv-net" }` to the `mbv` package's
  `[dependencies]` (for `native_tls_agent` in the feed-parse module) and
  `mbv-net = { path = "crates/mbv-net", features = ["test"] }` to its
  `[dev-dependencies]`, then rewrite the `mbv_core::native_tls_agent` and
  `mbv_core::mock_http` references in `src/` to `mbv_net::…`. Edit sites by
  role: the feed-parse infra module, the shell test module, and the
  remote-commands / context-actions / library-navigate-reveal /
  tick-integration test modules. Verify:
  `rg 'mbv_core::(mock_http|native_tls_agent)' src/ crates/` returns nothing.
- [x] 3.7 Run the group gate for `mbv-net`, and additionally confirm the feature
  gate still holds: `cargo check -p mbv-core` (no `--all-targets`, no `test`
  feature) succeeds and `cargo tree -p mbv-core -i mock_http` is not needed
  because `mock_http` is a module, not a crate — instead verify
  `cargo check -p mbv --release` succeeds, proving `mock_http` is not reachable
  from a release build.

## 4. `mbv-ws`

- [ ] 4.1 Create `crates/mbv-ws/Cargo.toml`: `name = "mbv-ws"`, `description`
  ("Emby websocket client transport for mbv."), the seven `*.workspace = true`
  package fields, `[lints] workspace = true`, and `tungstenite.workspace = true`,
  `serde_json.workspace = true`, `log.workspace = true`,
  `mbv-net = { path = "../mbv-net" }`. Register it in `members` and
  `default-members`. Verify: `cargo check -p mbv-ws` succeeds on the empty crate.
- [ ] 4.2 Move `crates/mbv-core/src/ws.rs` to `crates/mbv-ws/src/lib.rs` and
  change its one internal call, `crate::reconnect_backoff_sleep(&mut
  backoff_secs, "ws")`, to `mbv_net::reconnect_backoff_sleep(…)` — keep the
  `"ws"` log target string unchanged. Verify: `cargo check -p mbv-ws` succeeds
  and `OutboundMessage`, `WsSender`, `WsEvent` and `start` are `pub`.
- [ ] 4.3 Delete `pub mod ws;` from `crates/mbv-core/src/lib.rs`, add
  `mbv-ws = { path = "../mbv-ws" }` to `mbv-core`'s `[dependencies]`, and
  rewrite its `crate::ws::` references to `mbv_ws::`. Verify:
  `cargo check -p mbv-core --all-targets` succeeds.
- [ ] 4.4 Add `mbv-ws = { path = "crates/mbv-ws" }` to the `mbv` package's
  `[dependencies]` and rewrite every `mbv_core::ws::` reference in `src/` to
  `mbv_ws::`. Verify: `rg 'mbv_core::ws|crate::ws::' src/ crates/` returns
  nothing.
- [ ] 4.5 Run the group gate for `mbv-ws`.

## 5. `mbv-visualizer`

- [ ] 5.1 Re-check for collisions: `openspec list` plus
  `rg 'visualizer_worker|app_struct|state/construct' openspec/changes/*/tasks.md`
  to confirm no unchecked task in another in-flight change edits
  `src/app/state/app_struct.rs` or `src/app/state/construct.rs`. Verify: no
  unchecked box names either file; if one does, stop and report.
- [ ] 5.2 Create `crates/mbv-visualizer/Cargo.toml`: `name = "mbv-visualizer"`,
  `description` ("PipeWire stereo audio capture worker for mbv's visualizer."),
  the seven `*.workspace = true` package fields, `[lints] workspace = true`, and
  `pipewire.workspace = true`, `log.workspace = true`. Register it in `members`
  and `default-members`. Verify: `cargo check -p mbv-visualizer` succeeds on the
  empty crate.
- [ ] 5.3 Move `src/app/infra/visualizer_worker.rs` to
  `crates/mbv-visualizer/src/lib.rs`, widening `StereoSampleBuffer` and
  `join_worker` from `pub(crate)` to `pub`. `StereoSample`,
  `StereoSampleWindow` and `PipeWireWorker` are already `pub`. Verify:
  `cargo nextest run -p mbv-visualizer` succeeds.
- [ ] 5.4 Delete `pub(in crate::app) mod visualizer_worker;` from
  `src/app/infra.rs`, add `mbv-visualizer = { path = "crates/mbv-visualizer" }`
  to the `mbv` package's `[dependencies]`, and remove
  `pipewire.workspace = true` from it. Verify:
  `rg 'pipewire' Cargo.toml` shows it only under `[workspace.dependencies]`.
- [ ] 5.5 Rewrite the 9 remaining `crate::app::infra::visualizer_worker::…`
  references in `src/` to `mbv_visualizer::…`. Edit sites by role: the app-struct
  and construct state modules, the remote-slot state module, the visualizer infra
  module (its `use super::visualizer_worker::PipeWireWorker` becomes a
  `mbv_visualizer` path), the cast dispatch module, the run-loop teardown module,
  the visualizer painter, and the `src/app/tests.rs` fixture. Verify:
  `rg 'visualizer_worker' src/` returns nothing.
- [ ] 5.6 Run the group gate for `mbv-visualizer`, and additionally confirm the
  packaging constraint still holds: `crates/mbvd/Cargo.toml` has no path to
  `mbv-visualizer` and `cargo tree -p mbvd | rg pipewire` returns nothing (CI's
  build.yml asserts "mbvd must not depend on PipeWire runtime packages").

## 6. `mbv-text`

- [ ] 6.1 Create `crates/mbv-text/Cargo.toml`: `name = "mbv-text"`,
  `description` ("Fuzzy-match acceptance and control-character predicates for
  mbv's text input."), the seven `*.workspace = true` package fields,
  `[lints] workspace = true`, and `fuzzy-matcher.workspace = true`. Register it
  in `members` and `default-members`. Verify: `cargo check -p mbv-text` succeeds
  on the empty crate.
- [ ] 6.2 Move `src/app/infra/fuzzy_match.rs` and
  `src/app/infra/text_safety.rs` to `crates/mbv-text/src/`, declare both as
  `pub mod` in `crates/mbv-text/src/lib.rs`, and widen
  `fuzzy_match::word_match_score` and `text_safety::is_control_char` from
  `pub(in crate::app)` to `pub`. Keep `fuzzy_match.rs`'s `//!` doc block — it is
  the record of why word-local matching exists. Verify:
  `cargo nextest run -p mbv-text` runs the moved `fuzzy_match` tests and passes.
- [ ] 6.3 Delete the `fuzzy_match` and `text_safety` `mod` lines from
  `src/app/infra.rs`, add `mbv-text = { path = "crates/mbv-text" }` to the `mbv`
  package's `[dependencies]`, and remove `fuzzy-matcher.workspace = true` from
  it. Verify: `rg 'fuzzy-matcher' Cargo.toml` shows it only under
  `[workspace.dependencies]`.
- [ ] 6.4 Rewrite the 4 remaining references in `src/`:
  `crate::app::infra::fuzzy_match::word_match_score` →
  `mbv_text::fuzzy_match::word_match_score` (the inline-search component and the
  tree-browser operations module) and
  `crate::app::infra::text_safety::is_control_char` →
  `mbv_text::text_safety::is_control_char` (the feed-parse infra module and the
  library-panel overview-box component). Verify:
  `rg 'fuzzy_match|text_safety' src/` returns nothing.
- [ ] 6.5 Run the group gate for `mbv-text`.

## 7. Wrap-up

- [x] 7.1 Update the `AGENTS.md` "Repository map" section: add the six leaf
  crates with a one-line role each, under the existing `crates/` entries.
  Verify: `AGENTS.md` names all six of `mbv-ids`, `mbv-keybinds`, `mbv-net`,
  `mbv-ws`, `mbv-visualizer`, `mbv-text`.
- [ ] 7.2 Confirm no old path survives anywhere:
  `rg 'mbv_core::(ws|keybinds|id_types|mock_http|native_tls_agent|ItemId|MediaSourceId|EmbySessionId)|crate::(bounded|stream|encode_path_segment|reconnect_backoff_sleep)|visualizer_worker|fuzzy_match|text_safety' src/ crates/ docs/ openspec/specs/ AGENTS.md CONTEXT.md`
  returns only hits inside the six new crates' own source. Verify: command output
  contains no path under `src/`, `crates/mbv-core/`, or `crates/mbvd/`.
- [ ] 7.3 Run the full gate one final time plus
  `make check-code-file-lines` and `cargo fmt --all -- --check`. Verify: all
  clean, no governed file over 800 lines.
- [ ] 7.4 Comment on issue #814 recording that Tier 1 is complete, listing the
  six crate names and noting that `mbv-net` and `mbv-ids` now exist for Tier 2.
  Verify: the comment is posted.
