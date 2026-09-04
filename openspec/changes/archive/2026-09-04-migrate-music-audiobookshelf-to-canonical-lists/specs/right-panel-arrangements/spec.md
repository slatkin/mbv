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

### Requirement: Audiobookshelf Podcast and Book Wide surfaces route through the shared right pane

The Audiobookshelf Podcast Wide surface SHALL render through the shared hero-on-left right-pane arrangement rather than a bespoke painter. The Audiobookshelf Book Wide right rail already routes through the shared right pane; its defect is that the `render_book_browser` call reused there carries the inline selected-row replacement path, and that replacement path SHALL NOT be used in the Wide right rail. These are provider-arrangement repairs this slice owns, distinct from the canonical list control itself. The Podcast Wide right rail SHALL present the same pill row it presents at Narrow width. The Book Wide left pane SHALL use the shared provider-detail-workspace framing and content spacing used by grouped Music, and its right rail SHALL show ordinary fixed-height one-column rows with no selected-row replacement and no Inline hero. Neither surface SHALL define a destination-specific breakpoint, column-sizing rule, or fallback.

#### Scenario: Podcast Wide has pill-row parity with Narrow
- **WHEN** an Audiobookshelf Podcast library meets the shared Wide geometry conditions
- **THEN** its right rail renders the shared pill row over the one-column show browser
- **AND** it routes through the shared hero-on-left right pane, not a surface-specific painter.

#### Scenario: Book Wide uses shared workspace framing
- **WHEN** an Audiobookshelf Book library meets the shared Wide geometry conditions
- **THEN** the selected book's provider detail workspace renders in the left pane with the shared framing and spacing used by grouped Music
- **AND** the right rail renders ordinary fixed-height one-column rows with no selected-row replacement or Inline hero.
