# Tasks

Each numbered group is one commit and leaves the workspace green on its own.
Groups run in order, because each extraction depends on the crates below it.

**The gate for every group** (group 0 excepted), run from the repo root after
its tasks:

```
cargo fmt
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

- `cargo fmt` reflow is accepted, never reverted.
- No `allow`/`expect` attribute may be added to silence a lint. If a lint
  cannot be fixed at the source, stop and ask.
- No `pub use` re-export may be added in `mbv-core`, the TUI, or `mbvd` for any
  moved path. The old path must not resolve.

**Every new crate's `Cargo.toml`** copies the shape of
`crates/mbv-text/Cargo.toml`:

- `name`, a one-line `description`, and the seven `*.workspace = true`
  package fields;
- `[lints] workspace = true`;
- externals as `<dep>.workspace = true` (or the exact version line copied from
  `crates/mbv-core/Cargo.toml` for non-workspace deps such as `rust_cast`,
  `mdns-sd`, `flume`);
- path deps as `mbv-x = { path = "../mbv-x" }`.

Add the crate to both `[workspace] members` and `default-members` in the root
`Cargo.toml`. When a moved dependency has no user left in `mbv-core`, delete
its line from `crates/mbv-core/Cargo.toml`, then re-run `cargo check`.

**Bulk path rewrites** use `sed -i` (or `ast-grep`) over `src/ crates/` for the
exact prefixes named in each task; `cargo check` output then drives the rest.
Split by hand any grouped import that mixes moved and unmoved names (e.g.
`use mbv_core::{applog, player, remote_player};` in `src/main.rs`,
`use mbv_core::{applog, config, daemon};` in `crates/mbvd/src/main.rs`).

**Test gating** (design D7): when the compiler reports that a
`#[cfg(test)] pub(crate)` item is unreachable from another crate's tests,
change it to `#[cfg(any(test, feature = "test"))] pub`, add `test` to the
owning crate's `[features]`, and enable it in the caller's
`[dev-dependencies]`. For each new crate, `[features] test` forwards to the
`test` features of the lower crates whose test items it re-exposes or uses
outside `#[cfg(test)]` (e.g. `mbv-net/test`).

**Naming pitfalls:**

- The module `api` becomes the crate `mbv-emby` (`mbv_emby::`), not `mbv-api`.
- `crates/mbv-core/src/daemon/audiobookshelf.rs` and
  `crates/mbv-core/src/daemon/ws.rs` are daemon files. They move in group 7,
  not with `audiobookshelf` (group 2) or `mbv-ws`.
- `mbv_emby_model` (Tier 2, DTOs) ≠ `mbv_emby` (this change, the client).
- `player/types.rs` no longer exists after Tier 2; the Player vocabulary is
  `mbv_ctrl::player::`. Only `player::` paths to items still in
  `crates/mbv-core/src/player/` change in group 6.
- `service_runtime` stays in `mbv-core`. Only `EmbyFailure`,
  `EmbyFailureClass`, and `EmbyBootstrap` leave it (group 1).

## 0. Groundwork

- [ ] 0.1 Confirm Tier 2 has merged. Run `openspec list` and
  `git log --oneline -20`. Verify with all of:
  - `ls crates` shows `mbv-emby-model`, `mbv-queue`, `mbv-ctrl`, `mbv-config`,
    and `mbv-feed`;
  - `rg 'pub mod (config|ctrl|playback)' crates/mbv-core/src/lib.rs` is empty;
  - `crates/mbv-core/src/player/transition.rs` and
    `crates/mbv-core/src/player/tracks.rs` exist;
  - `rg 'crate::(api|player)' crates/mbv-core/src/audiobookshelf crates/mbv-core/src/audiobookshelf.rs crates/mbv-core/src/remote_player crates/mbv-core/src/remote_player.rs`
    shows no `player` hits and no production `api` hits except `EmbyClient`
    in `remote_player.rs`.

  If any check fails, stop and report which one.
- [ ] 0.2 Re-run `openspec list`. If `decompose-app-god-type` or
  `prune-tui-test-suite` is still in progress, rebase onto the latest `main`
  before each group (their `src/app/` edits collide only with path
  rewrites). Verify: `git status` is clean and the branch is based on the
  current `main`.

## 1. Cycle cuts inside `mbv-core`

- [ ] 1.1 Move the `audiobookshelf_response` helper and the two tests that use
  it (`audiobookshelf_me_http_boundary_uses_bearer_and_redacts_failures`,
  `dead_audiobookshelf_endpoint_is_connectivity`) from
  `crates/mbv-core/src/api/tests/failure.rs` into a new
  `crates/mbv-core/src/audiobookshelf/tests/failure.rs`, declared in
  `audiobookshelf/tests.rs`. Match the imports of the sibling
  `audiobookshelf/tests/*.rs` files. The
  Emby-only tests and the `emby_client` helper stay put. Verify:
  `rg 'audiobookshelf' crates/mbv-core/src/api` is empty and
  `cargo nextest run -p mbv-core failure` runs the same number of tests as
  before.
- [ ] 1.2 Move `EmbyFailureClass`, `EmbyFailure` (with its three `impl`
  blocks), and `EmbyBootstrap` from `crates/mbv-core/src/service_runtime.rs`
  into a new `crates/mbv-core/src/api/failure.rs`, declared
  `mod failure; pub use failure::*;` in `api.rs`. Rewrite
  `crate::service_runtime::{EmbyFailure,EmbyFailureClass,EmbyBootstrap}` and
  `mbv_core::service_runtime::{…}` for those three names to `crate::api::` /
  `mbv_core::api::` (~8 files). Verify: gate passes;
  `rg 'service_runtime' crates/mbv-core/src/api` is empty;
  `rg 'EmbyFailure|EmbyBootstrap' crates/mbv-core/src/service_runtime.rs`
  shows only the `use crate::api::…` import (if the compiler needs it).
- [ ] 1.3 Delete the orphaned `crates/mbv-core/src/id_types.rs`. Verify:
  `rg 'id_types' crates/mbv-core` is empty and `cargo check -p mbv-core`
  succeeds.

## 2. `mbv-audiobookshelf`

- [ ] 2.1 Create `crates/mbv-audiobookshelf`. Deps: `mbv-config`, `mbv-queue`,
  `mbv-emby-model`, `mbv-text`, `mbv-net`, `ureq`, `serde`, `serde_json`,
  `tungstenite`, `log`, plus others only if the compiler asks. Dev-deps:
  `rstest`, `mbv-net` with `test`, `mbv-config` with `test`. `git mv` the
  following:
  - `crates/mbv-core/src/audiobookshelf.rs` → `src/lib.rs`;
  - `crates/mbv-core/src/audiobookshelf/*` → `src/`.

  Inside the crate, rewrite `crate::audiobookshelf::` → `crate::`. Verify:
  `cargo nextest run -p mbv-audiobookshelf` passes and
  `rg 'mbv_core|crate::(api|cast|player|daemon|service_runtime)' crates/mbv-audiobookshelf`
  is empty.
- [ ] 2.2 Delete `pub mod audiobookshelf;` from `mbv-core/src/lib.rs`. Add
  `mbv-audiobookshelf` to `[dependencies]` in `mbv-core`, the TUI, and `mbvd`.
  Rewrite `crate::audiobookshelf::` (in `mbv-core`) and
  `mbv_core::audiobookshelf::` (TUI, `mbvd`) → `mbv_audiobookshelf::`. Verify:
  gate passes; `rg 'mbv_core::audiobookshelf|crate::audiobookshelf' src crates`
  is empty.

## 3. `mbv-cast`

- [ ] 3.1 Create `crates/mbv-cast`. Deps: `mbv-audiobookshelf`, `mbv-queue`,
  `serde_json`, `rust_cast`, `mdns-sd`, `log`, plus others only if the
  compiler asks. Dev-deps: `flume` (copy the comment from `mbv-core`'s
  manifest), `rstest`. `git mv` the following:
  - `crates/mbv-core/src/cast.rs` → `src/lib.rs`;
  - `crates/mbv-core/src/cast/*` → `src/`.

  Rewrite `crate::cast::` → `crate::` and `crate::audiobookshelf::` →
  `mbv_audiobookshelf::`. Verify: `cargo nextest run -p mbv-cast` passes.
- [ ] 3.2 Delete `pub mod cast;` from `mbv-core/src/lib.rs`. Add `mbv-cast` to
  the `[dependencies]` of `mbv-core` and the TUI. Rewrite `crate::cast::` and
  `mbv_core::cast::` → `mbv_cast::`. Remove `rust_cast`, `mdns-sd`, and
  `flume` from `crates/mbv-core/Cargo.toml` if `cargo check` passes without
  them. Verify: gate passes; `rg 'mbv_core::cast|crate::cast' src crates` is
  empty.

## 4. `mbv-emby`

- [ ] 4.1 Create `crates/mbv-emby`. Deps: `mbv-cast`, `mbv-config`,
  `mbv-ctrl`, `mbv-emby-model`, `mbv-text`, `mbv-net`, `mbv-ws`, `mbv-ids`,
  `ureq`, `serde`, `serde_json`, `uuid`, `rand`, `log`, plus others only if
  the compiler asks. Dev-deps: `rstest`, `mbv-net` with `test`, `mbv-config`
  with `test`. `git mv` the following:
  - `crates/mbv-core/src/api.rs` → `src/lib.rs`;
  - `crates/mbv-core/src/api/*` → `src/`.

  Rewrite `crate::api::` → `crate::` and `crate::cast::` → `mbv_cast::`.
  Keep `with_test_agent` and any other test-gated item gated as it is today;
  add a `test` feature per the test-gating rule if the TUI or a later crate
  calls one. Verify: `cargo nextest run -p mbv-emby` passes and
  `rg 'mbv_core|crate::(audiobookshelf|player|remote_player|daemon|service_runtime)' crates/mbv-emby`
  is empty.
- [ ] 4.2 Delete `pub mod api;` from `mbv-core/src/lib.rs`. Add `mbv-emby` to
  `[dependencies]` in `mbv-core`, the TUI, and `mbvd`. Rewrite
  `crate::api::` (in `mbv-core`) and `mbv_core::api::` (~123 TUI files,
  `mbvd`) → `mbv_emby::`. Fix any `use mbv_core::api;` followed by `api::X`
  short forms that the compiler reports. Verify: gate passes;
  `rg 'mbv_core::api|crate::api\b' src crates` is empty.

## 5. `mbv-remote-player`

- [ ] 5.1 Create `crates/mbv-remote-player`. Deps: `mbv-ctrl`, `mbv-config`,
  `mbv-queue`, `mbv-emby`, `mbv-emby-model`, `mbv-net`, `serde_json`, `log`,
  plus others only if the compiler asks. `[features] test = []` for the
  existing `#[cfg(any(test, feature = "test"))]` items (`stub`,
  `stub_with_command_rx`, the `connect.rs` test block). `git mv` the following:
  - `crates/mbv-core/src/remote_player.rs` → `src/lib.rs`;
  - `crates/mbv-core/src/remote_player/*` → `src/`.

  Rewrite `crate::remote_player::` → `crate::`. Verify:
  `cargo nextest run -p mbv-remote-player` passes and
  `rg 'mbv_core|crate::(player|daemon|service_runtime)' crates/mbv-remote-player`
  is empty.
- [ ] 5.2 Delete `pub mod remote_player;` from `mbv-core/src/lib.rs`. Add
  `mbv-remote-player` to the `[dependencies]` of `mbv-core` and the TUI. Add
  it with `features = ["test"]` to the `[dev-dependencies]` of the TUI and
  `mbv-core`. Wire `mbv-core`'s `test` feature to
  `"mbv-remote-player/test"` while `player` still lives there. Rewrite
  `crate::remote_player::` and `mbv_core::remote_player::` →
  `mbv_remote_player::`. Verify: gate passes;
  `rg 'mbv_core::remote_player|crate::remote_player' src crates` is empty.

## 6. `mbv-player`

- [ ] 6.1 Create `crates/mbv-player`. Deps: `mbv-remote-player`, `mbv-core`,
  `mbv-emby`, `mbv-emby-model`, `mbv-audiobookshelf`, `mbv-ctrl`,
  `mbv-config`, `mbv-queue`, `mbv-ids`, `mbv-net`, `mbv-ws`, `libmpv2`,
  `libmpv2-sys`, `libc`, `serde_json`, `log`, plus others only if the
  compiler asks. Its `[features] test` covers the gated items in `proxy.rs`
  and `controller.rs` and forwards to `mbv-remote-player/test`. Dev-deps:
  `rstest`, plus `mbv-remote-player`, `mbv-config`, and `mbv-net`, each with
  `test`. `git mv` the following:
  - `crates/mbv-core/src/player.rs` → `src/lib.rs`;
  - `crates/mbv-core/src/player/*` → `src/`.

  Rewrite `crate::player::` → `crate::` and `crate::remote_player::` →
  `mbv_remote_player::`. Verify: `cargo nextest run -p mbv-player` passes and
  `rg 'crate::(daemon|api|audiobookshelf|remote_player|service_runtime)' crates/mbv-player`
  is empty.
- [ ] 6.2 In `mbv-core/src/lib.rs`, delete `pub mod player;` and the whole
  `pub mod player_owner_state { … }` alias block. Add `mbv-player` to
  `[dependencies]` in `mbv-core` (daemon still lives there), the TUI, and
  `mbvd` if the compiler asks. Add it with `features = ["test"]` to the
  `[dev-dependencies]` of the TUI and `mbv-core`. Set `mbv-core`'s `test`
  feature to forward to `mbv-player/test` in place of
  `mbv-remote-player/test`. Rewrite `crate::player::` and
  `mbv_core::player::` → `mbv_player::`, and
  `mbv_core::player_owner_state::` (2 TUI files) →
  `mbv_player::owner_state::`. Promote any `#[cfg(test)] pub(crate)` helper
  that `daemon/tests` calls (e.g. `Player::spy_on_commands`) per the
  test-gating rule. Verify: gate passes;
  `rg 'mbv_core::player|crate::player\b|player_owner_state' src crates` is
  empty.

## 7. `mbv-daemon`

- [ ] 7.1 Create `crates/mbv-daemon`. Deps: `mbv-player`, `mbv-core`,
  `mbv-emby`, `mbv-emby-model`, `mbv-audiobookshelf`, `mbv-ctrl`,
  `mbv-config`, `mbv-queue`, `mbv-ids`, `mbv-net`, `mbv-ws`, `libc`, `uuid`,
  `serde_json`, `log`, plus others only if the compiler asks. Dev-deps:
  `rstest`, plus `mbv-player`, `mbv-config`, and `mbv-net`, each with `test`.
  `git mv` the following:
  - `crates/mbv-core/src/daemon.rs` → `src/lib.rs`;
  - `crates/mbv-core/src/daemon/*` → `src/`.

  Rewrite `crate::daemon::` → `crate::`, and rewrite the
  `pub(in crate::daemon)` visibilities to `pub(crate)`. Verify:
  `cargo nextest run -p mbv-daemon` passes.
- [ ] 7.2 Delete `pub mod daemon;` from `mbv-core/src/lib.rs`. Add
  `mbv-daemon` to the `[dependencies]` of the TUI (`src/local_daemon.rs`) and
  `mbvd`. Rewrite `mbv_core::daemon::` / `mbv_core::{…, daemon}` →
  `mbv_daemon`. Remove `mbv-player` from `mbv-core`'s dependencies. Verify:
  gate passes; `rg 'mbv_core::daemon|crate::daemon' src crates --glob '!crates/mbv-daemon/**'`
  is empty; `cargo tree -p mbv-core --depth 1 -e normal` lists no
  `mbv-player`, `mbv-remote-player`, or `mbv-daemon`.

## 8. `mbv-core` residue and manifests

- [ ] 8.1 Trim `crates/mbv-core`. `src/lib.rs` should declare only `applog`
  and `service_runtime`. Remove every `[dependencies]`/`[dev-dependencies]`
  line that `cargo check -p mbv-core --all-targets` does not need
  (`libmpv2`, `libmpv2-sys`, `tungstenite`, `ureq`, `rand`, `uuid`, `libc`,
  `time`, `mbv-ws`, `mbv-keybinds`, `mbv-cast`, `mbv-remote-player`, and so
  on). Remove its `test` feature if nothing in it is gated on it, and drop
  `features = ["test"]` from the TUI/`mbvd` dev-dependency on `mbv-core` in
  that case. Change `description` to
  "Service runtime state and application logging for mbv.". Verify: gate
  passes; `cargo tree -p mbv-core --depth 1 -e normal` lists only
  `mbv-emby`, `mbv-audiobookshelf`, and the externals `applog` needs.
- [ ] 8.2 Check the crate graph. Verify: `cargo tree -p mbv-audiobookshelf -e normal`
  contains none of `mbv-emby`, `mbv-cast`, `mbv-core`, `mbv-player`, or
  `mbv-daemon`; `cargo tree -p mbv-emby -e normal` contains none of
  `mbv-core`, `mbv-player`, or `mbv-daemon`; `cargo tree -p mbv-player -e normal`
  does not contain `mbv-daemon`.

## 9. Docs and pre-push

- [ ] 9.1 Update `AGENTS.md`'s repository map:
  - add one line each for `mbv-audiobookshelf`, `mbv-cast`, `mbv-emby`,
    `mbv-remote-player`, `mbv-player`, and `mbv-daemon`, in the style of the
    Tier 1 entries;
  - reword the `crates/mbv-core/` line to "Service runtime state + app
    logging";
  - reword the `src/local_daemon.rs` line if it names `mbv-core`.

  Verify: `rg 'mbv-audiobookshelf|mbv-cast|mbv-emby|mbv-remote-player|mbv-player|mbv-daemon' AGENTS.md`
  shows all six.
- [ ] 9.2 Run `make check-code-file-lines` before pushing. Split any file it
  flags along a responsibility seam, following the `splitting-files` skill.
  Verify: the check passes.
- [ ] 9.3 Comment on issue #814. Summarise the Tier 3–4 crates, the resulting
  graph (`mbv-audiobookshelf → mbv-cast → mbv-emby → mbv-core →
  mbv-remote-player → mbv-player → mbv-daemon`), and the two deliberate
  deviations from the issue: `cast` became its own crate rather than joining
  Emby (design D2), and `mbv-daemon` replaced `mbv-daemon-core` (design D5).
  Verify: `gh issue view 814 --comments` shows the comment.
