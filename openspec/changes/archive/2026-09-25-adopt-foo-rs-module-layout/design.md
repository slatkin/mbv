# Design

## Context

These facts were verified against `main` at 9f1caceb5:

- `find src crates -name mod.rs` finds 123 files. There are none outside `src/` and `crates/`, and no `tests/`, `benches/` or `examples/` directories hold one.
- `cargo qual fix --dry-run -a mod_rs` (cargo-quality, installed at `~/.local/bin/cargo-qual`) lists exactly those 123 renames. The only `#[path]` attribute in the tree is `src/mpris.rs:574` (`#[path = "mpris_tests.rs"]`), which does not sit in a `mod.rs`. Two files carry file-relative `include_bytes!`/`include_str!` paths — `src/app/infra/images.rs` and `src/app/render/theme/palette.rs` (`include_str!("mod.rs")`) — that required adjustment when the rename moved their containing files one directory shallower.
- No `Cargo.toml` has a `[lints]` or `[workspace.lints]` table yet. The workspace members are `.` (`mbv`), `crates/mbv-core` and `crates/mbvd`. `rust-version = "1.88"` supports workspace lints.
- These non-archived files outside `docs/plans/` reference `mod.rs`: `AGENTS.md:34` (`shell/run/mod.rs`), `.agents/skills/mbv-frontend/SKILL.md`, `.claude/agents/emby-research.md`, `docs/adr/{0006,0010,0013,0014,0015}`, `docs/invariants/{05,06,07,09,10,12,13}`, `docs/palette.{json,html}`, about 15 comments in `.rs` files, and the open changes `encode-local-queue-owner`, `clean-clippy-allows` and `remove-production-dead-code`.

## Goals / Non-Goals

**Goals:** have no `mod.rs` files in the tree; stop new ones through clippy; point current-state docs at real paths.

**Non-Goals:**
- Splitting, merging or renaming any module.
- Removing the `src/mpris.rs` `#[path]`. AGENTS.md forbids that too, but it's a separate fix.
- Editing archived changes or `docs/plans/`, which are historical records.

## Decisions

1. **Rename with `cargo qual fix -a mod_rs`, then `git add -A`.** The tool does a mechanical rename, and it's the only analyzer run, because the other analyzers' fixes are unwanted. The moves are byte-identical, so git's rename detection keeps `git log --follow` history just as `git mv` would. Rejected alternative: hand-scripting 123 `git mv` calls. AGENTS.md bans bespoke scripting, and the tool already exists.
2. **Enforce with the stock clippy lint `mod_module_files = "deny"` through workspace lints.** One table in the root `Cargo.toml` and one `[lints] workspace = true` line in each member manifest. This tightens the lints and loosens nothing, so the lint-suppression rule doesn't apply. Rejected alternative: a `cargo qual check` CI step. That adds a tool dependency to CI when clippy already runs there.
3. **Reference rewrite scope.** Update anything that describes the tree as it is now or will be: AGENTS.md, skills, agent definitions, ADR and invariant path mentions, palette docs, source comments, and `encode-local-queue-owner` (0/12 tasks, so it runs after this lands). Leave the `mod.rs` mentions in `clean-clippy-allows` and `remove-production-dead-code` alone. They describe completed tasks and are about to be archived. Keep the AGENTS.md rule text "never `mod.rs`".
4. **One commit.** The rename, the lint and the reference updates land together. Every intermediate state is then consistent, and a revert is a single step.

## Risks / Trade-offs

- **Merge conflicts with in-flight work** → Precondition: a clean tree, and no change mid-apply. `encode-local-queue-owner` must not have started. If it has, finish it first, because its paths would move under it.
- **An unfamiliar `cargo qual` also edits file contents** → Verify that `git diff --cached -M --stat` shows only 100%-similarity renames before the reference edits.
