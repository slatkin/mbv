# UI design system

> **Superseded / cancelled.** Revert the standalone #640 implementation. Its Audiobookshelf Books and Podcasts requirements are absorbed by `migrate-music-audiobookshelf-to-canonical-lists`; no requirement or scenario here is complete.

## MODIFIED Requirements

### Requirement: Podcast Wide rail uses shared geometry and semantic treatment
The Audiobookshelf podcast Wide rail MUST use the existing hero-on-left arrangement helpers, shared pill-bar geometry, and semantic surface/backdrop/focus framing. It MUST NOT introduce a podcast-specific breakpoint, arbitrary colors, or a second detached-detail variant.

#### Scenario: Wide rail at the threshold
- **GIVEN** a metadata-bearing selected show at the existing 82-column threshold
- **WHEN** rendered with Ratatui `TestBackend`
- **THEN** the capture shows one full-width row per show, the shared pill row, and shared rail framing
- **AND** selected, active, and played markers remain aligned to that row geometry.

### Requirement: Narrow presentation is preserved
The change MUST preserve the existing Narrow inline hero, selector placement, scroll/re-anchor behavior, explicit episode targets, and image-disabled rendering.

#### Scenario: Narrow regression
- **GIVEN** the same podcast fixture rendered below the Wide threshold
- **WHEN** it is rendered before and after the correction
- **THEN** the inline hero and list geometry remain equivalent, including re-anchoring when crossing the selected-row detail boundary.
