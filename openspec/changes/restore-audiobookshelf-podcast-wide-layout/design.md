## Status: Superseded / Cancelled

This design is retained as historical context for the cancelled standalone #640 plan. It is not an implementation direction, does not authorize a separate implementation or pull request, and has no remaining execution decisions.

## Historical Context

The former plan addressed an Audiobookshelf podcast surface whose Wide presentation diverged from the shared hero-on-left geometry. It identified detached detail placement, provider-local rail sizing, missing shared pills/framing, shell geometry projection, and the need to retain the provider episode workspace and Narrow behavior.

## Supersession Decision

The standalone plan was cancelled so that Audiobookshelf Books and Podcasts are repaired together with the canonical Music/Audiobookshelf composition. `migrate-music-audiobookshelf-to-canonical-lists` is the sole owner of those repairs, including the Podcast Wide rail and any non-list layout work needed to make canonical composition correct. This change supplies no parallel arrangement, exception, breakpoint, test plan, or delivery sequence.

The former design's shared-arrangement rationale is preserved only to explain the absorption: canonical controls and the established Hero-on-left/Inline presentations are preferred over a destination-specific detached-detail path; provider episode/chapter workspaces and typed intents remain provider-owned; and the established breakpoint and short-height behavior should not be regressed by the active canonical work.

## Scope Boundary

The cancelled plan never covered provider fetching or playback semantics, queue or protocol behavior, keyboard or mouse routing, or daemon changes. It also remains distinct from the Feeds Service and its Feeds tab, and from the Emby homevideos feed view. Those surfaces are not evidence for this change and are not folded into its supersession.

## Ownership After Supersession

Any source, geometry, visual verification, and test decisions formerly listed here are historical inputs only. They are to be evaluated and, if still needed, implemented and verified under `migrate-music-audiobookshelf-to-canonical-lists`; no work should be started from this design.
