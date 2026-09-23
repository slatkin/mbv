# Spec Delta

## Purpose

TV show-browsing modes expose a nested show, season, and episode list while retaining the established Hero Workspace and keeping episode-oriented modes flat.

## ADDED Requirements

### Requirement: Show-browsing modes expose the TV hierarchy
When browsing shows in `All` or a letter-range mode, the TV library SHALL display shows as selectable expandable roots, their seasons as selectable expandable children, and episodes as selectable children of seasons. The existing show group headings SHALL remain visible and non-selectable. Collapsed descendants SHALL not be reachable until expanded. A show's identity SHALL remain stable across refresh and geometry changes, and season and episode identities SHALL be scoped to their show and season where needed to distinguish occurrences.

#### Scenario: Expand a show and season
- **WHEN** a user expands a show and then one of its seasons
- **THEN** the list reveals that show's seasons and that season's episodes inline at successive depths
- **AND** collapsing either branch hides its descendants from navigation

#### Scenario: Group headings remain labels
- **WHEN** a show mode presents alphabet group headings
- **THEN** they label their following shows but cannot be selected or activated

#### Scenario: Preserve the selected row on refresh and resize
- **WHEN** the TV list refreshes or changes between panel geometries and its selected row still exists
- **THEN** the same show, season, or episode remains selected and the viewport keeps it visible

### Requirement: TV Hero remains independent of inline expansion
The TV Hero SHALL continue to show the selected show's detail, including its existing season pills and episode Workspace. Episodes shown in an expanded tree SHALL also remain available in that Workspace; the tree SHALL NOT replace or remove its season selector, episode selection, or episode actions. Selecting a season or episode in the tree SHALL retain its containing show as the Hero subject.

#### Scenario: Expanded episode coexists with Workspace episode
- **WHEN** a show and season are expanded to reveal an episode
- **THEN** that episode is visible inline and the Hero still presents the show's season pills and episode Workspace

#### Scenario: Child selection retains the Hero show
- **WHEN** a season or episode row becomes selected
- **THEN** the Hero continues to show its containing show's detail rather than an episode Hero

### Requirement: Tree rows retain TV browse actions
Show activation SHALL retain the TV show-detail interaction, season activation SHALL expand or collapse that season, and episode activation SHALL play that episode. Context and playback actions SHALL resolve the selected row's stable TV identity to the corresponding item, not to a stale numeric index or to the Hero Workspace's separate episode cursor. A heading SHALL emit no action.

#### Scenario: Activate an episode inline
- **WHEN** the user activates an expanded episode row
- **THEN** that episode is played without requiring selection in the Hero Workspace

#### Scenario: Activate a show
- **WHEN** the user activates a show row
- **THEN** the existing show detail/Workspace interaction remains available

### Requirement: Episode content modes and search retain their own presentation
`Latest` and `Upcoming` SHALL remain flat episode lists with direct episode activation and their existing Hero rules; they SHALL NOT acquire show/season nesting or group headings. Inline Search SHALL remain a separate result list, without changing the show-tree's settled expansion or selection when it is opened and dismissed.

#### Scenario: Latest stays flat
- **WHEN** `Latest` or `Upcoming` is selected
- **THEN** its episode rows remain flat and activate directly
- **AND** no show-tree hierarchy or show Hero Workspace is opened for the episode

#### Scenario: Dismiss search
- **WHEN** the user enters and then dismisses Inline Search from a show mode
- **THEN** the prior show-tree selection and expansion are restored
