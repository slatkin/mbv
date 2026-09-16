## Context

See `proposal.md` for motivation and `specs/queue-playback-panel/spec.md` for the requirements.

The current state that shapes the approach:

- Both playback panels consume one shared projection,
  `Model::transport_projection` in `src/app/shell_playback.rs`, which today derives the title as
  `QueueItem::title()` — the item's raw name/title for every kind.
- The panel's only two-part title is Emby episodes, produced by `App::playback_title_parts` in
  `src/app/render/components/chrome_player_context.rs`. It is reached by re-finding the queue item
  through `item.display_name() == title`, a formatted-string round-trip (the pattern
  `QueueItemContentId` was introduced to remove), and it is the only caller of that behaviour.
- `crates/mbv-core` already owns item presentation strings: `QueueItem::display_name()`,
  `QueueItem::display_name_parts()` (Emby episodes and Audiobookshelf episodes only), and
  `EmbyItem::playback_label()` (audio `artist - name`). None of them is consumed by the panel.
- `MediaListRow` (`src/app/components/media_list/`) is what media lists consume; its two-tone row
  painting is specified in `canonical-media-lists` and is **not** changing here.
- The two primitives the requested colours name already exist: `AQUA` (`#35a77c`, documented as the
  Emby brand / folder / watched colour) and `YELLOW` (`#dbbc7f`, documented as the focused-row
  title accent). No new colour value is needed, only new role names.
- `AudiobookshelfBookQueueItem` carries no per-file title, and ABS's per-file title
  (`audioTracks[].title`, which ABS defines as "the AudioFile with startOffset, contentUrl and
  title") is dropped by `AudioTrackWire` at parse. Audiobook playback is currently broken, so
  nothing about that path can be verified.

## Goals / Non-Goals

**Goals:**

- The panel's title naming is decided in one place, from the queue item, for every media type.
- That one place is reachable by media lists later without re-implementing the media-type table.
- The two colour roles are independent of every existing role, so unrelated theme edits cannot
  move the now-playing title.

**Non-Goals:**

- No change to media lists, their rows, or their colours.
- No change to the plain-text title forms (`playback_label()`, `display_name()`) used by toasts,
  the media-progress interface and log lines.
- No change to the queue item shape, the ctrl wire, or persisted queue state.
- No audiobook file-title naming (see D5).

## Decisions

### D1. The media-type table lives on `QueueItem` in `mbv-core`, colour-free

The mapping is presentation metadata about a core type, has no UI dependency, and sits beside the
`display_name*` methods that already answer the adjacent question. It returns a typed value naming
what each part *is* — a title part, and an optional context part — not a colour and not a
concatenated string.

Alternatives considered:

- **Widen `App::playback_title_parts` in place.** Smallest diff, but leaves the media-type table in
  the App layer, available only to the panel, and keeps the `display_name() == title` round-trip.
  Rejected: that is precisely the duplication the change exists to remove, and it would not be
  reachable by media lists.
- **Return a pre-joined string.** Rejected: joining fixes the separator, which is the thing the
  panel is no longer allowed to use.
- **Reuse `display_name_parts()`.** Rejected: it is ordered `(primary, secondary)` with primary =
  the *series* for an episode (the inverse of the panel's role assignment), it uses no typed roles,
  and it is consumed by Home's rows. Overloading it would silently re-role list rows; it stays as
  it is and its list consumers are untouched.

The context part for a feed entry is the subscription's display name, which lives in configuration
that `mbv-core` cannot see, so the mapping takes the caller-resolved feed name as an explicit
optional input. It is read only for feed items; every other kind ignores it. This keeps the table
in one place without teaching core about configuration.

### D2. Two new semantic roles, not the existing accent roles

`ACCENT` is documented as "focus accent, watched, folders, Emby brand glyph"; `TEXT_FOCUS_ACCENT`
as "focused-row title accent". Binding the now-playing title to either means a later focus-accent
or brand edit silently moves it — the coupling `unify-surface-colour-neutral` was fought to remove.
Two new roles are added to the theme vocabulary with the aqua and yellow values. `ui-design-system`
already provides for this ("no existing role fits a call site" → add a named role), so it needs no
delta.

Each new role also gets its own primitive, sharing today's value with the brand aqua and the
focused-row accent. That is this theme's established convention ("a primitive of its own, not the
border role's `OVERLAY`: a border-colour edit can never move a surface appearance"): without it,
a future brand-aqua or focus-accent primitive edit would still move the now-playing title, which is
the coupling D2 exists to prevent.

### D3. Colour delineates; no separator glyph inside the panel

The panel row is width-constrained (it shares a row with the controls, the seekbar and the status
pills) and a glyph spends that width to say what colour already says. A glyph is also ambiguous
when the data contains one — a podcast named `The Daily - Archive` collides with a ` - ` separator.
One space is kept between the parts, styled with the context role so the context reads as a unit.

The plain-text forms keep their separators: those surfaces have no colour, so the glyph is the only
information there. `playback_label()` and `display_name()` are therefore unchanged, which is also
what keeps toasts and the media-progress interface stable.

### D4. The parts are built from the item, and the builder only reads

`playback_title_parts` takes the queue item instead of a title string, which removes the
`display_name() == title` re-match. Everything it needs is a `&self` read of `App`, so the
receiver becomes shared and the by-value clones the current `&mut` receiver forces
(`now_playing_title.clone()`, the cloned `title`) go away.

### D5. Audiobook rows render the title alone

No context part is attempted for `AudiobookshelfBook`. DEFERRED: ABS's per-file title is a real
field (`audioTracks[].title`) that mbv currently discards, so the work is small once it is
verifiable — it needs a `title` field on the parsed audio track, a carrier on the playback session
and a position-to-file lookup. It is out of scope only because audiobook playback is broken, which
makes the result unverifiable rather than merely unverified. It is a follow-up change, not a
permanent exclusion.

### D6. The parts carry a closed role, not a colour

The projection and the render context carry each part with a closed role (title or context) and
let the painter resolve it through the theme, rather than carrying a resolved `Color` across the
shell→component boundary as the current `Vec<(String, Color)>` does. `ui-design-system` forbids
screens handing arbitrary `Color` values into shared components, and a closed role is also what
makes the two roles assertable in a buffer test without the test re-deriving the palette mapping.
The painter stays the single place that turns a role into a colour.

### D7. `StatusIndicator` pills and the codec value role are untouched

The status pills paint the codec *value* in the existing playback value role. The title leaving
that role is deliberate and does not imply the pills follow; changing them would alter a second
surface the proposal does not cover.

## Risks / Trade-offs

- **[Aqua contrast]** The title role's aqua primitive measures ~3.73:1 against the Library strip's
  resting fill and ~3.17:1 against its focused fill (`#3c4841`) — below WCAG AA body text (4.5)
  and dimmer than the green it replaces (~4.52 there, ~5.31 resting). On the queue transport band
  (`#1e2326`) it is ~5.26:1. → Mitigation: accepted as a deliberate choice of the brand aqua; the
  two roles are separate from the primitives, so a lighter aqua can be introduced later without
  touching a single call site. Confirm on a real terminal during apply rather than trusting the
  computed ratio.
- **[Two similar greens]** Aqua (`#35a77c`) and the existing playback value green (`#83c092`) are
  close in hue; the row must not show both. → Mitigation: the title role is the only aqua in the
  title row, and D7 keeps the codec value where it is. Any future second aqua in the same row
  reopens this.
- **[Role inversion against media lists]** Home's episode rows paint the *series* in the ordinary
  title role and the *episode* in yellow — the inverse of this change. After this change the panel
  and the lists assign the same two parts to opposite roles. → Mitigation: explicitly out of scope
  and stated in the proposal; the shared table (D1) makes the eventual list adoption a wiring
  change. The existing list two-tone tests are kept green as the guard that nothing moved.
- **[Per-tick feed lookup]** Resolving the subscription name runs during projection, which happens
  every tick while playback is active. → Mitigation: a linear scan of the configured subscriptions,
  only for feed items; no allocation beyond the name string.
- **[Untested seam]** The panel's two-part title has no test coverage today, so this change's
  behaviour is not protected by any existing failure. → Mitigation: the change adds the first
  coverage for that path (both panels, buffer-level) before relying on it.

## Migration Plan

Not applicable: no persisted state, no wire format, no configuration and no data model changes.
The change is additive at the theme and mapping level and is reverted by reverting the commit.

## Open Questions

None. The two questions that would have changed the specs, the approach or the task breakdown —
whether the media lists adopt the mapping now (no, future), and where the audiobook file title
comes from (ABS's dropped `audioTracks[].title`, deferred) — are resolved in D1 and D5.
