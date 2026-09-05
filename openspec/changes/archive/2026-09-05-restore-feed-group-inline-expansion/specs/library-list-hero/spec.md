## ADDED Requirements

### Requirement: Feed group picker uses the shared selected-row replacement

The Emby homevideos feed view group picker (an Emby homevideos feed view
library or an Emby podcast channel list) SHALL present the selected video with
the same variable-height Inline hero block that other hero-bearing browsers
use, at Normal geometry, and SHALL paint each visible video row exactly once.

Its expanded height SHALL be derived from the selected item's own compact
banner content at the block's text width, not from a fixed row count. Its
Inline hero SHALL show the title, metadata line, and truncated overview, and
SHALL own the selected row's hit geometry. The picker rows SHALL have exactly
one painter.

#### Scenario: Selected video expands in Normal geometry

- **WHEN** the picker is displayed in Normal geometry and the
  selected video carries runtime, genre, and an overview that wraps
- **THEN** its ordinary row is replaced by a framed Inline hero block whose
  height is the banner's content rows plus its fixed framing rows
- **AND** the block paints the title, the metadata line, and the truncated
  overview
- **AND** the rows below it keep their ordinary single-row presentation

#### Scenario: Selected video has no metadata

- **WHEN** the selected video has no runtime, genre, or overview
- **THEN** the picker still renders one row per video with one selected marker
- **AND** no framing or border row is painted outside that selection treatment

#### Scenario: Tall selected row reaches the viewport bottom

- **WHEN** the selected video's expanded block would extend past the bottom of
  the visible browser
- **THEN** scrolling moves upward far enough that the complete block is
  visible
- **AND** the remembered scroll position matches the landed offset

#### Scenario: Group pills stay reachable

- **WHEN** the picker's expanded selected row occupies the rows below the pill
  bar
- **THEN** the pill bar remains painted and clickable
- **AND** switching group re-derives the expansion from the new group's
  selected video
