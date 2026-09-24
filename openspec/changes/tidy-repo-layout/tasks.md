# Tasks

## 1. Remove dead files

- [x] 1.1 `git rm planning.html HANDOFF-music-tree-browser.md`. Verify: `rg -l 'planning.html|HANDOFF-music-tree-browser' --glob '!openspec/changes/archive/**'` returns nothing outside this change.
- [x] 1.2 `git rm assets/screenshot-music.png assets/screenshot-power.png assets/abs.svg`. Verify: `rg -n 'screenshot-(music|power)|abs\.svg' --glob '!openspec/changes/archive/**' --glob '!docs/plans/**'` returns nothing, and `cargo check -p mbv` still succeeds (the `include_bytes!` assets are untouched).

## 2. Separate dev scripts from shipped Lua

- [ ] 2.1 `git mv scripts/release.sh scripts/reset-root-checkout.sh tools/`. Update every reference: `.agents/skills/safe-mbv-release/SKILL.md`, the `build.yml:11` comment, each script's own usage lines, and whatever `rg -n 'scripts/(release|reset-root-checkout)'` finds outside archived changes. Verify: that `rg` returns nothing, `ls scripts` shows only `*.lua`, and `bash -n tools/release.sh tools/reset-root-checkout.sh` passes.

## 3. One module-file style

- [ ] 3.1 `git mv src/app/components/msg.rs src/app/components/msg/mod.rs`, `git mv src/app/render/components/media_list.rs src/app/render/components/media_list/mod.rs`, and `git mv crates/mbv-core/src/remote_player/connect.rs crates/mbv-core/src/remote_player/connect/mod.rs`. Fix any relative `include!`/`#[path]` inside the moved files. `connect.rs:4` does `include!("tests.rs")`, which already resolves against `connect/`, so check it still does. Verify: `cargo check --workspace --all-targets` passes, and this loop prints nothing: `for d in (find src crates -mindepth 1 -type d -not -name tests -not -name fixtures -not -name examples); test -f $d.rs; and echo $d; end`

## 4. Gates

- [ ] 4.1 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace` all pass. Commit.
