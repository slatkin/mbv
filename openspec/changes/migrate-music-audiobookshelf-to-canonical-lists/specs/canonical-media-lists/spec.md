# canonical-media-lists Specification

## Modified Requirements

### Requirement: Provider destinations compose canonical media controls

Grouped Music album browsing and Audiobookshelf Podcast show browsing and Book browsing SHALL prepare provider-owned content as canonical selectable `Item`, non-selectable `Heading`, and `Spacer` rows and compose `WideMediaList` for Wide rails and `InlineMediaBrowser` for Normal/Narrow selected-row replacement where the arrangement permits. The controls SHALL remain embedded beneath the mounted destination component; provider workspaces, images, selectors, surname buckets, effects, and typed intent translation remain parent-owned.

#### Scenario: Music groups retain provider authority
- **WHEN** a grouped Music album surface is rendered or navigated
- **THEN** album/group rows use the canonical control
- **AND** grouping, track authority, images, selection, and playback intents remain Music-owned
- **AND** no second list painter or App-owned interaction mirror runs.

#### Scenario: Audiobookshelf shows compose without losing episodes
- **WHEN** a Podcast library is shown Wide or Normal
- **THEN** shows use the canonical list presentation
- **AND** the selected show's episode workspace remains provider-owned, including episode filtering, images, and typed playback intents.

#### Scenario: Audiobookshelf books compose without duplicate detail
- **WHEN** a Book library is shown Wide
- **THEN** the selected book is represented by the sole canonical selected-row/hero presentation and is not painted again as a detached selected-row replacement
- **AND** chapter rows remain provider-owned seek targets for the selected book.

### Requirement: Audiobookshelf geometry has complete breakpoint fallbacks

Audiobookshelf Podcast and Book surfaces SHALL use the shared Hero-on-left or Inline arrangement at the established Wide/Normal breakpoints, preserve the short-height fallback, and hand off stable selected target and viewport anchor across breakpoint changes. Non-list repairs required to make the composition correct SHALL live in shared arrangements or the owning destination component, not a bespoke exception.

#### Scenario: Wide and short layouts are deterministic
- **WHEN** terminal width/height crosses the Wide threshold or the short-height guard
- **THEN** the surface selects the defined Wide, Normal, or short fallback arrangement
- **AND** the selected target, row offset, images, framing, and focus remain stable.

### Requirement: Mouse ownership crosses the parent/child seam once

The mounted Music or Audiobookshelf destination parent SHALL own mouse subscription and gesture recognition, while the embedded canonical control SHALL own render-derived hit regions. A recognized point SHALL be delegated to the child before parent workspace targets, with explicit child targets taking precedence; no global hit map or duplicate coordinate arithmetic is introduced.

#### Scenario: A list click resolves in the child
- **WHEN** a pointer gesture lands on a painted canonical row
- **THEN** the parent delegates the point to the embedded control's `HitRegions<Target>`
- **AND** the resolved target becomes the typed intent input without a second list hit path.
