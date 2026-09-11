## Why

The Wide hero main content box (`CONTEXT.md:469`) is painted by one shared primitive, but each
destination supplies that primitive's horizontal frame, and five destinations supply three
different frames. The result is a silent, visible 2-column drift: on Home, Movies and
homevideos the box and its overview sit two columns right of the title column they belong to,
while TV and Music place theirs flush. The primitive's `area: Rect` parameter cannot express
which frame it received, so nothing could catch the remainder — the requirement already says the
arrangement owns the box rect and the destination "SHALL NOT define its own box geometry or
surface" (`openspec/specs/right-panel-arrangements/spec.md:262-267`), and the drift survived both
`standardize-hero-on-left-pane` (which hardcoded the primitive's padding, D9) and a month of use.

## What Changes

- **BREAKING (internal API)**: `wide_hero_hero_pane` returns a `Copy` newtype
  `WideHeroHeroPane { pane, content }` instead of a bare inset `Rect`, and
  `wide_hero_hero_content_box` / `_with_surface` take that newtype plus the caller's
  `y`/`height` instead of a bare `Rect`. A destination can no longer hand the box a rect in a
  frame of its own choosing: the horizontal frame comes from the pane primitive once.
- The box's left edge becomes the Hero pane's content edge on every destination (one horizontal
  frame), and its payload keeps the existing `(PANE_PAD_X, PANE_PAD_Y)` inset.
- TV's two `saturating_sub(PANE_PAD_X)` "expand back out" compensations are deleted, and TV's
  focused episode box uses the named `WideHeroContentBoxSurface::FocusedTrackList` variant
  instead of a manual `SURFACE_ACCENT_SOFT` repaint.
- Audiobookshelf Books drops the wide `SELECTED_BLOCK_SIDE_PADDING` inset on its book hero so its
  hero text and its chapters box share the one content frame (the wide inset is a legacy of the
  inline presentation, and leaving it would create a fresh 2-column mismatch once the frame is
  unified).
- A cross-destination buffer invariant joins the conformance matrix: at Wide geometry, every
  destination's leftmost main-content-box column SHALL be its pane's content edge. It fails today
  for Home, Movies, homevideos, Feeds, Audiobookshelf Podcasts and Audiobookshelf Books.

Visible deltas, for live review: the overview box on Home/Movies/homevideos (the reported
defect), the hero box on Feeds, the episode box on Audiobookshelf Podcasts, and the book hero text
plus chapters box on Audiobookshelf Books. TV and Music are unchanged by design.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `right-panel-arrangements`: the main content box's horizontal frame becomes arrangement-owned
  and mechanically unconstructible from a destination, with a scenario pinning one frame across
  destinations.

## Impact

- `src/app/render/arrangements/wide_hero.rs` — pane primitive's return type, box primitive's
  signature.
- Call sites: `src/app/render/components/hero.rs`, `tv_wide.rs`, `music_wide.rs`, `feeds.rs`,
  `audiobookshelf_podcast.rs`, `audiobookshelf_book.rs`.
- Tests: `src/app/render/tests_conformance_matrix.rs` gains the invariant; existing buffer and
  characterisation tests must be re-read for drifted-baseline expectations.
- Out of scope (separate defect, same "partial-unification remainder" family): Feeds' second
  (watched-filter) pill row, whose row budget the arrangement never reserved
  (`feeds.rs:73-136`), and the three wide-hero text painters.
- **Sequencing**: this change is planned but not implemented — it must land *after*
  `unify-surface-colour-neutral`, whose byte-identical-buffer acceptance bar this change's
  deliberate pixel moves would invalidate, and which migrates two of the same files
  (`arrangements/wide_hero.rs`, `components/hero.rs`). See design.md — Migration Plan; re-verify
  every line citation before starting (task 0.1).
