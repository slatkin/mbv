# Scoping: 5.3c and 5.3d

Written before dispatch, to keep the remaining teardown from repeating the
5.3a pattern — three passes, a prerequisite discovered mid-flight, and a
shipped regression.

**Method.** Counts below are compile-forced edit sites (writes, and reads that
name a deleted field), not grep hits. Where a field has a single `open_*`
choke point the fan-out is one site; where it has none, every raise site is
forced. This is the `5.3-pre` lesson (`5d9e77ec`) applied ahead of time.

**Verdict.** Neither task is dispatchable as written. 5.3c needs a prerequisite
and a three-way split. 5.3d is four tasks in a trench coat, one of which
(`album_track_focus`) is the size of all of 5.3a.

---

## 5.3c — overlay/modal cluster

### Blocker, fix first

`src/app/shell_overlays.rs` is at **exactly 800 lines**, the repo cap, and
holds 11 of the 29 `sync_*` methods. Every 5.3c sub-task edits it. Split it
before any teardown lands — the natural seam is by overlay family, matching
the sub-task split below.

### Field inventory

Compile-forced sites, excluding tests:

| Field | Raise sites | Choke point? | Presence reads outside the cluster |
| - | - | - | - |
| `confirm_modal` | 9 across 7 files | **no** | 5 |
| `selection_modal` | 1 (`open_selection_modal`) | yes | 3 |
| `context_menu` | 2 (`open_context_menu`, `_at`) | yes | 1 |
| `save_playlist_dialog` | 2 | partial | 2 |
| `daemon_lost_modal` | 1 (`raise_daemon_lost_modal`) | yes | 1 |
| `remote_reanchor_popup` | 1 | yes | 1 |
| `multiselect_popup` / `library_routes_popup` / `feeds_manage_popup` | 1 each | yes | 0 |
| `show_settings` / `show_sessions` / `show_playlists` / `search_sidebar_open` | 2–4 each | partial | 2–3 each |

`show_help` is at **0 references** — Help (2.1) is fully torn down and is the
proof that a clean `migrated` row is reachable.

### The prerequisite: `confirm_modal` has no constructor

Nine `self.confirm_modal = Some(ConfirmModal { .. })` literals across
`queue_actions.rs` (3), `services_settings.rs` (2), `input_playlist_keys.rs`
(2), `app_emby_service_completion.rs`, `audiobookshelf_service_actions.rs`,
`feeds_manage_actions.rs`, `input_lib_keys.rs`, `input_confirm_keys.rs`. Every
one is compile-forced the moment the field moves.

**5.3c-pre** (behaviour-neutral, compiles standalone): add
`App::ask_confirm(ConfirmModal)` and route all nine through it. Same shape and
same justification as `5.3-pre`. Do this before anything else in the cluster.

The ~24 `confirm_modal = None` sites need no prerequisite: all but four live
inside `input_confirm_keys.rs`, which this task deletes anyway.

### The real design question — answer before dispatch

Five `impl App` sites ask *"is a blocking modal up?"* by reading the field
directly:

    input_mouse_panels.rs:159        input_playlist_keys.rs:150
    library_load_actions.rs:242      run_loop_drains.rs:178
    render/screens/root.rs:139,173,183

This is a **precedence** question, not overlay state, and it is the same
cluster-boundary violation that ejected `album_track_focus` from 5.3a: the
field is read by authorities outside its cluster. The shell can answer it —
`application.mounted(&ComponentId::Modal(..))`, built at 5.2 — but `impl App`
cannot reach `self.application`, so the readers cannot simply be rewritten in
place.

Two options, and this is a decision, not an implementation detail:

- **(a)** Relocate all five readers into the shell. Correct end state; makes
  5.3c materially larger and drags `root.rs`'s overlay-visibility assertions
  with it.
- **(b)** Keep one shell-set `blocking_overlay_active: bool` on `App` as a
  temporary adapter, deleted at 5.3d with the other mirrors. Keeps 5.3c
  mechanical; defers the same question a third time.

D14 permits (b) explicitly ("temporary adapters"). Recommend (b): it is the
only option that keeps 5.3c a mechanical task, and 5.3d has to revisit every
`impl App` reader regardless.

### Split

- **5.3c-pre** — `ask_confirm` helper; split `shell_overlays.rs`. No deletion.
- **5.3c-1 Modals** — `confirm_modal`, `daemon_lost_modal`,
  `remote_reanchor_popup`, `save_playlist_dialog` + `input_confirm_keys.rs`,
  `input_daemon_lost_keys.rs`, `input_remote_reanchor.rs`,
  `handle_key_save_playlist_entry`. All four have blocking-swallow semantics
  and one shared presence predicate.
- **5.3c-2 Sidebars** — `show_settings`, `show_sessions`, `show_playlists`,
  `search_sidebar_open` + `input_settings_keys.rs`, `input_playlist_keys.rs`,
  `services_settings.rs`'s three `handle_key_*`. Open-flags, no payload.
- **5.3c-3 Menus and popups** — `context_menu`, `selection_modal`,
  `multiselect_popup`, `library_routes_popup`, `feeds_manage_popup` +
  `input_context_menu.rs`, `input_selection_modal_keys.rs`,
  `input_feeds_manage_keys.rs`, and the duplicated variable-row geometry in
  `input_mouse_panels.rs` (212 lines). `selection_modal` is the largest single
  field in the cluster (166 refs) but routes through one `open_*`, so its
  fan-out is presence-reads only.

Each sub-task must re-home its **reset triggers**, not just relocate storage.
See `758d0a84`.

---

## 5.3d — framework removal

As written this holds `LegacyInput` + `CONTEXT_STACK` + `AppLayout` + all
remaining mouse paths + all 29 `sync_*` mirrors + `album_track_focus` +
relocating `render/screens/album_cursor.rs` + an unresolved narrow-mode
question. Handing that to one agent reproduces the 3.3/3.6 stall.

### Measured

| Piece | Refs | Files | Real cost |
| - | - | - | - |
| `LegacyInput` | 19 | 7 | small — deletion only, *after* every forwarder is gone |
| `CONTEXT_STACK` | 25 | 10 | small once nothing dispatches through it |
| `AppLayout` | 22 | 10 | **misleading** — see below |
| `album_track_focus` | 108 | 28 | 5.3a-sized on its own |
| `sync_*` mirrors | 29 methods | 18 files | one per surface, mechanical |

**`AppLayout`'s 22 references understate it by an order of magnitude.** The
cost is `layout.main.*`, read across **44 files** — 30 sites in
`input_mouse.rs` alone, 16 in `input_mouse_dispatch.rs`, 13 in
`lib_cursor_actions.rs`. Deleting `AppLayout` means every one of those reads
component-owned geometry instead.

And the mouse migration is barely started: **3 of 43 components own geometry;
12 still forward mouse to legacy** (`playback_prompt`, `feeds`, `context_menu`,
`music_workspace`, `browser`, `home`, `daemon_lost`, `tv_workspace`, `queue`,
`confirm`, `remote_reanchor`). That is 12 surfaces of `hit_test` migration
across 1,551 lines of `input_mouse*.rs`, which is larger than any group-4 task
and is also what 5.4 exists to verify. 5.3d and 5.4 are entangled and should
be sequenced as one lane.

### Split

- **5.3d-a `album_track_focus`.** Its own task, sized like 5.3a. Carries the
  three blockers already recorded under 5.3a plus the withdrawn `5.3a-post`
  finding, and must settle the narrow-mode question first: either mount
  `MusicWorkspaceComponent` in narrow mode, or prove the narrow path cannot
  reach a `Some`. Folds in `album_cursor.rs`'s three `pub(in crate::app)`
  entry points and their four `= None` resets.
- **5.3d-b mouse geometry.** The 12 forwarding components take their own
  `hit_test`, one commit per surface. Ends with `input_mouse.rs`,
  `input_mouse_dispatch.rs`, `input_mouse_panels.rs` deleted. Merge 5.4's six
  proofs into this lane — they assert exactly what it delivers.
- **5.3d-c mirrors and framework.** Delete the 29 `sync_*`, then `AppLayout`,
  `CONTEXT_STACK`, `LegacyInput`, in that order. Only tractable once (a) and
  (b) have landed; genuinely mechanical at that point.

### Sequencing

5.3d-a is independent of 5.3c and can run in parallel with it. 5.3d-b requires
5.3c (overlay hit-testing moves with the overlays). 5.3d-c requires everything.

---

## Why teardown is not mechanical, recorded once

Stage 1 was cheap because D14 deferred every hard question — a mirror answers
nothing, which is exactly why it costs nothing to add. Group 5 is where the
deferred questions come due, and they concentrate in two shapes:

1. **Reset triggers do not live in the field's own file.** `series_selection`'s
   reset was inside `move_lib_cursor`, keyed off a different field; Feeds' is
   in `feeds_manage_actions.rs`. Grepping the field never finds them. Deleting
   storage without re-homing the trigger converts a mirror guard into
   unconditional preservation — `153c9b97`, fixed by `758d0a84`.
2. **The cluster boundary assumption fails for precedence-shaped fields.** §5's
   preamble assumes a cluster's fields are read only within the cluster plus
   the shell. `album_track_focus` broke it; `confirm_modal`'s five presence
   reads break it the same way. Both are `impl App` code asking a question only
   the shell can answer.

Scoping ahead of dispatch is what makes the difference: 5.3b scoped in one
session with no stall because the code was read and the sites counted before
the work was described.
