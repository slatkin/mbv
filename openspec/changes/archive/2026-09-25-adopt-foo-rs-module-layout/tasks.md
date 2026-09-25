# Tasks

## 1. Precondition

- [x] 1.1 Confirm that `git status --short` is empty and no OpenSpec change is mid-apply. `encode-local-queue-owner` must still be at 0 tasks done. If it isn't, stop and report.

## 2. Rename

- [x] 2.1 Run `cargo qual fix -a mod_rs` (only that analyzer), then `git add -A`. Verify three things: `find src crates -name mod.rs` prints nothing; `git diff --cached -M --name-status | grep -c '^R100'` prints `123`; and `git diff --cached -M --name-status` has no lines other than `R100`. Then run `cargo check --workspace --all-targets`, which must pass.

## 3. Enforce

- [x] 3.1 Add `[workspace.lints.clippy]` with `mod_module_files = "deny"` to the root `Cargo.toml`, after `[workspace.dependencies]`. Add `[lints]` with `workspace = true` to the `mbv` package in the root `Cargo.toml`, and to `crates/mbv-core/Cargo.toml` and `crates/mbvd/Cargo.toml`. Verify: `cargo clippy --workspace --all-targets -- -D warnings` passes.

## 4. References

- [x] 4.1 Rewrite each `…/mod.rs` path to its new `….rs` path in `AGENTS.md` (line 34: `shell/run/mod.rs` → `shell/run.rs`), `.agents/skills/mbv-frontend/SKILL.md`, `.claude/agents/emby-research.md`, `docs/adr/`, `docs/invariants/`, `docs/palette.json`, `docs/palette.html`, comments in `.rs` files, and `openspec/changes/encode-local-queue-owner/`. Do not edit `docs/plans/`, `openspec/changes/archive/`, `clean-clippy-allows` or `remove-production-dead-code`. Keep AGENTS.md's "never `mod.rs`" rule. Verify: `rg -n --hidden 'mod\.rs' --glob '!target' --glob '!docs/plans/**' --glob '!openspec/changes/archive/**' --glob '!openspec/changes/clean-clippy-allows/**' --glob '!openspec/changes/remove-production-dead-code/**' --glob '!openspec/changes/adopt-foo-rs-module-layout/**'` shows only the AGENTS.md rule line.

## 5. Gates

- [x] 5.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace`; all must pass. Commit the rename, the lint and the references as one commit that closes #793.
