# Project: mbv

# Issue tracker and Documentation
- Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI.
- Specs/notes/plans should be posted to GitHub rather than stored locally.
- Notes in Gists are preferable to local markdown files.
- ADR docs are in `docs/adr/` at the repo root. Do keep ADR docs when necessary.

# Execution
- run_in_background for builds/tests.
- If a unit test is flaky, delete it and write a new one. Do not troubleshoot unit tests.
- Be conscious of actions which will explode your context — delegate to a subagent instead.

# Worktree and PR workflow
- After creating a PR from a worktree, switch back to main if you are working in the main area.
- All docs, specs, plans, and design artifacts must be committed with the code in the same PR. Do not leave them uncommitted or in separate stashes.

# Ctrl protocol
- Additive changes get a capability string, not a version bump. See the rule above `CTRL_PROTOCOL_VERSION` in `crates/mbv-core/src/ctrl.rs`.
