# General rules
- Prefer evidence over assumptions: verify outcomes before final claims.
- Choose the lightest-weight path that preserves quality.
- Consult official docs before implementing with SDKs/frameworks/APIs.
- Read `docs/agents/rules/` when starting a parent session.

## Issue tracker
Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI. External PRs are not pulled into triage. See `docs/agents/issue-tracker.md`.

## Domain docs
Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

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

# Delegation
 - Use delegation when it improves quality, speed, or correctness: Multi-file implementations, refactors, debugging, reviews, planning, research, and verification. Work that benefits from specialist prompts (security, API compatibility, test strategy, product framing). Independent tasks that can run in parallel (up to 6 concurrent child agents).
- Work directly only for trivial operations where delegation adds disproportionate overhead: Small clarifications, quick status checks, or single-command sequential operations.

Delegation steps:
1. Decide which agent role to delegate to (e.g., `architect`, `executor`, `debugger`)
2. Call `spawn_agent` with `message` containing the agent's role and task description
3. The child agent receives full role context and executes the task independently

Parallel delegation (up to 6 concurrent):
```
spawn_agent(message: "<architect prompt>\n\nTask: Review the auth module")
spawn_agent(message: "<executor prompt>\n\nTask: Add input validation to login")
spawn_agent(message: "<test-engineer prompt>\n\nTask: Write tests for the auth changes")
```
Claude Code spawns child agents via the `spawn_agent` tool (requires `multi_agent = true`).
To inject role-specific behavior, the parent MUST read the role prompt and pass it in the spawned agent message.

Delegation steps:
1. Decide which agent role to delegate to (e.g., `architect`, `executor`, `debugger`)
2. Call `spawn_agent` with `message` containing the prompt content + task description
3. The child agent receives full role context and executes the task independently

Parallel delegation (up to 6 concurrent):
```
spawn_agent(message: "<architect prompt>\n\nTask: Review the auth module")
spawn_agent(message: "<executor prompt>\n\nTask: Add input validation to login")
spawn_agent(message: "<test-engineer prompt>\n\nTask: Write tests for the auth changes")
```

Each child agent:
- Inherits AGENTS.md context (via child_agents_md feature flag)
- Runs in an isolated context with its own tool access
- Returns results to the parent when complete

Key constraints:
- Max 3 concurrent child agents
- Each child has its own context window (not shared with parent)
- Parent must read prompt file BEFORE calling spawn_agent
- Child agents can access skills ($name) but should focus on their assigned role, a spec/plan should be presented to an executor agent (default for both standard and complex implementation work). For non-trivial SDK/API/framework usage, delegate to research agent to check official docs first and to answer specific questions.

# Model Routing
Match agent role to task complexity:
- **Low complexity** (quick lookups, narrow checks): `explorer`
- **Standard** (implementation, debugging, reviews): `executor`, `debugger`, `test-engineer`
- **High complexity** (architecture, deep analysis, complex refactors): `architect`, `executor`, `critic`
