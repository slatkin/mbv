# right-panel-arrangements Specification Delta

## ADDED Requirements

### Requirement: Music and Audiobookshelf adopt the TV and Movies Wide precedent

Grouped Music and Audiobookshelf Podcast and Book destinations SHALL use the same Wide right-panel contract established by TV and Movies. When the shared width and minimum-height predicate is satisfied, the provider-owned detail/workspace SHALL occupy the left pane, while parent-owned browser-level pills followed by ordinary one-column rows SHALL occupy the right rail. The arrangement SHALL reuse the shared predicate, pane framing, content spacing, and short-height fallback. No Wide presentation SHALL use an Inline hero or selected-row replacement in the right rail.

#### Scenario: Wide provider workspace and ordinary right rail
- **WHEN** grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide geometry conditions
- **THEN** its provider-owned detail/workspace renders in the left pane
- **AND** its browser-level pills and ordinary one-column rows render in the right rail
- **AND** the right rail contains no Wide Inline hero or selected-row replacement.

#### Scenario: Shared geometry fallback is retained
- **WHEN** the destination crosses the shared width or minimum-height guard
- **THEN** it uses the same shared predicate, pane framing, content spacing, and short-height fallback as TV and Movies
- **AND** it uses the shared Inline fallback, or suppresses detail when the shared minimum cannot fit
- **AND** it does not define a destination-specific breakpoint or arrangement.
