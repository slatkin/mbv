# Design

## Context

See `proposal.md` — Why. Constraints that shape the approach:

- **The change lands before TV becomes a tree.** The shared list seam
  (`extract-shared-list-components`) is in flight, but TV migration is a later
  change. `Latest` and `Upcoming` are flat episode lists and the series modes
  are today's flat series list, so nothing here depends on tree internals. The
  design must not smuggle in tree assumptions.
- **Movie libraries share the existing pill machinery.** `LETTER_FILTER_BUCKETS`
  and `LetterFilter` are a single global set read by both `emby_library_content`
  (movies) and `tv_content` (TV). Freezing movies requires the TV bucket set and
  the movie bucket set to coexist without movie behavior changing.
- **The threshold's old job is partly gone.** `LIBRARY_PILL_THRESHOLD`
  currently triggers `maybe_capture_library_total_and_apply_default_pill`,
  which auto-scopes a large library to `A-C`. The capture half of that
  function stays: the first unfiltered top-level load's `total_count` is the
  only source of a library's show count (`get_user_views` carries no child
  counts), and the threshold decision needs it. What is replaced is only the
  `A-C` auto-apply, and only for TV: TV resolves its default mode instead,
  movies, feeds, and podcasts keep today's auto-scope byte-for-byte, and the
  quirk where a small TV library paints a pill row with `A-C` highlighted
  while showing an unfiltered list is retired on the TV row only.
- **The new-content marker is component-local today.** `visited_latest_sources`
  lives inside the Home component, and the marker is computed from the shell's
  frozen `HomeLatestLaunchWindow`. Two surfaces will now read the same section.
- **Verify the Upcoming feed's semantics against a real server.** `GET
  /Shows/Upcoming` exists in the official Emby SDK, but whether it can return
  episodes that are not playable in the library (virtual/unaired) is not
  settled by the route documentation. This is a manual check, not a test.

## Goals / Non-Goals

**Goals:**

- One TV content-mode selector whose modes are `Latest`, `Upcoming`, the
  alphabet ranges, and (only when the ranges are absent) `All`.
- The library's `Latest` and Home's `Latest` pill for that library are two views
  of one section, including its new-content acknowledgement.
- Movie, music, feed, and Audiobookshelf pill behavior is byte-for-byte
  unchanged.
- The selected mode is part of the sticky navigation position.

**Non-Goals:**

- No TV tree migration, no shared-seam dependency.
- No change to the `Latest` request/limit semantics or Home's sections beyond
  the shared marker and acknowledgement.
- No episode hero or workspace outside mini view.
- No unification of movie and TV pills.

## Decisions

### D1: The content mode is one value on the browse level, replacing the letter filter

The persisted `letter_filter: Option<LetterFilter>` cannot express `Latest`,
`Upcoming`, or `All`, and `None` currently overloads "unfiltered". Replace it
with a closed `TvContentMode` value on the top-level `BrowseLevel` — roughly
`Latest`, `Upcoming`, `All`, and `Range(LetterFilterIndex)` — carried in
`LibraryPositionLevel` and restored on reopen. `None`/absent means the default
mode for the library's size, resolved on load.

Rationale: one value keeps row order, cycling, persistence, and content
selection in agreement, and stops `None` from meaning both "no filter" and "the
default a large library would auto-apply". Alternative considered: keep
`letter_filter` and add a parallel `latest`/`upcoming` flag — rejected as three
fields that can disagree.

### D2: The alphabet bucket table becomes kind-aware; movies keep their own set

`LETTER_FILTER_BUCKETS` stays the movie set, untouched. A TV-specific
three-bucket table is added beside it (`A-I` = `NameLessThan("J")`, `J-R` =
`[J, S)`, `S-Z` = `NameStartsWithOrGreater("S")` with no upper bound) and
`LetterFilter` construction takes the library kind (or the caller passes the
table). The `#` pill disappears from the TV set because `NameLessThan("J")`
already covers everything sorting before `J`; the open upper bound on `S-Z`
also closes today's accented-after-Z gap.

Rationale: three single server ranges cover the whole alphabet with no title
lost and no compound fetch. Alternative considered: a `#` pill plus three
ranges — rejected as four pills against the requirement; a compound two-range
third pill — rejected as more client work for the same result.

### D3: The threshold gates the ranges, not `Latest`/`Upcoming`

`LIBRARY_PILL_THRESHOLD` (unchanged at 300) now decides whether the row
contains the three ranges. Above it: `Latest | Upcoming | A-I | J-R | S-Z` and
`Latest` opens. At or below it: `Latest | Upcoming | All` and `All` opens. The
decision input is the TV library's show count (`library_total`), and the
first-open sequence is fixed as follows. A library's first-ever open has no
count, so the existing unfiltered capture load runs unchanged; while the count
is unknown the mode row is not painted, matching today's pills appearing only
after capture. When the count lands, the default mode is auto-selected:
`Latest` for a large library (one episode fetch replaces the unfiltered list —
a second fetch, but never a letter-scoped one), `All` for a small one (the
unfiltered list already is the `All` content, so no re-fetch). Every reopen
resolves the mode from the restored `library_total` before any fetch, so a
large library's reopen loads `Latest` directly as its initial load. A restored
mode the current count no longer offers — the library crossed the threshold
between runs — is re-clamped to that count's default before paint or fetch: a
saved `All` above the threshold becomes `Latest`, a saved range below it
becomes `All`. The stale-mode row must not resurrect the retired quirk of a
pill highlighted over content it does not select. The
`A-C` auto-apply arm is removed for TV only; movie, feed, and podcast
libraries keep today's auto-scope and their small-library `A-C`-highlighted
row byte-for-byte.

Rationale: `All` is the only way to see everything when there are no ranges, so
it must exist below the threshold and must not exist above it (it would load the
whole library). `Latest` being the large-library default is the deliberate
behavior change from the old `A-C` default.

### D4: `Latest` reuses Home's request, `Upcoming` adds one

`Latest` selects the same feed as Home's TV section —
`get_latest_episodes(view_id, 30)` — but issues its own request from the library
path, so Home need not have loaded. `Upcoming` adds
`get_upcoming(parent_id, limit)` over `GET /Shows/Upcoming` with `ParentId` set
to the library. Both produce flat `EmbyItem` episode lists. Rows with a playable
episode id play directly and never open a series detail; `Upcoming` rows with no
episode id but a series reference navigate to that series and open its Workspace
(REVISED 2026-09-22 — the real server returns Virtual/id-less placeholders).

### D5: The new-content acknowledgement moves to the shell

Hoist the acknowledged-set from the Home component into the shell (Model),
keyed by `HomeLatestSource`, and have both the Home selector and the TV
`Latest` mode render `section.has_new_content && !acknowledged.contains(source)`.
The marker value itself stays the shell-computed `has_new_content`, which is
fixed against the launch window; a `Latest` fetch made after launch does not
recompute it (new content after launch must not mark).

Rationale: two components rendering one section's marker cannot each own a
private visited set. Note for review: `extract-shared-list-components` touches
component state-carrier ownership; this hoist should land as a small shell-owned
set, not a new component abstraction.

### D6: Episode rows play when playable, otherwise open their series (REVISED 2026-09-22)

`Latest` and `Upcoming` rows with a playable episode id carry the episode as their stable target and activate play, matching Home's episode rows. `Upcoming` rows with no episode id but a series reference carry a synthesized stable target (series + season/episode identity, never a shared empty `Id`) and activate navigation to that series' Workspace. Keyboard and mouse activation take the same path. The episode hero is produced only for the mini-view presentation, and non-mini geometries reclaim the hero pane instead of reserving it.

### D7: Cycling and mouse selection follow the painted row

`[`/`]` cycle over the modes actually present in the row (not a fixed list), so
a small library cycles `Latest → Upcoming → All`, and a large one cycles
`Latest → Upcoming → A-I → J-R → S-Z`, wrapping at the ends. Mouse selection
selects the clicked row's mode. This keeps the cycling and the painted row from
diverging.

## Risks / Trade-offs

- **`/Shows/Upcoming` may include episodes not playable from the library.** →
  Manual check against a real server before implementation is called done; if
  unplayable rows appear, either filter to items present locally or play through
  the same failure path as any unavailable item. (2026-09-22 LESSON: the check ran
  at close-out instead of before, and its negative finding was filed through 8.2's
  "or record" branch rather than reopening planning. Row 8.2 now requires a negative
  finding to reopen planning; the Virtual/id-less slate is tracked by tasks §9.)
- **Hoisting acknowledgement conflicts with the in-flight seam work.** →
  Sequence this change after `extract-shared-list-components` lands, and keep
  the hoist to a shell-owned set with no new abstraction.
- **A large library now opens on `Latest`, which is a real behavior change.** →
  This is intended; the design keeps the old default reachable by one keypress
  and records it in the proposal.
- **A large library's first-ever open briefly shows the unfiltered list before
  `Latest` auto-selects.** → Accepted: the capture load is the show count's
  only source. The swap happens once, on the first open only; reopens resolve
  the mode from the restored count before any fetch.
- **Movie and TV bucket tables can drift.** → The two tables live adjacent in
  `sort_filter.rs` and each is named for its library kind; tests assert movie
  labels and TV labels separately.
- **The mode value must not leak into the future tree as a flat-only type.** →
  Keep `TvContentMode` at the browse-state boundary (what to load, what to
  select), not inside any list widget, so the tree change adopts it as the
  selector input rather than reinterpreting it.
