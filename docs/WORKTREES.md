# Working in git worktrees

Rules and gotchas for agents that create or operate inside a git worktree
(e.g. via the `EnterWorktree` tool) for this repo. Read this before doing any
work in a worktree.

## Serena is not worktree-aware mid-session

Serena's MCP server binds to a project root once, at activation. If a worktree
is created *after* Serena has already activated against the main checkout,
Serena keeps operating against the main checkout's path for the rest of the
session — regardless of the shell's current working directory. There is no
tool exposed to the agent to re-point Serena at the worktree mid-session.

Concretely, this means every Serena tool (`find_symbol`, `replace_symbol_body`,
`replace_content`, `get_symbols_overview`, etc.) called while "in" a worktree
will silently read and write files in the **original** checkout, not the
worktree — even though `Read`/`Edit`/`Bash` correctly follow the worktree cwd.
If someone else has uncommitted changes in that original checkout, Serena
edits can land mixed into their working tree.

Serena's own docs describe worktree support as a first-class workflow, gated
on either launching with `--project-from-cwd` or calling `activate_project`
after switching directories. Neither of those currently happens automatically
when an agent session enters a worktree mid-conversation — this looks like a
session-sequencing gap in how the harness wires up Serena's activation, not a
fundamental Serena limitation.

**Rule:** once you enter a worktree in a session, do not use Serena's tools
for the rest of that session. Use `Read`, `Edit`, `Write`, `Grep`/`grep`, and
`Bash` directly instead. Verify file locations with `pwd` / absolute paths
when in doubt.

**If you suspect Serena wrote to the wrong place:** diff the same relative
path in both the worktree and the original checkout. If the original checkout
picked up changes it shouldn't have, extract just your intended changes with
`git diff -- <files> > /tmp/patch`, `git checkout -- <files>` in the original
checkout to restore it, then `git apply /tmp/patch` inside the worktree.
