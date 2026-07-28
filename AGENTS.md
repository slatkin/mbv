# General rules
- Prefer evidence over assumptions: verify outcomes before final claims.
- Choose the lightest-weight path that preserves quality.
- Consult official docs before implementing with SDKs/frameworks/APIs.
- Do not add unit tests by default.

# Issue tracker and Documentation
- Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI.
- Specs/notes/plans should be posted to GitHub rather than stored locally.
- Notes in Gists are preferable to local markdown files.
- ADR docs are in `docs/adr/` at the repo root. Do keep ADR docs when necessary.

# Execution protocols
- Explore first, then plan.
- Keep and update domain docs while planning.
- run_in_background for builds/tests.
- Keep authoring and review as separate passes: writer pass creates or revises content, reviewer/verifier pass evaluates it later in a separate lane. Never self-approve in the same active context.
- If a unit test is flaky, delete it and write a new one. Do not troubleshoot unit tests.

# Operation principles
- Be conscious of actions which will explode your context and delegate to a subagent to that task instead.
- Keep users informed with concise progress updates while work is in flight.
- Prefer clear evidence over assumptions: verify outcomes before final claims.
- Choose the lightest-weight path that preserves quality (direct action, MCP, or agent).
- Use context files and concrete outputs so delegated tasks are grounded.
- Consult official documentation via subagents before implementing with SDKs, frameworks, or APIs.
- Reuse existing utilities and patterns before introducing new ones.
- Do not add new dependencies unless the user explicitly requests or approves them.
- Keep diffs small, reversible, and easy to review.
