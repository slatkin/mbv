# Proposal

## Why

Issue #832, a follow-up to #814. `src/mpris.rs` (639 lines) and `src/tray.rs`
(230 lines) are the only code in the workspace that uses `zbus`, `tokio` or
`ksni`. They don't use `crate::app`: tray uses only `mbv-ctrl`, and MPRIS uses
`mbv-ctrl`, `mbv-emby-model`, `mbv-images` (`emby_card_cache_key`), and one
TUI helper, `crate::config::image_disk_cache_path`.
Because both files live in the TUI crate, that crate carries the three heavy
async/D-Bus dependencies. The AGENTS.md rule "`tokio` is edge-only" is
therefore only a review convention.

## What Changes

- New crate `crates/mbv-desktop/` with modules `mpris` and `tray`, moved
  unchanged from `src/`. It owns the `zbus`, `tokio` and `ksni` dependencies.
- `mbv_desktop::mpris::start` takes the art-path lookup
  (`fn(&str) -> Option<PathBuf>`) as a parameter, so the crate doesn't depend
  on the TUI's image-cache helpers. The TUI passes
  `crate::config::image_disk_cache_path`, the function it uses today.
- The TUI crate's `[dependencies]` no longer list `zbus`, `tokio` or `ksni`.
  Its callers (`local_daemon.rs`, `state/construct/remote.rs`,
  `dispatch/session/{switch,connect,daemon_restart}.rs`, `app_struct.rs`)
  import from `mbv_desktop::`.
- AGENTS.md: a repository-map entry for `crates/mbv-desktop/`, and the async
  bullet now names the crate.
- No change to behaviour, the D-Bus interface, tray menus or config. `mbvd`
  stays free of D-Bus.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

(none). This is a pure code move. No spec names these files or crates, so
`skip_specs: true`.

## Impact

- New crate `mbv-desktop`, which depends on `mbv-ctrl`, `mbv-emby-model`,
  `mbv-images`, `zbus`, `tokio`, `ksni` and `log`.
- Root `Cargo.toml`: new workspace member and dependency, and three direct
  dependencies removed.
- The MPRIS art cache-key mismatch found during discovery was fixed
  separately (#833, commit f21174d67); MPRIS now derives its key from
  `mbv_images::emby_card_cache_key`.
