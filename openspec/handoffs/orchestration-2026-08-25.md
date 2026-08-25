# Handoff — migrate-tui-to-tuirealm orchestration (2026-08-25)

## Role and cadence

**The user dispatches agents, not me.** I write the prompt as prose in chat;
the user pastes it into an agent and returns the agent's report. I do not call
the Agent tool. I do investigation myself rather than spawning read-only
research units — the user objected to that explicitly and was right.

Verification of an agent's report means **reading its diff** (`git show`), not
re-running the commands it already ran.

Worktree: `/home/slatkin/Dev/mbv/.worktrees/migrate-tui-to-tuirealm`
(a git worktree — never `cd` to the main repo; never bare `git stash`).

## Where the change stands

`openspec status --change migrate-tui-to-tuirealm --json` reported 50/58
before this session's commits. Groups 1-5.3c are done. Everything remaining is
under **5.3d**, plus 5.4/5.5/5.6.

### Landed and verified this session

| Commit | Unit |
| --- | --- |
| `9d59818c` | Album cursor Portion 1 — mount `MusicWorkspaceComponent` in narrow |
| `52364778` | Portion 2 — component album-order seam (`set_album_columns`, `move_album_rows`) |
| `9e730855` | Portion 2.5 — `set_inline_track_focus_enabled`, gate inline track focus to wide |
| `b28428cc` | Portion 3a — component owns album cursor targeting, emits `ShellRequest::MusicAlbumCursor { target, kind }` |

### In flight at handoff time

**Portion 3b is uncommitted in the working tree.** HEAD is `b28428cc`.
`rendered_album_target`, `rendered_album_jump_target`, and
`music_group_navigation` are already deleted from the tree, and *Album cursor
prep* is already ticked `[x]` in `tasks.md`, but **none of it is committed**.
When the report arrives: verify the diff, confirm the tick and the deletion
land in the same commit, and check the differential test in
`shell_music_workspace.rs` was **deleted** (its second path no longer exists,
so it cannot compile).

## Decisions made this session — do not relitigate

1. **Narrow mount (Path A) beat Path B.** `openspec/handoffs/album-cursor-prep.md`
   recommended keeping `App` authoritative for narrow and relocating the three
   cursor functions as `&mut App` delegators. That was overridden: the
   component now mounts in narrow *and* wide, because Path B would have put
   `&mut App` inside a component module, which the boundary rule rejects.

2. **`album_order` ≡ `music_group_navigation`** — verified in both the
   settled-catalog and fallback branches. Component navigation uses
   `MusicWideRenderCtx::album_order`. Do not rebuild a display plan.

3. **Verification policy (now recorded in `tasks.md` under 5.3d).** The
   compiler is the primary gate. No behaviour-preservation tests — this
   migration moves already-drifted behaviour, so such tests pin the drift.
   Differential tests only while two paths coexist, deleted with the second
   path. **No hand-set-coordinate mouse tests** — render into a `TestBackend`
   and hit-test the produced geometry, or write nothing. Per-unit gate:
   `rtk cargo check -p mbv`, `rtk cargo clippy --workspace --all-targets`,
   `rtk cargo nextest run -p mbv`, `rtk ast-grep scan`,
   `rtk make check-code-file-lines`. The maintainer's manual pass is
   acceptance.

4. **Behaviour has already drifted** — the user found it by hand and is
   deliberately parking it until the migration lands. Do not chase it. Do not
   claim a unit "preserves behaviour" as though the baseline were correct.

## Remaining work (rescoped in `tasks.md` this session)

- ***Album cursor prep*** — closes when 3b commits.
- ***Album track focus*** — delete `LibraryTab.album_track_focus`, re-home four
  `= None` resets. 30 files, 113 refs. 1-2 runs.
- ***Mouse geometry*** — rescoped from "12 components" to **nine**, in seven
  units: `browser`, `home`, `queue`, `tv_workspace`, `music_workspace` one
  each; `confirm`/`daemon_lost`/`remote_reanchor`/`playback_prompt` bundled;
  then Framework deletion (`input_mouse.rs` 653 + `input_mouse_dispatch.rs` 406
  + `input_mouse_gestures.rs` 172 + `AppLayout`). 6-11 runs. The ordering is
  one-directional — parallelises at the start, not the end.
- ***Mirrors and framework*** — 29 `sync_*` across 28 files, then
  `CONTEXT_STACK`, then `LegacyInput`, in that order. 2-3 runs.
- **5.4** — folded into the Mouse geometry lane's final unit. The
  table-vs-runtime question (D15: `KEY_POLICY` executes nowhere) must be
  decided **before** that unit starts.
- **5.5** ledger flip, **5.6** final gate — bookkeeping, 1 run.

Roughly 13-18 runs total remaining.

## What to check on every agent report

- Scope: exactly the permitted files, nothing else.
- `git status --short` clean and the commit actually present.
- The specific hazard the prompt named — agents hit those.
- New helpers that quietly introduce a *second* source of truth. Portion 3a did
  exactly this: it added `rendered_album_target` reading
  `layout.main.left_sorted_indices` (render output, written at paint time by
  `render/components/album.rs:165`, `album_inline.rs:65`,
  `list_letter_groups.rs:47`), creating a third order source and regressing
  `bool` fall-through so keys could drop to raw-index navigation. 3b removes it.
- Commit message style: neighbours use `refactor(tui): ...`. `b28428cc` does
  not. Not worth an amend on its own.

## Prompt-writing rules that worked

- State the hazard explicitly and name the file:line — agents hit named hazards
  and miss unnamed ones.
- Give the exact forbidden list (no rendering change, no `App` mutation, no
  checkbox tick, no ledger edit).
- Require verbatim command output, not a summary.
- "If X fails and you cannot fix it inside the listed files, stop and report
  instead of widening scope."
- End with "Do not delegate any part of this."
- Do **not** include orchestration language ("a sibling agent will...") — it
  gets executed.
