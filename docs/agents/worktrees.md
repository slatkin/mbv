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

### 3. Re-point Serena at the worktree, or stop using it there

If Serena's tools are available in this session, they are almost certainly bound to
whatever directory the Serena MCP server was launched in — normally the main
checkout, not the worktree you just created — for the entire life of the session.
Check whether this session's Serena exposes a way to switch its active project
(e.g. search for a tool along the lines of `activate_project`/`switch_project`/
`set_project`). **Do not assume such a tool exists — verify first**, since as of
this writing no Serena deployment used in this repo has exposed one at runtime,
even though every worktree ends up with its own `.serena/project.yml` (created
passively, unused for switching).

If no such tool is available:

- **Do not use Serena's symbolic edit tools** (`replace_symbol_body`,
  `insert_after_symbol`, `insert_before_symbol`, `replace_content`,
  `replace_in_files`, `rename_symbol`, `safe_delete_symbol`) **on files inside the
  worktree.** Their relative-path resolution targets Serena's fixed project root,
  so an edit meant for the worktree can silently land in the main checkout's copy
  of the same file instead — divergent trees, same relative path, no error. This
  happened in practice (mbv issue #260 / PR #265): a `replace_symbol_body` call
  intended for a worktree edited the main checkout instead, and the mistake was
  only caught by an incidental hash/diff check, not by any tool-level error.
  Serena's read-only tools (`find_symbol`, `get_symbols_overview`,
  `find_referencing_symbols`, etc.) are lower-risk for orientation/research, but
  confirm which root they're reading from before trusting a "no results" as
  meaningful for the worktree.
- Use plain-path tools instead for all worktree edits: `Read`/`Edit`/`Write` with
  the full worktree path, and `Bash` (`cd <worktree path> && ...` or absolute
  paths) for everything else.
- After every edit made while working in a worktree, verify it landed in the
  right tree before moving on:
  ```bash
  git -C <worktree path> status --short    # should show the change
  git -C <main checkout path> status --short  # must stay empty
  ```
  If the main checkout shows unexpected changes, stop immediately, diff against
  its own `HEAD` to confirm nothing besides the stray edit changed, revert it
  (`git checkout -- <file>` or `git restore <file>`, whichever the repo's own
  destructive-command policy prefers), and re-verify clean before continuing.

If a project-switching tool *is* available in a given session, use it to
re-activate Serena against the worktree path immediately after creating the
worktree, then Serena's tools are safe to use normally for the rest of that
worktree's work — re-verify with the same `git status --short` check after the
first edit regardless, since a new capability is worth confirming once before
trusting it silently.

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
- Serena status: whether a project-switching tool was found and used, or whether
  Serena's edit tools are being avoided for this worktree per step 3

## Integration

Use with:

- `writing-plans`
- `subagent-driven-development` - required before executing any tasks
- `executing-plans` - required before executing any tasks

Cleanup is handled by `finishing-a-development-branch`.
