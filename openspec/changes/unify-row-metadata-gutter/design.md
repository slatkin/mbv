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
- Not changing the duration slot's role, format, or position — it is a separate, deliberately distinct role from the gutter (per the "Durations share one precise format" scenario) and is untouched.
- Not introducing a new `MediaListTrailing` variant for anything beyond years/dates (e.g. no generic "badge" abstraction) — the enum still also carries the progress-badge case, which stays inline and FOAM-coloured, unchanged.
- Not changing which rows carry a year vs. a date — that's provider-specific data the call sites already decide; this change only unifies how the row painter renders whichever one a row carries.

## Decisions

**Merge `Year`/`Published` into one `MediaListTrailing::Gutter(String)` variant**, rather than keeping two variants that both render identically. Once placement and colour match, the two names carry no remaining behavioural difference — a second name pointing at identical rendering is dead distinction, and every call site becomes simpler (`Some(MediaListTrailing::Gutter(text))` instead of picking a variant that no longer means anything different). The progress-badge case is not part of this merge: it stays a semantically and visually distinct thing (inline, FOAM, tied to `MediaSemanticState::Active`/`NowPlaying`, not a bare string).

Alternative considered: keep `Year`/`Published` as distinct variants that both map to the same paint branch. Rejected — it preserves a naming distinction with no behavioural payload, and the existing doc comments on the two variants already show that split naming invites re-diverging them later (each variant currently has its own paragraph justifying a distinct role that no longer exists after this change).

**Delete `ROW_DATE_FG`, point at `STATUS_AVAILABLE`.** `ROW_DATE_FG` has exactly one remaining use (the gutter) once this change lands; a second name for the same `Color` value is the kind of thing `docs/invariants/` and the design-system spec warn against (a role that duplicates another role's value drifts silently). `STATUS_AVAILABLE` already carries the "positive/available metadata" meaning a date or year fits.

Alternative considered: keep `ROW_DATE_FG` as a role, redefine its value to `Palette::Green`. Rejected per explicit user direction — no scenario in this change needs dates to diverge from the green status colour, and the existing rationale comment for keeping it separate ("so a duration edit cannot move the dates") is about protecting against `DURATION`, which this change doesn't touch either way.

**Rendering mechanics**: in `media_list_row`, replace the two current branches (the `trailing_pieces` inline push for `Year`, and the `published: Option<&str>` capture for `Published`) with a single `gutter: Option<&str>` capture from `MediaListTrailing::Gutter`, reusing the existing `date_reserve`/`DATE_GUTTER_W` math and paint block verbatim (just recolouring it to `STATUS_AVAILABLE`). The inline `trailing_pieces` vector keeps handling only the progress-percentage piece (still FOAM-coloured, still inline).

## Risks / Trade-offs

- [Visual regression across every browse screen] Movies/TV/Music/Generic rows currently show their year right next to the title; after this change it moves to the row's right edge, which is a visible layout shift on every such screen in one commit. → Mitigation: this is exactly the standardisation the user asked for (grounded in the podcast screen as the reference model); no partial rollout is meaningful since `media_list_row` is the single shared painter.
- [Existing buffer tests assert the old placement/colour] `src/app/render/components/media_list.rs` has tests asserting `STATUS_AVAILABLE` at the old inline year column and `ROW_DATE_FG` at the gutter column. → Mitigation: these are exactly the tests the task list must update to assert the new single gutter position/colour; per `AGENTS.md`, port the *behaviour* the test should now prove, not literal old assertions.

## Migration Plan

No data/persistence migration — this is a pure rendering/type change confined to one process's in-memory row model. Steps: change the type, change the painter, update every construction call site, update palette, update tests, `cargo fmt`/`clippy`/`nextest`. No feature flag — `AGENTS.md` explicitly avoids backwards-compatibility shims for a single-user app with no external consumers of this enum.
