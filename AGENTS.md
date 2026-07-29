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
