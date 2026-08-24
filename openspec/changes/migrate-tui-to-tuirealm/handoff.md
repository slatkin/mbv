# Handoff: migrate-tui-to-tuirealm — precedence-gate infrastructure + browser scoping

**Date:** 2026-08-24 (two sessions same day: precedence gates, then 3.3/3.5 scoping)
**Progress:** 17/40 tasks marked complete in `tasks.md` (unchanged this
session — this work is prerequisite scaffolding for task 3.4's input half,
not one of the 40 named tasks itself).
**Schema:** spec-driven
**Branch/worktree:** `.worktrees/migrate-tui-to-tuirealm`

## What happened this session

Confirmed with the user: task 3.4's checkbox stays unticked until Home's
input is live (matches the state the prior session already left it in — no
`tasks.md` change needed).

Investigated what finishing 3.4's input actually requires. `key_policy.rs`'s
`KEY_POLICY` table is pure documentation (`#![allow(dead_code)]`, zero
callers) — none of its `SubClause` guards are wired. Home's legacy handler
only ever receives a key after all 16 higher-precedence `CONTEXT_STACK`
entries have declined it, so pre-empting Home safely requires those entries'
eligibility to be expressible as TuiRealm guards first.

Read the actual handler code for the 5 entries `KEY_POLICY` marked
`Custom("prompt visible")`/`Custom("visualizer active")`/etc. Only two turned
out to be genuinely state-gated:

- `confirm_skip_intro` — real gate: `App.skip_intro_end_ticks.is_some()`.
- `confirm_next_up` — real gate: `App.next_up_item.is_some()`.

The other three were **mis-described** in the existing table (now fixed):

- `clear_queue_prompt_c` — actually an unconditional 'c' key-match (no Alt),
  not gated on any prompt-visible flag. Now `KeyPolicyGate::Always`.
- `visualizer` — actually an unconditional 'v' key-match; doesn't require
  the visualizer to already be active. Now `KeyPolicyGate::Always`.
- `playback` (`handle_playback_key`) — resolves eligibility **per-key** via
  `resolve_key(InputContext::Playback, snapshot, chord)`, a command table.
  This **cannot** reduce to a static attribute check; documented inline in
  `key_policy.rs` as a known limit for whoever wires Home/other surfaces
  next.

## What's built and verified

- `src/app/components/playback_gates.rs` — `PlaybackGatesComponent`, a
  minimal `Props`-backed attribute carrier (not the real Playback surface —
  that's task 4.10) mounted at `ComponentId::Playback` for the whole
  session. Exposes `ATTR_SKIP_INTRO_PROMPT_VISIBLE` /
  `ATTR_NEXT_UP_PROMPT_VISIBLE` as real `Attribute::Custom` values a future
  `SubClause::HasAttrValue` guard can read. Two tests: default-false, and a
  full round-trip through a real `Application` registry (not just the bare
  struct).
- `src/app/shell_gates.rs` — `Model::sync_precedence_gates`, called every
  tick alongside the other `sync_*` calls in `shell.rs`. Mirrors
  `App.skip_intro_end_ticks`/`App.next_up_item` into the two attributes via
  `Application::attr` (not the downcast+setter pattern `sync_home` uses,
  since this component is never rendered/downcast).
- `key_policy.rs` — the two real gates documented precisely; the two
  mis-described entries corrected to `Always`; `playback`'s irreducibility
  documented inline.

**Verification (all clean):**
```
rtk cargo check -p mbv --all-targets   → 0 errors, 1 warning (pre-existing, unrelated)
rtk cargo nextest run -p mbv           → 1150 passed (1148 + 2 new)
rtk cargo fmt --all -- --check         → clean
rtk cargo clippy -p mbv --all-targets  → 0 errors, 3 warnings (all pre-existing, unrelated)
rtk ast-grep scan                      → 71 errors, unchanged from before this session,
                                          all in screens/root.rs, queue.rs, pills.rs
rtk make check-code-file-lines         → all governed files ≤ 800 lines
```

## What this does NOT do

Home's keyboard/mouse input is **still** on the legacy path, unchanged.
Nothing subscribes to the new attributes yet — this session built the
plumbing, not a consumer. Wiring Home live still needs, at minimum:

- A decision on mechanism: make Home TuiRealm-`active()` when it's the
  focused destination (matching how Confirm/Sessions/Search already work —
  `forward_to_active_component` is the only channel with real first-match
  semantics; `forward_to_subscriptions` calls *every* matching subscriber
  independently, so a passive `Sub`-based approach needs fully
  mutually-exclusive guards to avoid double-firing) vs. a subscription-based
  approach.
- A concrete answer for the unconditional-key-match entries above Home in
  precedence (`global_overlay_open`, `queue_column_width`, `lib_search`,
  `panel_mode_cycle_x`, `ctrl_l_force_clear`, `f5_refresh`, `album_track_mode`)
  — which specific keys each claims, so Home's handler can defer to them.
- A resolution for `playback`'s per-key irreducibility (see above) — the
  hardest remaining piece, since it can't be reduced to a static gate at all.

## Session 2: scoping tasks 3.3 + 3.5 for parallel agents

A separate agent was assigned task 3.4 (Home input); this session scoped 3.3
(Inline Search) and 3.5 (Emby generic/Movies/home-video browser) into
agent-sized briefs, since the task-list descriptions understate how
entangled they are with each other and with the not-yet-started 4.2
(TV)/4.3 (Music)/4.4 (album-track) tasks. Full analysis and the resulting
task breakdown: `openspec/changes/migrate-tui-to-tuirealm/scoping-3.3-3.5.md`.

Landed directly (small, mechanical, user chose to do it inline rather than
hand it to an agent): **3.5a**, defining `BrowserKind` and giving
`BrowserKey` real fields (`{ service: ServiceKind, library_id: String, kind:
BrowserKind }`), replacing the unit-struct placeholder. This unblocks both
3.3's and 3.5's `ComponentId` variants (`InlineSearch(BrowserKey)` /
`Browser(BrowserKey)`), which previously couldn't be constructed with real
data. See the scoping doc's "3.5a — DONE" section for the full account,
including a real design snag it surfaced in `key_policy.rs` (no single
static `ComponentId` can represent "the" `InlineSearch`/`Browser` instance,
since one exists per browser tab — left as `Active(None)` + a documented
`Custom` gate, not resolved).

Verification (all clean): `rtk cargo check -p mbv --all-targets` (0 errors),
`rtk cargo nextest run -p mbv` (1152 passed, up from 1150), `rtk cargo fmt
--all -- --check` (clean), `rtk cargo clippy -p mbv --all-targets` (0
errors, 3 pre-existing warnings only), `rtk ast-grep scan` (71 pre-existing
errors, unchanged), `rtk make check-code-file-lines` (clean).

Not started: 3.5b/3.5c (render+input seam extraction, then conversion — the
seam-extraction step was scoped **broad**, covering all four
`handle_key_emby_library`/`list.rs` concerns at once per user choice, not
just the generic/Movies/home-video path 3.5 itself needs), 3.5's
input/action lane (`library_load_actions.rs`, `lib_event_actions.rs` — the
790-line `lib_event_actions.rs` was not read this session), and 3.3 itself.
`tasks.md` is unchanged — no task's checkbox reflects this session's work,
since 3.5a isn't one of the 40 named tasks (same footing as the earlier
precedence-gate work).

## Prior history (task 3.4, render half — 2026-08-23)

Home's **rendering** runs through a mounted `HomeComponent`
(`src/app/components/home.rs`), sharing one orchestration function with the
legacy `App::render_home_list` path so they can't drift. Full detail (files
touched, preserved quirks — the dead `.` key, Home's two independent cursor
systems, mouse-wheel behavior) is in
`/tmp/opencode/migrate-tui-to-tuirealm-handoff-2026-08-23-home-render.md`.

Tasks 2.1–2.5, 3.1–3.2, 3.7 were completed in earlier sessions. Task 3.3
(inline library Search) and 3.5 (Emby browser, `BrowserKey`) remain
explicitly deferred — 3.5 must stay last among the currently-visible options
per a standing user constraint.
