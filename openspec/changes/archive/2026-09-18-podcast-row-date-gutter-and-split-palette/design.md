## Context

See proposal.md — Why. The relevant current state:

- The row painter (`render/components/media_list/row.rs`) already lays out a left-aligned
  metadata slot (`MediaListTrailing::Year`, green) and a right-aligned duration slot
  (`DURATION`, Queue-only). The podcast episode rows project neither.
- The split-row palette paints the primary part with `PLAYBACK_CONTEXT_FG` — the same role
  the playback strips use for their own context part — and the secondary part with
  `SPLIT_ROW_TITLE_FG`. Only `podcast_content.rs` and Home's episode rows project a
  secondary title.
- `fmt_publish_date` renders the hero meta row's `19 Jun 2015`.
- The episode browse state carries `published_at: Option<u64>`.

## Goals / Non-Goals

**Goals:**

- A publish-date column that lines up across rows and never collides with the title.
- Quieter split-row title colours for browse lists, without touching the playback strips.
- One closed vocabulary: a destination declares what a row carries, never where or how it
  paints.

**Non-Goals:**

- Changing the hero's date format, the Queue's duration column, or any other destination's
  rows.
- Making the gutter width or the colours user-configurable.
- Changing row height, hit geometry, or the reveal-on-selection title policy.

## Decisions

**D1 — The publish date rides the existing metadata slot as a variant, not a new field.**
`MediaListTrailing::Published(String)` joins `Year(String)`; each variant carries its own
placement and role, and the painter reserves and paints the gutter only when a row carries
one. Alternative considered: a new `date: Option<String>` field on every `MediaListRow::Item`
literal — rejected: it restates a new `None` at every row construction site (37 today,
mostly test fixtures) to express something only one destination ever sets, and the existing
slot's own doc already states that its variants carry their role.

**D2 — The gutter is a fixed-width, right-aligned column.** The column is
`DATE_GUTTER_W = 6` columns for every dated row, whatever the date string's own length is,
so the dates line up on one edge and the title's slot shrinks by exactly that much. The
title budget reserves `QUIET_GAP + DATE_GUTTER_W` when a row carries a date, so no column
collides. The date sits left of the duration when a row carries both (no row does today).

**D3 — The format is day plus abbreviated month, no year.** `17 Sep`, unpadded day, so a
single-digit day right-aligns inside the same six columns. A year would either widen the
column or make the format inconsistent; the publish date's job here is telling two episodes
of one show apart. `fmt_publish_date_short` is the gutter's formatter beside the hero's
`fmt_publish_date`, which keeps its four-digit year.

**D4 — The split-row palette moves to its own roles rather than editing the shared playback
roles.** `SPLIT_ROW_CONTEXT_FG` (soft white) replaces the list row's use of
`PLAYBACK_CONTEXT_FG`, and `SPLIT_ROW_TITLE_FG` becomes the light grey (`SUBTLE`) primitive.
The playback strips keep `PLAYBACK_CONTEXT_FG`/`PLAYBACK_TITLE_FG`, so the strip's context
stays gold and its title stays aqua — the accepted spec scenario "the now-playing playback
strip's title roles are unaffected". The list palette stays shared by every split-row
producer (the podcast list and Home's episode rows), with no per-list opt-in.

**D5 — The date role is its own (`ROW_DATE_FG`, the yellow primitive).** Its own role rather
than `DURATION` or `STATUS_AVAILABLE`, so a duration-colour or status-colour edit cannot move
the dates.

## Risks / Trade-offs

- [A light-grey item title is dimmer than the sage it replaces, so the marqueed episode title
  reads as secondary] → that is the intent: the podcast name is the row's resting identity and
  the title is the revealed detail. The played state still mutes the title to `TEXT_MUTED`.
- [Home's episode rows inherit the new split palette] → the palette is shared by rule ("no
  per-list opt-in"); the change is deliberate and recorded here rather than forked per list.
- [Six columns is fixed, so a future format change resizes the column] → one constant
  (`DATE_GUTTER_W`) plus the formatter's own test; no row reserves a length it does not have.
- [A nonsense timestamp renders the epoch's date rather than nothing] → pre-existing
  saturation behaviour of the shared formatter, pinned by a test rather than changed here.

## Migration Plan

None: no persisted state, no protocol or config change. Rollback is dropping the variant from
the episode row projection (the gutter then reserves nothing).
