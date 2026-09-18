## Why

The podcast tab's flat episode browser prints the episode title on every row, so an
episode list is a wall of similar text: the row under the cursor is hard to pick out,
and the parent podcast — the cue that actually distinguishes rows in a multi-show view
— recedes behind it. Revealing the episode name only on the row the user is looking at
gives the list a readable resting shape and shows the title exactly when it matters.

## What Changes

- Podcast episode rows keep their split-row identity (the parent podcast name in the
  context role) but stop painting the episode title on every row; the title appears
  only on the row that is the list's current selection.
- The selected row's full context-and-title text marquees as one long title whether or
  not it fits its title slot, so the episode name is always readable there.
- The shared media-list vocabulary gains a closed, list-level title-reveal policy:
  `Always` (every row paints its full title — today's behaviour) and
  `RevealOnSelection` (the podcast episode list's). It is declared once by the
  destination that composes the list and consumed at the single shared row-paint seam;
  no destination paints or branches rows of its own.
- The shared marquee rule is amended: a title that fits its slot is still never
  marqueed in a list that reveals titles on every row, while a list that reveals on
  selection always marquees its selected row's title.
- The marquee clock keys on the full marqueed title text (context plus item title), so
  two rows sharing a context name no longer resume each other's scroll position.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `canonical-media-lists`: lists gain a title-reveal policy for their rows, and the
  selected-row marquee requirement is extended (always marquee for a reveal-on-selection
  list) and re-keyed on the full title text.
- `audiobookshelf-podcast-browsing`: downloaded-episode rows reveal the episode title
  only on the selected row.

## Impact

- `src/app/components/media_list/` — the shared owner's title-reveal policy and its
  carrier pass-through.
- `src/app/render/components/media_list/` — the presenter passes the policy and keys
  the marquee; the row painter applies the reveal.
- `src/app/render/components/marquee.rs` — the forced-scroll window for a fitting title.
- `src/app/components/podcast_content.rs` — the one destination that opts in.
- Every other destination keeps `Always`, so Home, Queue, TV, music, books, feeds, and
  inline-search rows are unchanged.
- Root/panel composition, keyboard routing, and mouse delivery are untouched: no new
  chord, subscription, hit region, or panel slot.
