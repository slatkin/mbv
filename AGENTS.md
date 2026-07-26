# General rules
- Prefer evidence over assumptions: verify outcomes before final claims.
- Choose the lightest-weight path that preserves quality.
- Consult official docs before implementing with SDKs/frameworks/APIs.

## Issue tracker
Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI.

## Domain docs
Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Module naming conventions (src/app/)
Modules in `src/app/` follow two naming conventions plus a default:
- **Prefix `types_`**: For modules that primarily define type aliases, enums, or structs used across the app (e.g., `types_browse.rs`, `types_playback.rs`)
- **Suffix `_actions`**: For modules that implement action handlers or event dispatch logic (e.g., `queue_actions.rs`, `notify_actions.rs`)
- **Bare noun**: For all other modules (constructors, state, input handling, etc.)

Future module splits should follow this existing pattern. When in doubt, prefer bare nouns — the prefix/suffix conventions are reserved for modules that genuinely benefit from categorization. Existing files are grandfathered in; do not rename for convention compliance alone.

# Execution protocols
Broad requests: explore first, then plan. Keep and update domain docs while planning. 2+ independent tasks in parallel. run_in_background for builds/tests. Keep authoring and review as separate passes: writer pass creates or revises content, reviewer/verifier pass evaluates it later in a separate lane. Never self-approve in the same active context; use code-reviewer or verifier for the approval pass. Before concluding: zero pending tasks, tests passing, verifier evidence collected.

# Operation principles
- Use the `worktrees` skill before delegating to executor agents. Executors should always work in isolated worktrees.
- Delegate specialized or tool-heavy work to the most appropriate agent.
- Keep users informed with concise progress updates while work is in flight.
- Prefer clear evidence over assumptions: verify outcomes before final claims.
- Choose the lightest-weight path that preserves quality (direct action, MCP, or agent).
- Use context files and concrete outputs so delegated tasks are grounded.
- Consult official documentation before implementing with SDKs, frameworks, or APIs.
- For cleanup or refactor work, write a cleanup plan before modifying code.
- Prefer deletion over addition when the same behavior can be preserved.
- Reuse existing utilities and patterns before introducing new ones.
- Do not add new dependencies unless the user explicitly requests or approves them.
- Keep diffs small, reversible, and easy to review.

# Working agreements
- Write a cleanup plan before modifying code.
- Prefer deletion over addition.
- Reuse existing utilities and patterns first.
- No new dependencies without an explicit request.
- Keep diffs small and reversible.
- Run lint, typecheck, tests, and static analysis after changes.
- Final reports must include changed files, simplifications made, and remaining risks.
