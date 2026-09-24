# Proposal

## Why

Two independent file/folder layout audits (2026-09-24) found repo-level hygiene problems that are cheap to fix and unrelated to the larger module restructures (`modularize-mbv-core-layout`, `modularize-app-layout`):

- Stray planning files are tracked at the repo root. `planning.html` (2.3 MB) is an `/opsx:explore` transcript and `HANDOFF-music-tree-browser.md` is its handoff; the grouped music tree browser has since shipped.
- Shipped runtime files and dev files are mixed together. `scripts/` holds the mpv Lua that mbv loads at runtime alongside two maintainer shell scripts. `assets/` holds files compiled into the binary alongside README screenshots that nothing references.
- Module files use two styles. Almost every directory module uses `foo/mod.rs`, but three use `foo.rs` + `foo/`.

## What Changes

- Delete `planning.html` and `HANDOFF-music-tree-browser.md`. Git history keeps them.
- Move `scripts/release.sh` and `scripts/reset-root-checkout.sh` to `tools/`, and update the `safe-mbv-release` skill and every other reference. `scripts/` then holds only the mpv Lua.
- Delete the unreferenced `assets/screenshot-music.png`, `assets/screenshot-power.png` and `assets/abs.svg`.
- Convert the three `foo.rs` + `foo/` modules to `foo/mod.rs`: `src/app/components/msg`, `src/app/render/components/media_list`, `crates/mbv-core/src/remote_player/connect`.

No runtime behaviour changes. Install paths and packaging outputs are unchanged.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. This is a pure refactor/tooling change, so `skip_specs: true`.

## Impact

- `.agents/skills/safe-mbv-release/SKILL.md`, `.github/workflows/build.yml` (only if it names the moved dev scripts).
- File moves under `scripts/`, `assets/`, `src/app/components/`, `src/app/render/components/`, `crates/mbv-core/src/remote_player/`.
- `mbv-core`'s public API is unchanged.
- Out of scope: the audits also flagged `test-support` being enabled on a runtime dependency. `coverage-quick-wins` (#771), task 1.1, already fixes that, so it is not repeated here.
- Umbrella: tracks alongside `modularize-mbv-core-layout` and `modularize-app-layout`. Land this change first; it is small and makes the later diffs smaller.
