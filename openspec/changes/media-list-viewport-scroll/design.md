## Context

`MediaListCore::resolve_viewport` (`src/app/components/media_list/mod.rs`) computes the painted
window each frame from the stored scroll: it clamps the offset to the content, then keeps the
selected display row on screen. The raise branch sets `offset = row` — exactly to the selection's
row. On a grouped list the selection's first stop on the way back up is the first selectable row of
a group, so the raise lands one row below that group's `Heading` and the label never re-enters the
window. `Home` on a grouped list has the same effect.

## Decision

Extend only the raise branch: after `offset = row`, walk `offset` upward while the row above
(`rows[offset - 1]`) is non-selectable (`selectable_target().is_none()`), clamped at row 0. The
walk necessarily stops at the previous group's last selectable row, so the window shows exactly the
label of the group containing the selection — never the previous group's rows.

Everything else is unchanged: the bottom branch (`row >= offset + height`), the stored scroll, the
painter's `set_scroll` write-back, restore/hand-off anchoring, and every input binding (wheel,
`PgUp`/`PgDn`, cursor chords, `Home`/`End`).

## Alternatives rejected

- Viewport-step model (window moves independently of the selection and drags it along): implemented
  on `feat/media-list-viewport-scroll`, abandoned unmerged — it changed three input semantics to fix
  one keyboard symptom, was non-standard for a TUI, and needed its own follow-up change to fix the
  drag-edge landing it introduced.
- Cursor stops on `Heading` rows: changes what `↑` does on every list.
- Top-of-list special case: fixes only the list's first group; every other group's label disappears
  mid-list by the same rule.
