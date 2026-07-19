# Using Git Worktrees

Create an isolated branch workspace with safe defaults.

## Required Start

Announce: `I'm using the using-git-worktrees skill to set up an isolated workspace.`

## Directory Selection Priority

Check in order:

```bash
ls -d .worktrees 2>/dev/null     # Preferred (hidden)
ls -d worktrees 2>/dev/null      # Alternative
```

1. If `.worktrees/` exists - use it. If both exist, `.worktrees/` wins.
2. If `worktrees/` exists - use it.
3. Check project guidance file (e.g. `CLAUDE.md`) for a stated preference:
   ```bash
   grep -i "worktree.*director" CLAUDE.md 2>/dev/null
   ```
4. Ask user.

## Safety Check

For project-local worktree directories, verify ignore rules before creating:

```bash
git check-ignore -q .worktrees 2>/dev/null || git check-ignore -q worktrees 2>/dev/null
```

If not ignored:

1. Add the appropriate line to `.gitignore`.
2. Commit the `.gitignore` change immediately before proceeding. An uncommitted ignore entry is easy to lose and leaves the worktree contents exposed to accidental staging.

## Creation Steps

### 1. Detect project root and branch name

```bash
project=$(basename "$(git rev-parse --show-toplevel)")
```

Choose a descriptive `BRANCH_NAME` for the feature being isolated.

### 2. Create worktree and branch

```bash
git worktree add <path> -b <BRANCH_NAME>
```

The `cd <path>` in this step does not persist across separate shell calls. Use the full worktree path, or `cd <path> && <command>`, in every subsequent shell call rather than assuming the working directory carried over.

### 3. Serena binds to a worktree only if its own session started there

Serena is launched here as `serena start-mcp-server --context=claude-code
--project-from-cwd` (see the `serena` entry in the Claude Code MCP config).
`--project-from-cwd` makes Serena activate whatever directory the MCP server's
own process was started in — evaluated once, when that server process spawns,
which is when *this Claude Code session* started. It is not re-evaluated when
you later run `cd <worktree>` inside a `Bash` tool call in the same session:
Serena's server is a separate long-running process with its own fixed cwd: your
shell's cwd moving around inside this conversation does not move it. Per
Serena's own docs, `--context=claude-code` is a "single-project context," and
since a project was already provided at startup (`--project-from-cwd`),
Serena's `activate_project` tool — which *would* otherwise let you switch
projects mid-conversation — is deliberately disabled for the rest of the
session. There is no in-conversation escape hatch here; this is Serena's
documented, intentional behavior for this context, not a bug or a
configuration gap to work around.

**The correct fix is to do worktree work from a separate Claude Code session
started with its cwd inside the worktree** (e.g. `cd <worktree path> && claude`
in a new terminal/session, or whatever mechanism starts a fresh session there)
— that new session's own `serena start-mcp-server --project-from-cwd` call will
correctly bind to the worktree. This is Serena's own documented
worktree-parallelization model (see
https://oraios.github.io/serena/02-usage/999_additional-usage.html): one
Serena-backed session per worktree, not one session hopping between
directories. It also means `.serena/project.yml` existing in a worktree
(Serena creates one passively on first activation there) is *not* itself
evidence that the current session can use it — it only matters to whichever
session's MCP server was actually launched with that worktree as cwd.

**If you cannot start a fresh session** (e.g. you're a subagent dispatched
inside an already-running session that must continue operating across
multiple worktrees), Serena's tools for this session stay bound to wherever
that session started — treat that as fixed and route around it for any other
worktree:

- **Do not use Serena's symbolic edit tools** (`replace_symbol_body`,
  `insert_after_symbol`, `insert_before_symbol`, `replace_content`,
  `replace_in_files`, `rename_symbol`, `safe_delete_symbol`) **on files inside
  a worktree that isn't the one this session's Serena is bound to.** Their
  relative-path resolution targets Serena's fixed project root, so an edit
  meant for the worktree can silently land in the bound checkout's copy of the
  same file instead — divergent trees, same relative path, no error. This
  happened in practice (mbv issue #260 / PR #265): a `replace_symbol_body`
  call intended for a worktree edited the main checkout instead, and the
  mistake was only caught by an incidental hash/diff check, not by any
  tool-level error. Serena's read-only tools (`find_symbol`,
  `get_symbols_overview`, `find_referencing_symbols`, etc.) are lower-risk for
  orientation/research, but confirm which root they're reading from before
  trusting a "no results" as meaningful for the other worktree.
- Use plain-path tools instead for all such edits: `Read`/`Edit`/`Write` with
  the full worktree path, and `Bash` (`cd <worktree path> && ...` or absolute
  paths) for everything else.
- After every edit made this way, verify it landed in the right tree before
  moving on:
  ```bash
  git -C <worktree path> status --short           # should show the change
  git -C <serena-bound checkout path> status --short  # must stay empty
  ```
  If the Serena-bound checkout shows unexpected changes, stop immediately,
  diff against its own `HEAD` to confirm nothing besides the stray edit
  changed, revert it (`git checkout -- <file>` or `git restore <file>`,
  whichever the repo's own destructive-command policy prefers), and re-verify
  clean before continuing.

### 4. Run project setup

Auto-detect the project ecosystem and run the appropriate setup:

```bash
# Node.js
if [ -f package.json ]; then npm install; fi

# Rust
if [ -f Cargo.toml ]; then cargo build; fi

# Python
if [ -f requirements.txt ]; then pip install -r requirements.txt; fi
if [ -f pyproject.toml ]; then poetry install; fi

# Go
if [ -f go.mod ]; then go mod download; fi
```

If none of these files exist, skip dependency installation and note it in the output.

### 5. Run baseline tests

Run the project-appropriate test command to confirm the worktree starts clean:

```bash
# Use whichever applies
npm test
cargo test
pytest
go test ./...
```

## Failure Handling

If baseline tests fail, report the failures and ask whether to continue or investigate before proceeding.

## Success Output

Report:

- Worktree path (full absolute path)
- Branch name
- Ecosystem detected and setup command(s) run
- Baseline test status (passing count or failure summary)
- Serena status: whether this session's Serena is bound to this worktree (started
  here), or whether a fresh session should be started here and Serena's edit
  tools are being avoided for this worktree in the meantime per step 3

## Integration

Use with:

- `writing-plans`
- `subagent-driven-development` - required before executing any tasks
- `executing-plans` - required before executing any tasks

Cleanup is handled by `finishing-a-development-branch`.
