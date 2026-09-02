# right-panel-arrangements Specification Delta

## MODIFIED Requirements

### Requirement: Hero-on-left and Inline list presentations use canonical controls
- Hero-on-left media surfaces SHALL place a persistent one-column `WideMediaList` in the right rail.
- Normal/non-wide selected-row replacement SHALL compose a persistent `InlineMediaBrowser`.
- Non-hero browsers SHALL retain their existing two-column arrangement policy.
- The arrangement owns pane placement and breakpoints; the embedded control owns list geometry and interaction.

#### Scenario: Narrow fallback is single-column
- **WHEN** the wide guard is not satisfied
- **THEN** the surface uses InlineMediaBrowser's single-column replacement
- **AND** it does not retain a duplicated wide rail or blank selected row.
