## Context

The archived change `unify-surface-colour` (2026-09-11, PR #689) proved the
mechanics: a closed 34-identity `Surface` table, one resolver, all ~66
production paint sites migrated, two ast-grep guardrails, a conformance test.
It also replaced every site's own focus input with a frame-level `FocusState`
(its sections 1 and 3, its row 4.6, and its declined rework 7.1–7.8). That
normalisation changed which surfaces react to focus and when — the visible
change that got PR #689's rework declined. This change keeps the mechanics and
locks the behaviour to main's. See proposal.md — Why.

Facts established against main that this design relies on:

- main already reacts to focus at ~35 production sites, through per-site
  signals: the shared lever `resolve_surface_focus(bit)`, direct
  `SURFACE_FOCUSED` pins, a pane's `LeftPaneFocus::Workspace(held)` /
  `ReadOnly` match (`render/arrangements/wide_hero.rs:303`), the
  `queue_focused` backdrop bool (`render/components/chrome.rs:33`), each
  component's own `focused` bool, and the `SelectedRowSurface` policy
  (`render/components/media_list/wide.rs:203`).
- Only a handful of sites derive their bit from a cursor rather than focus:
  `tv_wide.rs` (~576: `ctx.focused && episode_cursor.is_some()`),
  `card.rs:245`, and the ABS podcast / Music books highlight gates.
- The two highlight-gating fixes on the declined branch (a114ac17, 763aca18)
  are NOT in main; main's ungated behaviour is what this change migrates.

## Goals / Non-Goals

**Goals:**

- The archived change's end state for colour *identity*: every painter names a
  `Surface`; roles are unreachable from screens; level edits reach every row of
  a level; the guardrails and conformance test hold.
- Bit-for-bit rendered neutrality against main, *including* when each surface
  reacts to focus — proven, not asserted.
- Zero edits to main's existing test files.

**Non-Goals:**

- No `FocusState`, no `panel_focus_state.rs`, no shell/layout/sync changes.
- No surface becomes more or less reactive to focus; no value moves.
- No port of the declined rework's behaviours (hero panes, gutters, flat
  selected row, inset recesses, soft-white overview text) or of the two
  highlight-gating fixes — each is a separate, visible change if wanted.

## Decisions

**D1 — One resolver takes the site's own bit.**
`surface_colors(surface, focused: bool) -> SurfaceColors`, single entry point.
The bool is whatever the call site already computes in main — component focus,
a `held` bit, a Panel focus bool, or a cursor predicate. Rejected: the archived
`&FocusState` shape (it *is* the behaviour change); a `FocusState` extended
with per-site facts (contradicts the archived design's own "only what a colour
depends on" rule and re-creates the normalisation risk). The archived branch
itself already admitted this seam as `surface_colors_for_column_focus(own_
column_focused)`; here it is the only entry point, documented honestly: the
table governs colour *identity*, not the bit's provenance, and the guardrail
polices names, not bits.

**D2 — Row derivation rule.**
For each surface: focused fill = the value main's site(s) paint when their bit
is true; resting fill = when false; fixed sites (modals, always-dark backdrops)
pin focused == resting. Values are main's existing role values — the lever's
two arms, `SURFACE_FOCUSED`, `SURFACE_RESTING`, `SURFACE_BACKDROP`,
`SURFACE_CHROME`, the recess and soft-body values. The 34-identity set is the
archived 2.1 inventory re-walked against main (including `QueueCardVisualizer`
at content body and the context menu's selected row at `ACCENT_ACTIVE`).

**D3 — The special sites, migrated individually.**
(a) Modal and fixed-fill sites: focused == resting rows. (b) The
`SelectedRowSurface` policy family: stays; each variant maps to its declared
selected-row identity. (c) The cursor-driven sites above: keep their predicate
verbatim as the bool. (d) The hero-pane match: the call site collapses
`ReadOnly` / `Workspace(held)` to the one bool it effectively computes today.
No row in the table may branch on anything but its bool.

**D4 — Guardrail text, adjusted.**
The archived `no-surface-role-in-painters` rule banned role names *and*
`resolve_surface_*` calls in painters. Here `surface_colors` is the permitted
resolver, so the rule bans role names and any resolver that is not it, with the
same fixtures-plus-probe verification. `no-role-background-in-painters` ports
as-is.

**D5 — Neutrality is the acceptance bar, mechanically checked.**
Three proofs, all required: (1) `git diff origin/main` shows zero edits inside
existing test files — new test modules (table pins, conformance) are the only
test additions; main's buffer expectations pass byte-identical, which pins
today's values and today's reactivity; (2) the Rgb literal multiset across
`src/` is identical to main's, count deltas permitted only for the purpose-
named primitive splits (same bytes, two names); (3) the conformance test pins
each locatable region to `surface_colors(surface, site_bit).fill` in both bool
states.

**D6 — What transfers from the archived branch.**
Cherry-pick candidates: the table macro/skeleton, the two guardrail rules with
fixtures, the conformance harness shape, and the role-retirement / primitive-
split substance of its rows 4.3 and 5.1. Re-derived: every row's focused value
per main (D2), every call-site diff's focus-input line (D1/D3). Dropped:
`FocusState`, `panel_focus_state.rs`, the archived sections 1 and 3, row 4.6,
and rework 7.1–7.8. Expect patch conflicts where the declined branch rewrote
the same files (theme modules, media_list tests); they are resolved toward
main, never toward the declined tree.

**D7 — Names.**
Surface identity names are the archived change's, verbatim — they are the
reviewed vocabulary and the design.md mapping table there matches them.
CONTEXT.md's "Panel focus" term is used for the Library/Queue navigation
concept; the resolver's bool is not that concept and is not named as if it
were.

## Risks / Trade-offs

- **The bool seam weakens "one input".** A cursor predicate can still drive a
  colour, as it can in main today. Accepted for neutrality; normalising those
  sites is a visible change, available later as its own proposal (the PR #689
  variant remains the reference).
- **Row values are re-derived by hand.** Mitigated by D5's three proofs —
  main's untouched buffer tests pin both the values and the reactivity, so a
  wrong derivation fails a test, not a user's eye.
- **Cherry-pick debt from the declined branch.** Same-file rewrites guarantee
  some conflicts; resolved toward main. Budgeted as part of each unit, not a
  separate phase.
- **The known load-sensitive SIGABRT flake** (recorded in the archived
  change's residuals) may fire under parallel full-suite runs; pre-existing,
  passes in isolation.
