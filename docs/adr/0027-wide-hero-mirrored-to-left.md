---
status: accepted
supersedes: 0026-wide-hero-placement-pane-arrangement.md
---

# Wide Hero Mirrored To The Left

Supersedes [ADR 0026](0026-wide-hero-placement-pane-arrangement.md). The
accepted history of ADR 0026 remains intact; this ADR only mirrors the pane
order back.

## Decision

Within Wide hero the shared arrangement's two roles keep their names but swap
sides again:

- **hero** — the selected-item hero or provider-owned detail workspace, on the
  left pane.
- **browser** — the single-column Library browser and its browser-level
  pills, on the right pane.

Sizing, the shared width breakpoint, minimum-height guard, gap, minimum pane
widths, and padding all carry over unchanged from ADR 0026. Only the pane
order is mirrored again. Destination renderers still consume the role-named
geometry from `wide_hero_split` without re-splitting rectangles; only the
shared arrangement changed.

Pane focus remains browser focus versus structured-workspace focus,
independent of physical side. Enter/Esc transitions, key dispatch, typed
requests, and pointer targets do not change.

## Considered options

- **Reverse panes independently in each destination:** rejected for the same
  reason ADR 0026 rejected it — duplicates geometry and defeats the shared
  arrangement contract.

## Consequences

Wide output differs from the ADR 0026 layout only by pane order; Normal and
width-wide-but-short output are unchanged. Current source, tests, live specs,
`CONTEXT.md`, and current ADRs use only **Wide hero** and **Inline hero**;
archived OpenSpec history is not rewritten.
