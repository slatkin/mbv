## Context

See `proposal.md` - Why. Today's implementation, in the single shared row painter (`src/app/render/components/media_list/row.rs`) and the row type (`src/app/components/media_list/mod.rs`):

- `MediaListTrailing::Year(String)` paints inline, immediately after the title text, in `palette::STATUS_AVAILABLE` (green). It contributes to `trailing_w`, which shrinks the title slot.
- `MediaListTrailing::Published(String)` paints right-aligned in a fixed `DATE_GUTTER_W = 6` column, left of the duration slot, in `palette::ROW_DATE_FG` (`Palette::Iris`, whose trailing comment claims "sage" — the role has already drifted from its own documentation). It contributes to a separate `date_reserve`.
- Both paths coexist in `media_list_row` as two branches over the same `trailing: Option<MediaListTrailing>` field, so a row carries at most one already.

`media_list_row` is the only row painter for the canonical lists (one call site, in `wide.rs`), so this is a single-site rendering change fanning out through the `MediaListTrailing` construction sites.

Music is no longer one of those sites. The grouped music browser is a tree-view component with its own label renderer (`src/app/components/music_tree_label.rs`), which paints the album year right-aligned in its own `YEAR_GUTTER_WIDTH = 6` gutter in `ROW_DATE_FG`. Its Hero Workspace track list does go through the canonical painter (`music_content_workspace.rs` builds `MediaListRow`s).

The three Hero Workspace row builders — `build_episode_rows` (TV), `track_row` (music), `chapter_rows` (books) — each hard-code `duration: None` under the comment "Library lists carry no time column", which mirrors the current requirement that only the Queue list projects a duration.

## Goals / Non-Goals

**Goals:**
- One placement (a right-aligned gutter at the row's right edge) and one colour (`STATUS_AVAILABLE` green) for both years and publish dates.
- A gutter width that matches the fact it holds: four columns for a year, six for a date.
- Durations on Hero Workspace rows, in the existing precise `M:SS`/`H:MM:SS` form and the existing `DURATION` role.
- No change to `Heading`/`Spacer` rows, which carry neither gutter nor duration.

**Non-Goals:**
- Not changing the duration slot's role or format — only which lists project one.
- Not adding a generic "badge" abstraction to `MediaListTrailing`; the progress-badge case stays inline and FOAM-coloured, unchanged.
- Not changing which rows carry a year vs. a date — that's provider-specific data the call sites already decide.
- Not migrating the music tree browser onto the canonical row painter.
- Not aligning the year and date gutters to a common column across screens.

## Decisions

**Keep `Year` and `Published` as two variants, with different gutter widths.** A year is four characters; a date is at most six (`17 Sep`). Reserving six for a year wastes two columns of title on every Movies/TV/Generic row, so the two variants keep a real behavioural difference after unification — `YEAR_GUTTER_W = 4` and `DATE_GUTTER_W = 6` — and the enum keeps two names because there are two behaviours.

Alternative considered: merge both into one `MediaListTrailing::Gutter(String)` variant sharing one six-column width. Rejected per explicit user direction — it is the width that differs, and collapsing the variants would force the year to pay for the date's column budget or force the painter to guess a width from string length.

Consequence, accepted deliberately: a podcast row's gutter starts two cells left of a movie row's, so the two columns do not line up between screens. No single list mixes years and dates, so nothing misaligns *within* a list, and this is not a defect to undo later.

**Delete `ROW_DATE_FG`, point both consumers at `STATUS_AVAILABLE`.** A second name for the same colour value drifts silently — this role's own comment already describes a value it does not hold. `STATUS_AVAILABLE` carries the "positive/available metadata" meaning a date or year fits. Its two consumers are the shared row painter's gutter and `music_tree_label.rs`.

Alternative considered: keep `ROW_DATE_FG` and redefine its value. Rejected per explicit user direction — no scenario in this change needs dates to diverge from green, and the role's stated rationale (protecting dates from a `DURATION` edit) is about a colour this change doesn't touch.

**The music tree keeps its own gutter implementation, narrowed to four columns.** The duplication is authorised: the grouped music browser is a tree-view component rather than the canonical fixed-row list, and its gutter applies only at the album level of the tree, so it is not a second painter for the same surface. Its `YEAR_GUTTER_WIDTH` moves 6 → 4 for the same reason the canonical year gutter is 4, and its colour follows `ROW_DATE_FG`'s deletion. `YEAR_GUTTER_WIDTH` and the painter's `YEAR_GUTTER_W` stay separate constants belonging to separate components.

**Hero Workspace lists project a duration; browse lists still do not.** The line is the Library panel's internal split:

```text
Library panel
  +-- Browse list   (series, albums, podcasts, feeds)  --> gutter: year/date, no duration
  +-- Hero
  +-- Workspace     (episodes, tracks, chapters)       --> duration
```

A Workspace row is the playable leaf about to be started, where runtime is the fact the viewer wants; a browse row is a container being navigated, where it is noise. This carves the Workspace out of the existing "library browse list row … SHALL NOT carry one" rule rather than deleting that rule.

No painter change is needed: all three Workspace lists already render through `media_list_row`, which paints `duration` when present. The data is already in hand — Emby's `runtime_ticks` is requested for episodes and tracks, and `BookRow::Chapter{start,end}`/`AudioFile{duration}` give seconds — and `crate::app::ui_util::fmt_duration_short` is already the required `M:SS`/`H:MM:SS` form.

## Risks / Trade-offs

- [Visual regression across every browse screen] Movies/TV/Generic rows' years move from beside the title to the row's right edge, and the music tree's year shifts two cells, in one commit. → Mitigation: this is the standardisation the user asked for, grounded in the podcast screen as the reference model; `media_list_row` is the single shared painter, so no partial rollout is meaningful.
- [Existing buffer tests assert the old placement/colour] `media_list.rs`, `tests_music_characterization.rs` and `tests_music_groups.rs` assert the old inline year column, the old `ROW_DATE_FG` role, and column positions derived from `YEAR_GUTTER_WIDTH = 6`. → Mitigation: per `AGENTS.md`, update each to prove the behaviour that should now hold, rather than porting literal old assertions.
- [Title budget on Workspace rows] A Workspace row now reserves the duration slot, shrinking its title. Workspace rows carry no trailing gutter today, so the two reserves do not stack in practice — but they would if a year were ever added to a track row, which at Narrow widths would cost ~14 columns.

## Migration Plan

No data/persistence migration — a rendering and row-model change confined to one process's in-memory state. Steps: painter widths and colour, palette deletion, music tree constant and colour, the three Workspace duration fields, tests, then `cargo fmt`/`clippy`/`nextest`. No feature flag — `AGENTS.md` avoids compatibility shims for a single-user app with no external consumers of these types.
