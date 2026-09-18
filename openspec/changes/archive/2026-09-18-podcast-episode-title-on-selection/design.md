## Context

See proposal.md — Why. The relevant current state:

- The shared row vocabulary (`MediaListRow::Item`) already carries an optional
  secondary title, and the row painter already paints a primary/secondary pair as a
  split row (primary in the context gold role, secondary in the split-row sage role).
- `src/app/render/components/media_list/wide.rs` holds the **only** call site of
  `media_list_row`, and every geometry — Wide and non-Wide — paints through that one
  embedded presentation over one `MediaList` owner.
- That presentation already owns the selected row's marquee clock through the list
  (`list.marquee_state(&primary)`), and it keys the clock on the row's primary text.
- `marquee_spans` returns the static parts when they fit the window; the painter only
  takes the marquee path for the selected row on a focused list when the parts overflow.
- `openspec/specs/canonical-media-lists/spec.md` pins "any row whose title already fits
  its slot" as never marqueed, with the scenario `Fitting title never marquees`.
- The podcast destination (`src/app/components/podcast_content.rs`) builds its episode
  rows as split rows whose primary is the parent podcast name and secondary the episode
  title; Home is the only other split-row producer.

## Goals / Non-Goals

**Goals:**

- The podcast episode list reads as parent podcast names at rest, with the episode title
  revealed on the row the user has selected, marqueed as one long title.
- The behaviour is one named policy consumed at the single shared row-paint seam, so no
  destination paints or branches its own rows.
- Every other list keeps its current row output byte-for-byte.

**Non-Goals:**

- Changing the podcast hero, the Queue list, or any other destination's rows.
- Changing the marquee cadence, the selected-row bar, the split-row colour roles, zebra
  striping, or hit geometry (row geometry is unchanged: no slot appears or disappears).
- Making the reveal configurable by the user.

## Decisions

**D1 — The policy lives on the shared list owner, not on each row.** A closed
`MediaListTitleReveal` policy (`Always` default, `RevealOnSelection`) is stored on
`MediaList`, exposed through both wrappers (`MediaListCarrier` for the destination that
composes the list, the embedded presentation for the painter), and read at the single
row-paint seam. Alternatives considered:

- *A field on every `MediaListRow::Item` literal.* Rejected: it restates one list-wide
  constant at every row construction site (37 today, mostly test fixtures), and the
  requirement is a property of the browser, not of one row.
- *Deriving it in the Library panel from the destination kind.* Rejected: it puts podcast
  knowledge in the panel and skips the list's own content declaration. The panel derives
  the skeleton arm (Wide/non-Wide, hero shape), not row content.
- *A bespoke podcast row painter.* Rejected outright: one painter per surface.

**D2 — Reveal follows selection; animation keeps the focus gate.** A row outside the
list's current selection paints only its primary context text — no secondary fragment and
no ellipsis standing in for it. The selection is the painter's existing `selected` bit,
which already means "cursor row or multi-selected row", so a multi-selected row reveals
exactly like the row that paints its bar. The cursor row of an *unfocused* list reveals
its full title too, truncated statically: revealing is about which row the user picked,
while the animation stays behind the existing focus gate (the list that does not hold
focus does not animate).

**D3 — A forced marquee scrolls the fitting title out of the window and back.** The
forced case reuses the existing cycle (`hold → scroll → hold → scroll back`) with the
travel set to the title's own width, so a title that fits slides fully out of the window
and returns. Alternative considered: padding the marquee text with a synthetic gap part —
rejected, it needs a colour for a part that carries no glyph and fabricates content in the
row model.

**D4 — The marquee clock keys on the full marqueed text.** Today the presenter keys the
clock on the row's primary text, so two rows that share a context name (two episodes of
one podcast) reuse one clock position, contradicting the accepted rule that the clock
restarts whenever the marqueed text changes. The key becomes the joined context-and-title
text. This is a defect the reveal makes visible — the title is now only on the selected
row — so it is fixed here rather than left to resurface.

**D5 — Only the podcast episode list opts in.** Every other destination keeps the default,
so Home's split rows, the Queue list, TV, music, books, feeds, and inline search keep their
present row output.

**D6 — The amended marquee requirement keeps its existing name.** OpenSpec matches a
MODIFIED requirement by exact header text and refuses to drop scenarios the base spec
still has; renaming it would orphan the base requirement at archive. The name is broader
than the amended text, which is recorded here rather than forced through a rename.

## Risks / Trade-offs

- [Rows carry less information at rest, so a user scanning for a specific episode reads
  one row at a time] → the Wide hero already names the selected episode, and the selected
  row reveals it in the list; the group headings still carry the age grouping. Reverting is
  one line in the destination.
- [The selected row animates even when its title fits, which can read as busy in a long
  list] → the cycle is the same bounded cadence the player strip already uses, and only
  the focused selected row animates.
- [Re-keying the marquee clock changes when other lists restart their cycle] → the new key
  makes the accepted rule ("restart whenever the marqueed text changes") true for split
  rows; the presenter's buffer tests cover the restart.
- [The policy could drift into a destination-specific painter arm over time] → the policy
  is a closed enum consumed at the one call site of the row painter; a destination that
  painted its own rows would have to add a second painter, which the presentation-test
  ownership rules and the ledger review already reject.

## Migration Plan

None: no persisted state, no protocol or config change. Rollback is flipping the
destination's declared policy back to `Always`.
