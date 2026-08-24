# Handoff: migrate-tui-to-tuirealm

**Date:** 2026-08-24 (session 4 — re-scoping, doc-only)
**Baseline:** `ccc6565d` on `feat/migrate-tui-to-tuirealm`.
**Progress:** 17/40 tasks. No code written in sessions 2–4.

## Read this first

`tasks.md`'s "How to read this task list" section, then design **D14**.
Everything below is why they were rewritten.

## What was wrong

Three consecutive apply sessions (3.3, then 3.3 again, then 3.6) stalled at
200–300k context without writing code. Each traced the surface's `App` field,
found it read by several unrelated authorities, and correctly concluded the
task could not land as scoped.

The cause was a contradiction inside the change, not the code. `tasks.md`'s
preamble required every surface task to move state off `App` — "delete, don't
mirror". But:

- the capability spec scopes that rule to the **completion gate** and
  explicitly permits "internal checkpoints, behaviour-preserving commits, and
  temporary adapters";
- `design.md`'s Migration Plan puts mirror removal in phase 5;
- and all seven landed conversions **mirror**. `shell_overlays.rs` /
  `shell_home.rs` push `App` state into each component every tick via
  `get_component_mut` + downcast, and the ledger rows say so plainly
  ("forwards keys to existing `handle_key_confirm_modal`").

So `tasks.md` demanded, per task, something the design disclaimed and no
landed task had done — and the contradiction was undetectable without reading
all four artifacts, which is exactly the expensive trace that kept exploding.

The bridge that session 3 concluded "does not exist" **does** exist. It is the
`sync_<surface>()` pattern, now specified as design D14.

## What changed (session 4)

- **`tasks.md` preamble:** stage-1 bar is now component owns render + local
  state, shell mirrors, legacy input keeps forwarding, **do not delete `App`
  state**. Deletion is group 5.
- **§3.5-chain retired.** Its five tasks were serialized only because the
  shared wide-list render seam had no task of its own. New **3.11** does that
  extraction once (~1,500 lines across `list.rs`, `tv_wide.rs`,
  `movies_wide.rs`, `music_wide.rs`, `detail.rs`), mirroring what 3.1 did for
  Search. 3.3/3.5/4.2/4.3/4.4 are gated on 3.11 and then independent.
- **Phase 5 restructured** into per-authority-cluster teardown (5.3a Library/
  browse, 5.3b Feeds, 5.3c overlays, 5.3d framework) — clusters are the unit
  at which deletion is actually tractable.
- **New 1.10** (doc-only, do first): add the ledger's `component` state and
  demote the seven rows that read `migrated` while still mirroring.
- **design D14** added; one spec scenario added for the `component` state.

Two "open design questions" from session 3 are now closed as **deferred by
construction** to 5.2: Home's `key_policy` precedence gate, and the
per-instance `SubClause` for `lib_search`/`album_track_mode`. A stage-1
surface keeps legacy input, so neither guard is needed yet.

## Where to start

1. **1.10** — doc-only, ~20 minutes, unblocks honest ledger records.
2. Then any of **3.6, 3.8, 3.9, 3.10, 3.4, 4.1, 4.5–4.10** — independent,
   small under the corrected bar. 3.6 (Feeds) is fine now: mirror
   `App.feed_tab`, leave every consumer alone.
3. **3.11** when someone has budget for the largest diff in the change.

`scoping-3.3-3.5.md` remains accurate as a *code trace* and is the source for
3.11's file list. Ignore its scheduling recommendations — superseded here.
