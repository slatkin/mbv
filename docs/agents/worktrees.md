# Using Git Worktrees

Create an isolated branch workspace with safe defaults.

Before creating anything, check whether you're already in one:

```bash
GIT_DIR=$(cd "$(git rev-parse --git-dir)" 2>/dev/null && pwd -P)
GIT_COMMON=$(cd "$(git rev-parse --git-common-dir)" 2>/dev/null && pwd -P)
git branch --show-current
```

If `GIT_DIR != GIT_COMMON` (and `git rev-parse --show-superproject-working-tree`
is empty, i.e. you're not just in a submodule), you're already in a linked
worktree — skip to Step 3 (project setup), don't create another one.

If a native worktree tool is available in this session — in Claude Code this
is the `EnterWorktree` tool — prefer it over raw `git worktree add` for
**switching this session's own working directory** into a worktree, or for
creating a throwaway one where the forced branch-name pattern (below) doesn't
matter. It handles directory placement (`.worktrees/<name>/`) and
cleanup (`ExitWorktree`) for you, and moves the session's working directory,
write access, and project configuration (`CLAUDE.md`, settings) to the
target.

**Two things to know before reaching for it, both confirmed by testing in
this repo (2026-07-19, while writing this doc):**

1. **`EnterWorktree` forces the branch name to `worktree-<name>`.** Tested:
   `EnterWorktree(name: "test-branch-naming-probe")` created branch
   `worktree-test-branch-naming-probe`, not `test-branch-naming-probe`. mbv's
   convention is descriptive, often issue-numbered branch names
   (`issue-260-load-timing`) with no forced prefix — if that matters for the
   task, use Step 1b's `git worktree add -b <BRANCH_NAME>` instead, then
   optionally `EnterWorktree(path: <that worktree>)` to move the session into
   it (see point 2 for what that does and doesn't fix).
2. **`EnterWorktree` does not rebind Serena.** Tested: entered an existing
   worktree with `EnterWorktree(path: ...)`, confirmed the session's own
   `Bash` cwd correctly moved there, then called a Serena symbolic tool
   (`find_symbol`) on a file that differs between the worktree and the main
   checkout — it returned the **main checkout's** version, not the
   worktree's. See Step 2 below before trusting Serena's tools after
   switching.

## Step 1b: Git Worktree Fallback

Use this when no native tool is available, or when Step 1a's forced branch
name doesn't fit (mbv's common case).

### Directory Selection

1. This repo's established convention is `.worktrees/<branch-name>/`
   — matching Claude Code's own native default path for `EnterWorktree`/
   `--worktree` (see Step 1a), so a worktree created manually here looks and
   behaves the same as one Claude Code would have created natively. Use it
   unless the user or task instructions say otherwise.
2. If for some reason that's not applicable, fall back to checking for
   `.worktrees/` or `worktrees/` at the repo root, in that order, before
   asking the user.

### Safety Check

Verify the worktree directory is actually ignored before creating anything
under it:

```bash
git check-ignore -q .worktrees 2>/dev/null
```

`.worktrees/` is already listed in this repo's `.gitignore` — if that
check fails (e.g. someone removed the entry, or you're using a different
directory), add the appropriate line and commit it before proceeding. An
uncommitted ignore entry is easy to lose and leaves worktree contents exposed
to accidental staging.

### Create the Worktree

```bash
git worktree add .worktrees/<BRANCH_NAME> -b <BRANCH_NAME>
```

### Before Pushing a PR Branch

Before pushing or opening a PR from a worktree branch, update it against the
current integration branch:

```bash
git fetch origin
git merge --no-ff origin/main
```

Resolve any conflicts in the worktree, then run the required checks before
pushing. Prefer this merge step over discovering conflicts after GitHub has
already opened the PR.

Note the `cd`-doesn't-persist caveat: a `cd <path>` inside one `Bash` tool
call does not carry over to the next call. Either use the full worktree path
(or `cd <path> && <command>`) in every subsequent `Bash` call, or use
`EnterWorktree(path: <path>)` to make the session's working directory (and
every tool that respects it) follow you there for the rest of the session —
remembering that Serena is the one thing that won't follow (next section).

- **Do not use Serena's symbolic edit tools** (`replace_symbol_body`,
  `insert_after_symbol`, `insert_before_symbol`, `replace_content`,
  `replace_in_files`, `rename_symbol`, `safe_delete_symbol`) **on files inside
  a worktree that isn't the one this session's Serena is bound to.** Their
  relative-path resolution targets Serena's fixed project root, so an edit
  meant for the worktree can silently land in the bound checkout's copy of
  the same file instead — divergent trees, same relative path, no error. This
  happened in practice (mbv issue #260 / PR #265): a `replace_symbol_body`
  call intended for a worktree edited the main checkout instead, and the
  mistake was only caught by an incidental hash/diff check, not by any
  tool-level error. Serena's read-only tools (`find_symbol`,
  `get_symbols_overview`, `find_referencing_symbols`, etc.) are lower-risk
  for orientation/research, but confirm which root they're reading from
  before trusting a "no results" as meaningful for the other worktree — a
  live symbol lookup that should differ between the two trees (like the test
  above) is a cheap, concrete way to check.
- Use plain-path tools instead for all such edits: `Read`/`Edit`/`Write` with
  the full worktree path, and `Bash` (`cd <worktree path> && ...` or absolute
  paths) for everything else.
- After every edit made this way, verify it landed in the right tree before
  moving on:
  ```bash
  git -C <worktree path> status --short               # should show the change
  git -C <serena-bound checkout path> status --short   # must stay empty
  ```
  If the Serena-bound checkout shows unexpected changes, stop immediately,
  diff against its own `HEAD` to confirm nothing besides the stray edit
  changed, revert it (`git checkout -- <file>` or `git restore <file>`,
  whichever the repo's own destructive-command policy prefers), and
  re-verify clean before continuing.
