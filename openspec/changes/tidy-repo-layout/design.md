# Design

## Context

See proposal.md for why this change exists. The facts below were verified against the tree before planning:

- **`test-support` users outside `mbv-core`.** Every non-test use of a `test-support` item from `mbv` is already `#[cfg(test)]`:
  - `src/app/construct.rs:56` calls `player.inhibit_mpv()` under `#[cfg(test)]`.
  - `src/app/library_browse_actions.rs:1199` uses `MockHttp` inside the `#[cfg(test)]` module at line 1116.
  - `signal_local_daemon_service_setup` (`remote_player/connect.rs:119`) is ungated production code. Only its re-export line sits next to a gated one.
- **Feature resolution.** The root package is edition 2021, so Cargo's resolver 2 applies. Under resolver 2, features that are enabled only through `[dev-dependencies]` are not unified into normal (non-test) builds.
- **`scripts/` users.**
  - The Lua files are named in the `[package.metadata.deb]` assets, `PKGBUILD`, `PKGBUILD-git` and `.github/workflows/build.yml:113-114`.
  - The Lua path is also runtime-resolved: `<checkout>/scripts/mbv.lua` is located through `CARGO_MANIFEST_DIR` (see `CONTEXT.md:161` and `config_tests_script_source.rs`).
  - `release.sh` is named by `.agents/skills/safe-mbv-release/SKILL.md` and mentioned in a comment in `build.yml:11`.
- **`assets/` users.** `tray_icon.bin` is used by `include_bytes!` in `src/tray.rs:6`. `power-card-placeholder.webp` is used by `include_bytes!` in `src/app/images.rs:95`. `icon.svg` is used by the packaging files. The two screenshots and `abs.svg` have no reference outside archived plans. `README.md` does not embed them.

## Goals / Non-Goals

**Goals:** stop release builds from compiling test-support code; remove dead files from the root and `assets/`; make `scripts/` hold only shipped runtime files; use one module-file style throughout.

**Non-Goals:**
- Any `src/app` or `mbv-core/src` module restructure (the sibling changes cover that).
- Changing installed paths such as `/usr/share/mbv/scripts/`.
- Converting `include!()`.

## Decisions

1. **Move the dev scripts, not the Lua.** The Lua owns the name `scripts/`: mpv calls them scripts, the install path is `/usr/share/mbv/scripts/`, and checkout resolution hard-codes `<checkout>/scripts/mbv.lua`. Moving the Lua would touch runtime path code, its tests, `CONTEXT.md`, both PKGBUILDs, the deb metadata and CI. Moving the two shell scripts touches one skill file and a comment. Alternative rejected: `mpv/` or `assets/mpv/` for the Lua.
2. **`tools/` for maintainer scripts.** It's a neutral name that can't collide with the Cargo `scripts`/`examples`/`benches` conventions. Alternative: `xtask` (a Rust crate). Rejected, because AGENTS.md forbids adding bespoke scripting, and converting shell scripts to Rust would be exactly that.
3. **Dev-dependency re-declaration for `test-support`.** `[dependencies]` keeps `mbv-core = { path = "crates/mbv-core" }` with no features. `[dev-dependencies]` adds `mbv-core = { path = "crates/mbv-core", features = ["test-support"] }`. This is the standard Cargo idiom. Alternative: a `mbv` feature that forwards to it. Rejected as extra configuration with no second user.
4. **`foo/mod.rs` everywhere.** 19 directory modules already use it and 3 don't, so the majority wins. Alternative: migrate everything to the 2018 `foo.rs` style. Rejected, because that is 19 moves instead of 3.
5. **Delete, don't archive, the root planning files and unreferenced assets.** Git history is the archive. Moving them to `docs/` would keep dead weight in every checkout.

## Risks / Trade-offs

- A release build could fail if some production path secretly needs a test-support item. Mitigation: task 1.1 verifies with `cargo check -p mbv --release` and `cargo build -p mbvd --release`, and any failure there is a real bug to fix, not to re-gate.
- External bookmarks to `scripts/release.sh` break. There's a single maintainer, and the skill file gets updated in the same commit.
