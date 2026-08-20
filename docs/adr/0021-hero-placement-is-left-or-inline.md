# Hero Placement Is Left Or Inline

Hero-bearing browse surfaces have exactly two supported presentations. When the
right panel meets the shared width breakpoint and existing minimum-height guard,
the selected detail is **hero-on-left**: it occupies the left workspace beside a
single-column browser in the right rail. Otherwise, selected detail is **inline**:
it follows the active media row in the single-column browser.

Top placement is not an arrangement and is not a compatibility fallback. No surface
may reserve a separate hero area above its browser or choose a surface-specific
responsive threshold. A width-wide but short terminal therefore uses inline detail;
if even the minimum active row and minimum detail cannot fit, detail is suppressed.

## Scope

The rule applies to Home, Movies, TV shows, grouped Music, Emby podcasts, Emby
home videos, Audiobookshelf podcasts, Audiobookshelf books, and Feeds. Surface
renderers retain ownership of content, artwork, provider-native state, and explicit
child targets, but not placement geometry.

Inline detail is part of scrolling list flow. Hero-only rows are inert; activation
remains on the owning media row. Existing episode, track, chapter, and selector
targets remain interactive. Hero-on-left read-only artwork and framing are inert as
well.

## Verification inventory

Every hero-bearing surface must be checked in all three geometry cases before and
after render-path changes:

| Surface | Wide + sufficient height | Narrow width | Wide width + short height |
| --- | --- | --- | --- |
| Home | hero-on-left | inline | inline or suppressed |
| Movies | hero-on-left | inline | inline or suppressed |
| TV shows | hero-on-left | inline | inline or suppressed |
| Grouped Music | hero-on-left | inline | inline or suppressed |
| Emby podcasts | hero-on-left | inline | inline or suppressed |
| Emby home videos | hero-on-left | inline | inline or suppressed |
| Audiobookshelf podcasts | hero-on-left | inline | inline or suppressed |
| Audiobookshelf books | hero-on-left | inline | inline or suppressed |
| Feeds | hero-on-left | inline | inline or suppressed |

Focused render/input checks for each row must also cover selected-row tracking,
variable detail height, scroll visibility, inert hero framing, and any explicit
child targets owned by that surface. Images-disabled rendering remains part of the
same inventory.

## Considered options

- **Retain top placement as a fallback:** rejected because it creates a third,
  surface-dependent behavior and preserves obsolete activation and geometry paths.
- **Ignore the minimum-height guard:** rejected because a left workspace without
  enough height makes selected detail and browser content inaccessible.
- **Build the complete component framework now:** rejected; that belongs to issue
  #563 and is not required to establish this placement invariant.

## Consequences

Placement is recomputed from shared geometry whenever the panel changes size or
Panel mode changes. Surface-specific content remains stable while only composition
changes. Removing the top path is intentional and requires current source, tests,
live specs, `CONTEXT.md`, and current ADRs to use only the two supported terms;
archived OpenSpec history is not rewritten.
