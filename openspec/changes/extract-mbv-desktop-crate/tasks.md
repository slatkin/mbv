# Tasks

One commit. It must pass the gate:

```
cargo fmt
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

Accept whatever `cargo fmt` reflows. Never add an `allow`/`expect` attribute,
and never add a `pub use` shim.

## 1. Create the crate and move the modules

- [ ] 1.1 Create `crates/mbv-desktop/Cargo.toml`, following the shape of
  `crates/mbv-visualizer/Cargo.toml`, with a one-line `description`. Its
  dependencies are `mbv-ctrl` and `mbv-emby-model` (path dependencies),
  `log.workspace = true`, and the `zbus`, `tokio` and `ksni` lines cut
  verbatim from the root `Cargo.toml` `[dependencies]`. Add the crate to the
  root `[workspace] members` and `default-members`, and add
  `mbv-desktop = { path = "crates/mbv-desktop" }` to the root
  `[dependencies]`. Verify with `cargo check -p mbv-desktop` once 1.2 is done.
- [ ] 1.2 `git mv src/mpris.rs crates/mbv-desktop/src/mpris.rs` and
  `git mv src/tray.rs crates/mbv-desktop/src/tray.rs`. Create `src/lib.rs`
  with `pub mod mpris; pub mod tray;`, and delete `mod mpris;` and
  `mod tray;` from `src/main.rs`. Make `MprisSource` and `MprisHandle` `pub`.
- [ ] 1.3 In the moved `mpris.rs`, move the two constants
  `IMAGE_CACHE_SUFFIX_CARD_PRIMARY` and `IMAGE_CACHE_SUFFIX_ALBUM_CARD` from
  `src/config.rs` into the module, with their `#[cfg(not(test))]` gates and
  doc comments. Delete them from `src/config.rs`.
- [ ] 1.4 Apply design D1. Add `art_path: fn(&str) -> Option<std::path::PathBuf>`
  as the last parameter of `start`, carry it into the `interface` struct and
  the poll thread, and replace both `crate::config::image_disk_cache_path`
  references with it. `make_metadata` takes it as a parameter. Update the doc
  comments that name `crate::config::image_disk_cache_path` to say that the
  caller supplies it.

## 2. Rewire the TUI

- [ ] 2.1 Rewrite `crate::mpris::` to `mbv_desktop::mpris::` and
  `crate::tray::` to `mbv_desktop::tray::` in `src/`. In
  `src/app/state/construct/remote.rs::start_mpris`, pass
  `crate::config::image_disk_cache_path` as the new final argument. Fix the
  stale `mpris::test_handle` sentence in that file's comment: say that tests
  leave `mpris` unset. Verify with
  `rg "crate::(mpris|tray)|src/(mpris|tray)\.rs" src crates` (no hits, apart
  from historical docs).
- [ ] 2.2 Run the gate. Then verify the boundary:
  `cargo tree -p mbv -e normal --depth 1 --prefix none` must list neither
  `zbus`, `tokio` nor `ksni`, and `cargo tree -p mbvd -e normal --prefix none`
  must list neither `zbus`, `ksni` nor `mbv-desktop`. Record both results in
  the commit message.

## 3. Docs

- [ ] 3.1 AGENTS.md: add `* crates/mbv-desktop/ — MPRIS D-Bus server and
  system tray (owns zbus/tokio/ksni).` to the repository map. Change the async
  bullet to "`tokio` is edge-only (`crates/mbv-desktop`, `zbus`)". Update any
  `src/mpris.rs` / `src/tray.rs` path in `CONTEXT.md`, `docs/invariants/` and
  code comments. Verify with `rg "src/(mpris|tray)\.rs" AGENTS.md CONTEXT.md docs/invariants src crates`
  (no hits).
