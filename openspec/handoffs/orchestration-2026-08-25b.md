# Orchestration handoff — migrate-tui-to-tuirealm, 2026-08-25 (second)

Supersedes `orchestration-2026-08-25.md` for everything it contradicts. Read
that one only for background on units before `bcf8558c`.

## Your role and the cadence

You are the orchestrator. **You do not dispatch agents.** You write prompts as
prose in chat; the maintainer pastes them into an agent and returns the report.

- **Assume an agent is in flight at all times** unless the maintainer says it
  finished. Do not offer the next prompt and do not write to the repo while one
  is running. Read-only investigation is fine.
- Write prompts as plain text, **never in a fenced or quoted block** — the
  maintainer copies them by hand and blocks make that painful.
- Do **not** wrap prompts in `/opsx:apply` or any slash workflow. The wrapper
  makes the agent re-read every context file and re-derive a scope it was
  handed, and its "pick the next unchecked task" loop causes agents to wander
  into the following unit. Agents tick `tasks.md` unprompted; name the exact row
  when the checkbox is nested.
- Verify a report by **reading its diff** (`git show`), not by re-running the
  commands it already ran.
- Do not run or gate on `rtk make check-code-file-lines`. It is a pre-PR check.
  `shell.rs` sitting at exactly 800 lines mid-lane is not a blocker and must not
  trigger a file split.

## State at handoff

HEAD is `c3bdb6a5`. **An agent is in flight on Framework deletion and the
working tree is broken mid-move** — `mouse_gestures.rs` exists with methods
outside an `impl App` block, `input_mouse*.rs` are deleted, `shell.rs` has
unresolved `E0599`s. That is a transient state, not a defect. Do not touch the
tree, do not "fix" it, and do not start anything until that agent reports.

Session commits, oldest first: `9e730855`, `b28428cc`, `ccc75e30` (album cursor
prep, complete), `bcf8558c` (rescope + verification policy), `b5799185` +
`24c550bc` (browser), `c7784c47` (home), `dc2de146` (orphan note), `d6d4fada` +
`1cce9cd6` (queue), `c70e3e03` (tv_workspace), `e11d5eab` (podcast note),
`c3bdb6a5` (D16).

## Decisions — do not relitigate

- **D16** (`design.md`, written this session): mouse is accepted-broken for
  alpha. The legacy mouse framework is deleted rather than migrated;
  `music_workspace` and the modal bundle are **not** migrated first. Mouse is
  verified post-alpha against real use. The ledger records that mouse is not
  part of any row's `migrated` criteria — without that, 5.5 would deadlock.
- **Verification policy** (recorded under 5.3d in `tasks.md`): the compiler is
  the primary gate. No behaviour-preservation tests — they pin drift. No mouse
  tests that hand-set `layout.main.*`. Manual use is acceptance. The maintainer
  declined an acceptance checklist; do not re-offer it.
- **D14** two-stage conversion and **D15** `Cmd` in / `Msg` out still govern.
- Deleting legacy branches was always **optional** in this lane. Three of five
  units deleted nothing and were right to.

## What is left after the in-flight unit

1. **5.4** — six precedence proofs. D16 demotes the three mouse ones to
   structural checks (absence of the three `input_mouse*.rs` files, no global
   hit map). The keyboard proofs stand. `KEY_POLICY` and
   `KeyPolicyGate::sub_clause()` still execute nowhere; `key_policy.rs` still
   carries `#![allow(dead_code)]`. Decide table-vs-runtime before starting.
2. **Album track focus** — delete `LibraryTab.album_track_focus`. Recount after
   the deletion lands: it was 20 write sites / 107 refs / 30 files, and 9 write
   sites lived in the three deleted files, so it should be materially smaller
   now. `tasks.md` schedules it inside the mouse lane; that still holds.
3. **Mirrors and framework** — 29 `sync_*` across 28 files, then `CONTEXT_STACK`,
   then `LegacyInput`, in that order.
4. **5.5** ledger flip, **5.6** final gate. Run `check-code-file-lines` here and
   split whatever is over.

## Failure modes seen this session — check for these in every report

- **Copy instead of move.** Twice an agent "extracted" a method by writing a new
  one while leaving the original inline, producing two correct implementations
  of one rule that the compiler cannot link. Caught only by
  `git show <sha> --numstat` on the source file showing zero deletions. Put that
  check in the prompt and verify it in the report.
- **Duplicated state with divergent initialisers.** `BrowserComponent` grew its
  own `last_click_pos` initialised `(0,0)` where `App`'s is
  `(u16::MAX, u16::MAX)` deliberately. Every gate passed; the bug was reachable.
  The rule that fixed it: the component owns **where**, the shell owns **when**.
- **Deleting on an unproven premise.** `c70e3e03` removed TV child-panel branches
  arguing podcasts never populate them, but the render gate
  (`is_wide_tv_library || is_podcast_library`) and the mount gate
  (`collection_type == "tvshows"`) were never shown to coincide. D16 moots it.
  Make agents state both predicates and refuse the deletion if they cannot close
  one.
- **Template prompts that assert the previous surface's shape.** The
  `tv_workspace` prompt listed four gestures; the component claimed one. The
  agent burned a long stretch reconciling the contradiction and wrote no code.
  **Read the component's `handle_mouse` before writing its prompt.**
- **Deleting by filename.** The first Framework-deletion prompt was wrong:
  every method the migrated shell arms call lived inside the three files marked
  for deletion. Delete by **entry point**, not by file.

## Known problems recorded but not fixed

- `LayoutMain`'s `tv_wide_*` / `wide_music_*` fields are screen-named geometry
  describing hero-on-left's child panels, and `is_wide_tv_active()` infers
  arrangement state from whether those fields were painted. This naming misled
  an implementer into treating shared geometry as surface-exclusive. Recorded in
  D16; not renamed. There is **one** Wide arrangement — hero-on-left,
  `hero_on_left_panes` takes only a `Rect` and is surface-independent. There is
  no such thing as a "TV layout"; do not use that phrase.
- `is_podcast_library` (`feed_actions.rs:386`) matches a library whose **name**
  contains "podcast". Pre-existing; worth an issue after the migration.
- Nothing in `openspec/specs/` was updated for D16. If a current spec asserts
  mouse behaviour, that delta belongs in this change's spec deltas before
  archive.
