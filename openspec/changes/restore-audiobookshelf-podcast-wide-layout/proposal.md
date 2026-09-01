## Status: Superseded / Cancelled

The standalone #640 implementation is cancelled and must not proceed. Any work associated with that standalone plan is to be reverted where present; the required Audiobookshelf Books and Podcasts repairs are absorbed by `migrate-music-audiobookshelf-to-canonical-lists`, which is the sole active owner of their composition and required non-list fixes.

This change remains only as historical planning context. It does not promise an implementation, tests, delivery, or a separate pull request.

## Historical Context

Issue #640 described an Audiobookshelf podcast Wide surface that bypassed the shared hero-on-left arrangement. The earlier plan recorded a separate right/hero panel, provider-local column sizing, missing shared pills and framing, and the need to preserve the provider episode workspace and existing Narrow behavior.

## Supersession and Absorption

The canonical Music/Audiobookshelf slice supersedes this plan. Its scope includes the shared composition and the related Audiobookshelf Books and Podcasts repairs, including the Podcast Wide rail and other non-list layout defects needed for canonical composition. Those repairs belong there without a bespoke #640 exception or a second destination-sized list path.

The explicit revert/absorption rule prevents this cancelled change from being implemented independently or from creating a competing ownership path. No task in this directory is evidence that the absorbed work is complete; completion is tracked only by `migrate-music-audiobookshelf-to-canonical-lists`.

## Historical Capability References

The retained spec files record the capabilities that the cancelled plan had considered:

- `right-panel-arrangements` — the historical Podcast Wide placement and rail presentation.
- `ui-design-system` — the historical shared geometry, semantic framing, and Narrow-preservation considerations.

They are not active capability deltas for this cancelled change.

## Impact

This cancelled change has no current code, API, dependency, test, implementation, or delivery impact. The canonical Music/Audiobookshelf change owns any resulting implementation and verification. Feeds Service work and the Emby homevideos feed view remain separate and are not absorbed by this change or by the Podcast repair rationale here.
