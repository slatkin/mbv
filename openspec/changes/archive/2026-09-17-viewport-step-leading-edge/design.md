## Context

The shared media-list owner moves its visible window and drags the selection into it when a step would
otherwise leave the selection outside (`MediaList::scroll_viewport` → `drag_selection_into_window`,
`src/app/components/media_list/mod.rs`). The archived `media-list-viewport-scroll` change specified that
drag as "the nearest selectable row the window shows", which mechanically resolves to the window edge the
selection just left: stepping up from the window's last row parks the cursor on the new window's last row,
stepping down from the window's first row parks it on the new window's first row. Both the wheel and the
keyboard verbs (`Ctrl+e`/`Ctrl+y` one row, `PgUp`/`PgDn` a painted height) route through that one method,
so the landing row is a single, shared decision.

## Goals / Non-Goals

**Goals**

- The selection travels with the gesture: after a step, it sits on the row the step brought into the
  window at the leading edge, so holding the verb keeps the cursor moving rather than pinning it to the row
  it is leaving.
- One rule for every input that steps the window; no per-surface landing behavior.

**Non-Goals**

- Changing when a step drags at all (the step still moves the selection only when it would leave the
  window). A step whose window still contains the selection keeps moving only the window, which is what
  stops the wheel and the chords from hijacking the selection.
- The cursor path (`↑`/`↓`, `j`/`k`, `Home`/`End`) and its leading-context rule.
- The wheel's direction mapping, the chord bindings, the page height, and the read-only paint.

## Decisions

### D1 A step drags the selection to the leading edge of its direction

When a step would put the selection outside the window, the drag resolves against the **step direction**:
a step toward the preceding display row drags to the first selectable row of the new window, and a step
toward the following display row drags to the last selectable row of the new window. The resolution stays
selectable-only (a `Heading`/`Spacer` run is skipped to the nearest selectable row at the leading edge) and
falls back to leaving the selection untouched when the new window shows no selectable row.

Rationale: this is what the gesture conventions already teach. Vim's `Ctrl+e`/`Ctrl+y` scroll the window and
carry the cursor with them; editors place the caret on the first or last line of the newly paged view for
`PgUp`/`PgDn`. The previous nearest-row rule inverted that, which a keyboard-dominant list feels
immediately: the cursor rides the edge it is leaving and never advances relative to the screen while the
verb is held.

This decision is not local to the change. It is the standing rule recorded as **Invariant 6**
(`docs/invariants/06-viewport-step-leading-edge.md`): no edge-riding scroll may be implemented in this
application again, in any surface, and new scrollable surfaces are in scope from the day they are written.
The rule is also stated in `.agents/skills/mbv-frontend/SKILL.md`, the gate every TUI change passes
through.

Alternative considered: keep the nearest-row landing (the archived change's D3) and let the cursor keys be
the only way to travel. Rejected — it makes the only chord named after scrolling behave as a selection
brake, and the reported symptom ("scrolling back up pins the cursor to the bottom row") is that behavior.

### D2 The leading-edge rule is the trailing-edge rule's exact inverse, not a second policy

Only the drag target's resolution changes; the conditions around it are untouched. A step still drags only
when the selection would fall outside; a step that keeps the selection inside still moves the window alone;
the window still clamps to `[0, total - height]`; the page form still steps by the painted height; no
display-row index crosses a row-flow replacement. The change is therefore visible only where a drag fires.

### D3 This invalidates the archived change's D3 landing clause, and the rule outlives the change

`media-list-viewport-scroll` (archived 2026-09-17) states the nearest-shown-row landing in its design D3,
its tasks, and the spec clause this change modifies. The rest of that change's decisions stand: the single
writer (its D2), the cursor leading-context rule (D4), the row-flow anchor (D5), the page height (D6), the
chords (D7), the echo split (D8), and the retired hand-patches (D9). Recorded as an explicit invalidation
of the landing clause only.

## Risks / Trade-offs

- [A step can now move the selection a whole page when the drag fires] → that is the intent: the verb is a
  scroll, and the selection lands where the new view begins. Holding the verb then advances it one row per
  step at the leading edge, so the jump happens once per direction change, not per press.
- [A one-row step from the trailing edge jumps the cursor across the window] → the alternative (a gradual
  walk) is what the cursor keys already do; keeping the steps distinct preserves "the wheel/chord scrolls,
  the cursor keys walk".
- [Per-surface tests pin the old landing] → they are updated in the same slice; the ledger and spec
  verification records name the leading-edge landing.

## Migration Plan

1. Owner rule and owner unit tests (both directions, structural rows, empty-window fallback, content-end
   clamps).
2. Per-surface drag-at-edge assertions and the tick-integration viewport-step evidence.
3. Full gates, then archive and sync the deltas into the main specs.

Rollback is the commit revert; no persisted format, protocol, or configuration change.

## Open Questions

(none)
