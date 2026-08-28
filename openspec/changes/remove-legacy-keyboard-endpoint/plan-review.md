# Plan review — complexity the design/tasks did not surface

Author: orchestrator review, 2026-08-28. Base `23466ac8`.
Prompted by: units U2 and U3 both looked over-scoped/mis-sequenced on
dispatch; user flagged the plan as problematic "in more ways than one."

This is a read-only review of the *plan*, grounded in the TuiRealm 4.1
source and the current code. It is not an implementation.

## 0. The headline

The design frames the remaining work as a routing/ownership change with a
"serial, compile-complete families" path. That path is technically possible,
but it rests on a property of TuiRealm that the design assumes rather than
proves, and the unit breakdown treats that property as free. It is not free;
it is the bulk of the risk. Several stated premises are also factually wrong
against the current code. Before any more units dispatch, the design needs a
revision pass (this is an `openspec-explore` situation, not an apply).

## 1. TuiRealm 4.1 delivery semantics (verified from crate source)

From `tuirealm-4.1.0/src/core/application.rs::tick`:

```rust
let mut messages = events.iter()
    .filter_map(|x| self.forward_to_active_component(x))   // channel A: focused
    .collect();
if !self.sub_lock {
    self.forward_to_subscriptions(&events, &mut messages); // channel B: subs
}
```

And `forward_to_subscriptions`:

```rust
for sub in &self.subs {
    if self.view.has_focus(sub.target()) { continue; }      // skip focused
    if !sub.forward(ev, ...) { continue; }                   // EventClause + SubClause
    if let Some(msg) = self.view.forward(sub.target(), ev)... { messages.push(msg); }
}
```

Three load-bearing facts:

1. **Dual delivery, fan-out.** Every keyboard event goes to the focused
   component (channel A) AND, separately, to every subscription whose
   `EventClause` matches and whose `SubClause` guard passes (channel B). These
   are independent channels.
2. **Subscriptions skip the focused owner.** A subscription only fires when its
   target is *not* focused. The focused owner always gets the event via channel A.
3. **All eligible subscriptions fire.** No first-match-wins among subscriptions.
   No short-circuit. Every subscription whose guard passes pushes a `Msg`.

`EventClause::Keyboard(KeyEvent)` matches a *specific* chord exactly;
`EventClause::Any` matches everything. `SubClause` guards can inspect
`HasAttrValue`, `HasState`, `IsMounted`, and boolean combos — **but never the
key itself.** Per-key eligibility must be expressed as one subscription *per
chord*, not one subscription with a key-aware guard.

## 2. The semantic mismatch the plan hides

`CONTEXT_STACK` is **first-match-wins** with 11 ordered layers. TuiRealm
subscriptions are **fan-out (all-eligible-fire)**. The design's claim that
"mutually exclusive runtime gates" replicate first-match-wins is *technically
true but operationally the hardest part of the change*, and the tasks treat it
as a wiring detail. Concretely:

- **Blocking-overlay exclusion must be on EVERY subscription, not just
  `global_overlay_open`.** Today only that one entry carries
  `NotHasAttrValue(Playback, ATTR_BLOCKING_OVERLAY_ACTIVE, true)`. The parent
  bindings (`queue_column_width`, `panel_mode_cycle`, `clear_queue_prompt_c`,
  `visualizer`, `playback`) and the always-globals (`ctrl_l`, `f5`) must ALL
  carry the same exclusion or they double-fire while a modal is focused. The
  `ATTR_BLOCKING_OVERLAY_ACTIVE` attribute must also be kept in sync by the
  shell on every mount/dismiss — currently it is not the single source of truth
  for "any blocking overlay is up" across all subs.

- **Per-key gates cannot be SubClause guards.** `playback` is a per-key command
  table (`resolve_key` + player/remote eligibility). To express this in TuiRealm
  you install ONE subscription PER playback chord (`Space`, `Esc`, `k`, `j`,
  `a`, …), each with `EventClause::Keyboard(<that chord>)` and a guard on
  `HasAttrValue(Playback, "player_active", …)` etc. That is a large, fragile
  fan-out of subscriptions plus shell-side attribute mirroring — not "the
  playback table entry."

- **Legacy `FallThrough` ≠ TuiRealm `None`.** In the legacy loop, a context
  *declining* a key means the next-lower context sees it. In TuiRealm, the
  active component returning `None` does *not* redirect to a subscription —
  subscriptions fire from the independent channel B regardless. So "Space falls
  through on the first press" is preserved only because *no other owner claims
  Space*. Every converted leaf must be audited to ensure its `None`-returning
  keys are genuinely unclaimed elsewhere, or behavior silently changes. The
  routing matrix is the only thing that catches this, and we just folded it
  into per-family rows that don't exist yet.

- **The active component always fires.** For a global like `q` to quit and not
  double-act, every focused leaf must *explicitly ignore* `q`/`Tab`/`1-9` in its
  `on()` (return `None`) so only the UiRoot `q` subscription acts. Today those
  leaves emit `GlobalViewKey(q)` to delegate. After conversion they must each be
  touched to return `None` for globals. That is a per-component change across
  6+ leaf components — real fan-out the plan splits into U11–U13 but doesn't
  flag as the risk center.

## 3. Stated premises that are factually wrong against the code

- **"Two DIRECT `handle_key_with_home_context` call sites in `shell_home.rs`
  that bypass `handle_legacy_key` entirely" (design Context + task 1.1).**
  FALSE. `shell_home.rs:655` and `:701` are inside `#[cfg(test)] mod tests`
  (starts at `:232`). The only production call sites are `shell.rs:141` (inside
  `handle_legacy_key`) and `input.rs:77` (the no-arg `handle_key` wrapper).
  The U1 inventory caught this; the design didn't. This *simplifies* the
  deletion scope (U14) but means the design's "deeper entanglement" framing
  was partly built on a misread.

- **"Converted media components still emit `GlobalViewKey`, so
  `handle_legacy_key`... remain reachable by construction" (design Context).**
  TRUE, but understated. `GlobalViewKey` is the *only* legacy endpoint for
  leaf-focused keys: `route_terminal_observer_message` *drops*
  `TerminalObserverEvent::Key` when `focused != UiRoot`
  (`shell.rs:86-93`). So the legacy loop is reached exclusively via
  `GlobalViewKey` for leaf focus, and via the UiRoot observer only when UiRoot
  itself is focused. The conversion must eliminate `GlobalViewKey` emitters in
  6 components (home, browser, tv_workspace, music_workspace,
  audiobookshelf_book, audiobookshelf_podcast) — each a separate fan-out unit.

- **D7 / `ComponentId::Library` owner (design + key_policy.rs).** Confirmed:
  no `LibraryComponent` is ever mounted (`vec![]` subs everywhere except
  `root.rs`). `panel_mode_cycle_x` is owned by `ComponentId::Library` in the
  shadow table but nothing is mounted there. The design says "leave the
  separate Library-parent decision to #607" — but `panel_mode_cycle_x` (`x`
  cycle) currently runs for *all* destinations via `handle_key_panel_mode_cycle`
  in `CONTEXT_STACK`, not a mounted Library. Routing it to an unmounted owner
  means it never fires. This is an unresolved routing gap, not a deferred
  discrepancy.

## 4. The subscription model is greenfield in this codebase

Every component is mounted with `vec![]` (no subscriptions) except UiRoot,
which has one `Sub::new(EventClause::Any, SubClause::Always)`. The design
proposes installing per-chord/per-owner subscriptions with boolean-combo guards
across ~11 precedence layers — a capability TuiRealm supports but this codebase
has **never exercised**. The plan treats "install subscriptions" as a known
quantity (U3 was "activate key_policy subscriptions"). It is a greenfield
integration with its own unknowns; it needs a proof-of-concept spike before it
gets sequenced into 16 units.

## 5. Why U2 and U3 looked wrong (the sequencing symptom)

- **U2 (routing matrix)** was big because the matrix is the *only* thing that
  can catch the fan-out regressions in §2 — and it has to exist before the
  conversions, not after. Folding it into per-family rows (current decision)
  means the first conversion unit has no guardrail.

- **U3 (activate subscriptions)** was mis-sequenced because installing live
  subscriptions *before* the owners interpret their keys locally produces either
  inert scaffolding (subs that do nothing until U4/U5/U10) or double-action
  (subs + legacy `CONTEXT_STACK` both firing). The subscription installation
  can only land *with* its owning family, never before.

Both symptoms share one root cause: **the plan sequences by task-number order,
but the safe sequence is by precedence layer, and each layer is a
{component-local-interpretation + subscription-install + CONTEXT_STACK-entry-
removal + matrix-row} bundle that must land together.** Splitting any of those
four across units creates the exact double-action/inert/unguarded states the
matrix exists to forbid.

## 6. What a sound plan probably looks like (sketch, not a commitment)

The atomic unit is not "task 2.1" or "the Queue family." It is **one precedence
layer, fully converted top-to-bottom**:

  (a) the owning component(s) interpret their keys locally in `on()`;
  (b) their `GlobalViewKey`/`*Key` emitter + `to_crossterm` call deleted;
  (c) the matching `CONTEXT_STACK` entry removed or neutered;
  (d) the corresponding TuiRealm subscription(s) installed with correct guards,
      including blocking-overlay exclusion;
  (e) shell mirrors any dynamic state (player_active, prompt flags, panel mode)
      into component attributes the guards read;
  (f) the routing-matrix row(s) for that layer added and green.

Layers, roughest-to-safest order (each gated on the matrix being green first):

1. **Always-globals on UiRoot** (`ctrl_l`, `f5`, plus F1 help, plus
   `q`/`Tab`/`1-9` if those stay global) — smallest, purest subscription
   proof. Births the matrix. Resolves the D7 `panel_mode_cycle_x` routing gap
   (assign `x` to UiRoot, not the unmounted Library).
2. **Blocking overlays** (confirm/daemon_lost/remote_reanchor/context_menu/
   playback_prompt/save_playlist) — convert to accept/cancel/move/submit
   intents; their swallow-by-focus replaces the legacy ordering. Proves the
   `NotHasAttrValue(ATTR_BLOCKING_OVERLAY_ACTIVE)` exclusion.
3. **Playback** (per-chord subs + attribute mirror + double-tap state in
   component) — the hardest dynamic gate; do it once the overlay exclusion is
   proven.
4. **Queue** (`queue_column_width` correct gate + clear-queue + queue-local
   nav + `QueueKey` deletion).
5. **Library destinations** (`.` to focused leaf + target resolution moved into
   each component; `GlobalViewKey` deletion across the 6 leaves).
6. **Global teardown** — delete `CONTEXT_STACK`, `handle_legacy_key`,
   `handle_key_with_home_context`, `typed_key.rs`, raw `*Key` variants, static
   policy scaffolding; replace 5 blanket pushes with targeted pushes.
7. **Architecture gate + final verification.**

This is ~6-8 real units, each one precedence layer, each landing (a)-(f)
together. It is still a large change, but each unit is independently
verifiable and the matrix grows with it.

## 7. What I need from the maintainer before dispatching more units

1. **Acknowledge or reject the fan-out framing.** If the design's "mutually
   exclusive gates" is still the chosen path, it needs a revision pass that
   spells out the per-chord subscription fan-out for playback and the
   blocking-overlay exclusion on every sub — both absent today.
2. **Resolve the D7 `panel_mode_cycle_x` / `ComponentId::Library` routing
   gap** — assign `x` to UiRoot, or define where it lives. The current table
   routes it to an unmounted owner.
3. **Decide the matrix-first sequencing.** I folded the matrix into per-family
   rows on your call; given §2, I think one shared matrix file born at unit 1
   and extended each layer is the safer read. Your call.
4. **Confirm the `shell_home.rs` "direct call sites" correction** so the design
   and task 1.1 stop asserting a production bypass that doesn't exist.
5. **Greenlight a small TuiRealm subscription spike** (install one real
   per-chord subscription with a `HasAttrValue` guard, prove single-action via
   the matrix) before sequencing the dynamic layers. The subscription model is
   greenfield here; a 1-hour spike de-risks U3+.

Until 1-4 land, I should not dispatch further implementation units. U1 (the
inventory) stands; everything downstream of it is reopened.