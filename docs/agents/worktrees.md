# Using Git Worktrees

Create an isolated branch workspace with safe defaults. This mirrors the
current `superpowers:using-git-worktrees` skill's structure (Step 0 detect →
Step 1a native tools → Step 1b git fallback); this file adds mbv-specific
convention (`.claude/worktrees/`, Rust setup/test commands) and a Serena
caveat learned the hard way (see #260 / PR #265 / PR #266).

> ⚠️ **Before you touch a second worktree in this session: Serena will not
> follow you there.** It's a separate process, bound to a directory once, at
> session start — nothing you do mid-session moves it, including the native
> `EnterWorktree` tool. Using Serena's symbolic edit tools
> (`replace_symbol_body` and friends) against the wrong worktree silently
> edits the wrong tree with no error. This bit mbv issue #260 / PR #265.
> **Read Step 2 below before running any Serena edit tool once more than one
> worktree is in play.**

## Required Start

Announce: `I'm using the using-git-worktrees skill to set up an isolated workspace.`

## Step 0: Detect Existing Isolation

Before creating anything, check whether you're already in one:

```bash
GIT_DIR=$(cd "$(git rev-parse --git-dir)" 2>/dev/null && pwd -P)
GIT_COMMON=$(cd "$(git rev-parse --git-common-dir)" 2>/dev/null && pwd -P)
git branch --show-current
```

If `GIT_DIR != GIT_COMMON` (and `git rev-parse --show-superproject-working-tree`
is empty, i.e. you're not just in a submodule), you're already in a linked
worktree — skip to Step 3 (project setup), don't create another one.

## Step 1a: Native Worktree Tools (preferred)

If a native worktree tool is available in this session — in Claude Code this
is the `EnterWorktree` tool — prefer it over raw `git worktree add` for
**switching this session's own working directory** into a worktree, or for
creating a throwaway one where the forced branch-name pattern (below) doesn't
matter. It handles directory placement (`.claude/worktrees/<name>/`) and
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

1. This repo's established convention is `.claude/worktrees/<branch-name>/`
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
git check-ignore -q .claude/worktrees 2>/dev/null
```

`.claude/worktrees/` is already listed in this repo's `.gitignore` — if that
check fails (e.g. someone removed the entry, or you're using a different
directory), add the appropriate line and commit it before proceeding. An
uncommitted ignore entry is easy to lose and leaves worktree contents exposed
to accidental staging.

### Create the Worktree

```bash
git worktree add .claude/worktrees/<BRANCH_NAME> -b <BRANCH_NAME>
```

Note the `cd`-doesn't-persist caveat: a `cd <path>` inside one `Bash` tool
call does not carry over to the next call. Either use the full worktree path
(or `cd <path> && <command>`) in every subsequent `Bash` call, or use
`EnterWorktree(path: <path>)` to make the session's working directory (and
every tool that respects it) follow you there for the rest of the session —
remembering that Serena is the one thing that won't follow (next section).

## Step 2: Watch out for Serena — it will not follow you into the worktree

Serena is launched here as `serena start-mcp-server --context=claude-code
--project-from-cwd` (see the `serena` entry in the Claude Code MCP config).
`--project-from-cwd` binds Serena to whatever directory the MCP server
process itself was started in — evaluated once, when that process spawns,
which is when *this Claude Code session* started. **Nothing available inside
an already-running session moves it afterward** — not a plain `Bash cd`, and
not `EnterWorktree` either (tested, see Step 1a above): Serena's server is a
separate long-running process with its own fixed cwd, independent of the
session's own working-directory tracking.

**Proven directly with `ps`/`/proc`, not inferred** (checked while writing
this doc, after already having called `EnterWorktree` earlier in the same
session):

```
$ ps -eo pid,ppid,lstart,args | grep serena
3932 3886 ... /home/.../serena start-mcp-server --context=claude-code --project-from-cwd
$ readlink -f /proc/3932/cwd    # Serena (child)
/home/slatkin/Dev/mbv
$ readlink -f /proc/3886/cwd    # Claude Code CLI (parent, ppid of 3932)
/home/slatkin/Dev/mbv/.claude/worktrees/<worktree>
```

Serena (pid 3932) is a **direct child process of the Claude Code CLI itself**
(ppid 3886), spawned automatically ~4 seconds after the CLI process starts —
before any conversation content, before any tool call, before the session
has any chance to do anything at all. There's no "activate Serena" step
under a session's control to delay; the harness spawns it as part of MCP
initialization. And even setting timing aside: the two `readlink`s above
show the *parent* CLI process's own cwd really did change (confirming
`EnterWorktree` performs a genuine `chdir` on the CLI process, not just
internal bookkeeping) while the *child* Serena process's cwd did not move —
ordinary POSIX semantics: a child doesn't track its parent's cwd after
`fork`/`exec`, so nothing re-executes Serena when the parent's cwd changes
later. The only lever that works is which directory the `claude` process
itself launches from, *before* Serena's child process is spawned — i.e. a
fresh `cd <worktree> && claude` invocation, not anything achievable from
inside an already-running session.

Per Serena's own docs, `--context=claude-code` is a "single-project context,"
and once a project is provided at startup (`--project-from-cwd` counts),
Serena's `activate_project` tool — which would otherwise allow mid-session
project switching — is deliberately disabled for the session's lifetime. This
much *is* documented Serena behavior for this context, not a bug
(`02-usage/050_configuration.html`'s "contexts" section: single-project
contexts disable `activate_project` "since changing the active project
ceases to be a relevant operation in this case").

**What is not documented anywhere in Serena's docs is the specific failure
mode this section exists to prevent:** an already-running MCP server whose
*client* redirects its own working directory mid-session (e.g. via
`EnterWorktree`) without restarting the server. Checked the workflow,
additional-usage, and configuration pages, and the site's top-level nav —
there is no troubleshooting/FAQ page, no mention of multiple simultaneous
Serena instances, no guidance on restarting/reconnecting to pick up a new
directory, and no discussion of what happens to in-flight tool calls when
the client's cwd and the server's cwd disagree. The silent-wrong-file-edit
behavior described below (mbv issue #260 / PR #265) was found by direct
testing in this repo, not by reading published guidance — treat it as
empirically confirmed for this Serena version/config, not as a stated
guarantee that will necessarily hold across Serena updates.

**The actual fix is to do worktree work from a separate Claude Code session
whose own cwd is the worktree from the start** — a fresh `claude` process
launched with `cd <worktree path> && claude` (or Claude Code's native
`claude --worktree <name>` flag, which creates the worktree and starts that
fresh session in one step — though note it has the same forced
`worktree-<name>` branch pattern as `EnterWorktree`, so combine it with a
manual `git branch -m` rename afterward if a clean name matters). That new
session's own `serena start-mcp-server --project-from-cwd` binds correctly
from its own launch. This matches Serena's documented worktree-parallelization
model (https://oraios.github.io/serena/02-usage/999_additional-usage.html):
one Serena-backed session per worktree, not one session hopping between
directories — and it means a worktree's `.serena/project.yml` (Serena creates
one passively on first activation there) is *not* itself evidence that the
*current* session can use it; it only matters to whichever session's MCP
server was actually launched with that directory as cwd.

**If you cannot start a fresh session** (e.g. you're a subagent dispatched
inside an already-running session that must touch a worktree other than the
one this session's Serena is bound to — including via `EnterWorktree`, which
moves the session but not Serena, per the tested finding above): treat
Serena's binding as fixed for the rest of this session and route around it
for any other worktree:

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

## Step 3: Run project setup

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

For mbv specifically, this is always `cargo build --workspace`.

If none of these files exist, skip dependency installation and note it in the output.

## Step 4: Run baseline tests

Run the project-appropriate test command to confirm the worktree starts clean:

```bash
# Use whichever applies
npm test
cargo test --workspace   # mbv
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
- Serena status: whether this session's Serena is bound to this worktree
  (started here), or whether a fresh session should be started here and
  Serena's symbolic edit tools are being avoided for this worktree in the
  meantime per Step 2 above

## Integration

Use with:

- `writing-plans`
- `subagent-driven-development` - required before executing any tasks
- `executing-plans` - required before executing any tasks

Cleanup is handled by `finishing-a-development-branch`, or by `ExitWorktree`
if the worktree was entered via `EnterWorktree`.
