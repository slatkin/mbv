## Context

See `proposal.md` - Why. Today's implementation, in the single shared row painter (`src/app/render/components/media_list/row.rs`) and the row type (`src/app/components/media_list/mod.rs`):

- `MediaListTrailing::Year(String)` paints inline, immediately after the title text, in `palette::STATUS_AVAILABLE` (green). It contributes to `trailing_w`, which shrinks the title slot from the left side of the trailing/duration block.
- `MediaListTrailing::Published(String)` paints right-aligned in a fixed `DATE_GUTTER_W = 6` column, left of the duration slot, in `palette::ROW_DATE_FG` (`Palette::Yellow`). It contributes to a separate `date_reserve` that shrinks the title slot.
- Both paths currently coexist in `media_list_row` as two independent branches over the same `trailing: Option<MediaListTrailing>` field, so a row can carry at most one already.

`media_list_row` is the only row painter (`grep` confirms one call site, in `wide.rs`), so this is a single-site rendering change; it fans out only through the `MediaListTrailing` construction sites named in the proposal's Impact section.

## Goals / Non-Goals

**Goals:**
- One placement (the fixed six-column right gutter) and one colour (`STATUS_AVAILABLE` green) for both years and publish dates.
- No change to the gutter's width, truncation, or reservation math — it already handles both a 4-character year and a 6-character date correctly.
- No change to `Heading`/`Spacer` rows, which never carry `trailing` today.

**Non-Goals:**
- Not changing the Queue's duration slot: it keeps its precise `M:SS`/`H:MM:SS` format and gold `DURATION` role (per the "Durations share one precise format" scenario, which stays Queue-scoped). Hero meta-rows (`HeroFacts.duration_row`) are likewise untouched.
- Not introducing a new `MediaListTrailing` variant for anything beyond years/dates (e.g. no generic "badge" abstraction) — the enum still also carries the progress-badge case, which stays inline and FOAM-coloured, unchanged.
- Not changing which rows carry a year vs. a date — that's provider-specific data the call sites already decide; this change only unifies how the row painter renders whichever one a row carries.

  *Superseded (2026-09-21, user direction after the 5.2 manual check)*: durations are no longer untouched-by-scope — hero workspace lists and the audiobook browser row now project durations into the gutter (see the decisions entry below). The Queue's gold `DURATION` slot and hero meta-rows remain out of scope as stated above.

## Decisions

**Merge `Year`/`Published` into one `MediaListTrailing::Gutter(String)` variant**, rather than keeping two variants that both render identically. Once placement and colour match, the two names carry no remaining behavioural difference — a second name pointing at identical rendering is dead distinction, and every call site becomes simpler (`Some(MediaListTrailing::Gutter(text))` instead of picking a variant that no longer means anything different). The progress-badge case is not part of this merge: it stays a semantically and visually distinct thing (inline, FOAM, tied to `MediaSemanticState::Active`/`NowPlaying`, not a bare string).

Alternative considered: keep `Year`/`Published` as distinct variants that both map to the same paint branch. Rejected — it preserves a naming distinction with no behavioural payload, and the existing doc comments on the two variants already show that split naming invites re-diverging them later (each variant currently has its own paragraph justifying a distinct role that no longer exists after this change).

**Delete `ROW_DATE_FG`, point at `STATUS_AVAILABLE`.** `ROW_DATE_FG` has exactly one remaining use (the gutter) once this change lands; a second name for the same `Color` value is the kind of thing `docs/invariants/` and the design-system spec warn against (a role that duplicates another role's value drifts silently). `STATUS_AVAILABLE` already carries the "positive/available metadata" meaning a date or year fits.

Alternative considered: keep `ROW_DATE_FG` as a role, redefine its value to `Palette::Green`. Rejected per explicit user direction — no scenario in this change needs dates to diverge from the green status colour, and the existing rationale comment for keeping it separate ("so a duration edit cannot move the dates") is about protecting against `DURATION`, which this change doesn't touch either way.

**Hero workspace durations join the gutter; audiobook browser rows carry total runtime (2026-09-21, user direction).** The three Workspace producers — music tracklists (`music_content_workspace.rs` track_row), TV episode lists (`tv_content/mod.rs` `build_episode_rows`), and ABS book chapter lists (`book_content.rs` `chapter_rows`) — project each item's duration as `MediaListTrailing::Gutter(...)`, and the audiobook browser row (where movies/TV show the production year) projects the book's total runtime the same way. Rationale: the gutter already owns right-aligned per-row metadata in the green role; a duration in that same place with that same colour is the same fact with no behavioural payload of its own, so giving it a separate name (a new variant) or the gold `DURATION` slot (a Queue-only, precise-format role this change deliberately does not touch) would re-create exactly the dead-distinction pattern this change removes. The Queue's precise `fmt_duration_short`/gold slot is unchanged, and hero meta-rows (`HeroFacts.duration_row`) are unchanged.

**Gutter durations use a minutes-precision format: `M:SS` under an hour, `H:MM` at or over an hour.** The zero-padded `fmt_duration_hms` would produce 8-character values (`01:00:00`) that the fixed `DATE_GUTTER_W = 6` gutter truncates; the Queue's `fmt_duration_short` is precise (`H:MM:SS`) and its format is spec-pinned as Queue-only. A dedicated small helper (minutes-precision, always ≤6 columns) lives beside the other duration formatters and carries unit tests.

Alternative considered: a new `MediaListTrailing::Duration(String)` variant. Rejected — it would render identically to `Gutter`, which is the dead distinction this change exists to delete; the gutter string's meaning is already caller-owned.

**Rendering mechanics**: in `media_list_row`, replace the two current branches (the `trailing_pieces` inline push for `Year`, and the `published: Option<&str>` capture for `Published`) with a single `gutter: Option<&str>` capture from `MediaListTrailing::Gutter`, reusing the existing `date_reserve`/`DATE_GUTTER_W` math and paint block verbatim (just recolouring it to `STATUS_AVAILABLE`). The inline `trailing_pieces` vector keeps handling only the progress-percentage piece (still FOAM-coloured, still inline).

## Risks / Trade-offs

- [Visual regression across every browse screen] Movies/TV/Music/Generic rows currently show their year right next to the title; after this change it moves to the row's right edge, which is a visible layout shift on every such screen in one commit. → Mitigation: this is exactly the standardisation the user asked for (grounded in the podcast screen as the reference model); no partial rollout is meaningful since `media_list_row` is the single shared painter.
- [Existing buffer tests assert the old placement/colour] `src/app/render/components/media_list.rs` has tests asserting `STATUS_AVAILABLE` at the old inline year column and `ROW_DATE_FG` at the gutter column. → Mitigation: these are exactly the tests the task list must update to assert the new single gutter position/colour; per `AGENTS.md`, port the *behaviour* the test should now prove, not literal old assertions.

## Migration Plan

No data/persistence migration — this is a pure rendering/type change confined to one process's in-memory row model. Steps: change the type, change the painter, update every construction call site, update palette, update tests, `cargo fmt`/`clippy`/`nextest`. No feature flag — `AGENTS.md` explicitly avoids backwards-compatibility shims for a single-user app with no external consumers of this enum.
