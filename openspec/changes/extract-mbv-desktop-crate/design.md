# Design

## Context

- `src/mpris.rs`: the D-Bus server machinery is under `#[cfg(not(test))]`, so
  that no test process claims `org.mpris.MediaPlayer2.mbv` (issue #757). Its
  public surface is `start`, `rebind`, `MprisHandle` and `MprisSource` (the
  last two are currently `pub(crate)`). The TUI references the art lookup
  `crate::config::image_disk_cache_path` in two places: `make_metadata`
  (called from the `interface` impl) and the poll thread inside `start`. The
  pure decision `resolve_art_url(item, album, resolve_path)` already takes the
  lookup as a parameter.
- `src/tray.rs`: the public surface is `spawn`. It has one caller,
  `local_daemon.rs`, and its unit tests use `#[cfg(test)]` helper methods.
- Every TUI call to `mpris::start` is already `#[cfg(not(test))]`
  (`state/construct/remote.rs::start_mpris`).
- The comment in `construct/remote.rs` mentions `mpris::test_handle`, which no
  longer exists.

## Goals / Non-Goals

**Goals:**

- `cargo tree -p mbv -e normal --depth 1` lists neither `zbus`, `tokio` nor
  `ksni`. `cargo tree -p mbvd -e normal` lists neither `zbus` nor
  `mbv-desktop`.
- `src/mpris.rs` and `src/tray.rs` no longer exist, and there are no
  re-export shims.

**Non-Goals:**

- Fixing the MPRIS art cache-key mismatch (see proposal Impact).
- Moving the image disk-cache helpers out of `src/config.rs`.
- Changing any `#[cfg(not(test))]` gating, or the existing
  `#[expect(clippy::cast_precision_loss)]` on `us_to_seconds`. It moves
  unchanged, and no new suppression is added.

## Decisions

### D1: Inject the art-path lookup instead of moving the cache helpers

`start` gains a final parameter, `art_path: fn(&str) -> Option<PathBuf>`.
`MprisSource` does not store it, because it isn't swapped on `rebind`. The
server `interface` struct and the poll thread each take a copy (`fn` pointers
are `Copy`), and they pass it where `crate::config::image_disk_cache_path`
appears today. `make_metadata` takes it as a parameter. The TUI passes
`crate::config::image_disk_cache_path`.

Alternative rejected: moving `image_disk_cache_*` into `mbv-images`. That would
add `mbv-config` to `mbv-images` (for `cache_dir()`), and it moves eight
functions to serve one caller. The lookup is already a parameter at the point
where the decision is made.

### D2: The crate mirrors the modules; the manifest owns the heavy dependencies

The layout is `crates/mbv-desktop/src/lib.rs` (`pub mod mpris; pub mod tray;`),
`src/mpris.rs` and `src/tray.rs`, moved with `git mv`. The manifest follows
the shape of `crates/mbv-visualizer/Cargo.toml`. The `zbus`, `tokio` and
`ksni` lines move verbatim from the root `[dependencies]`, with their feature
lists unchanged. `MprisHandle` and `MprisSource` become `pub` because the TUI
names `MprisHandle` in `App`. Everything else keeps its current visibility,
with `pub(crate)` read relative to the new crate.

Keeping `#[cfg(not(test))]` in the crate works for both test builds. When the
crate's own tests run, the D-Bus machinery is compiled out, as it is today.
When the TUI's tests run, the crate compiles normally, but the TUI calls
`start` only from `#[cfg(not(test))]` code.

Alternative rejected: separate `mbv-mpris` and `mbv-tray` crates. They share
the desktop-integration dependency set, and a crate of 230 lines doesn't
justify a second manifest.

## Risks / Trade-offs

- [A `pub` item that isn't `#[cfg(not(test))]` and isn't used by the crate's
  own tests trips nothing, because a `pub` item in a library is never
  `dead_code`] → no action needed.
- [An item inside the crate that is only used from `#[cfg(not(test))]` code
  becomes dead in the crate's test build] → the file already gates these
  items, so moving the file unchanged keeps them correct. If clippy reports
  one, gate it the way its neighbours are gated. Never add `allow`/`expect`.

## Migration Plan

This is a pure refactor with a single commit, and it can be reverted.
