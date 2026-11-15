## MODIFIED Requirements

### Requirement: Every Settings request reaches the Client core

Every request that the Settings screen sends SHALL reach the Client core. No permission layer SHALL stand between the window and the Client core, so no Settings action is refused before the core sees it.

#### Scenario: A sidebar press reaches the core

- **WHEN** the user presses a sidebar entry
- **THEN** the core receives the section request, and the next frame shows that section

## REMOVED Requirements

### Requirement: The window content owns the screen's interaction state

**Reason**: Its reload rule and its scenario "A reload keeps the section" need a window-content reload, and the Blitz window has none.
**Migration**: The rule, without the reload, moves to "The window content holds the screen's interaction state".

## ADDED Requirements

### Requirement: The window content holds the screen's interaction state

The Settings screen's cursor, keyboard position, scroll, form drafts, choice marks before apply, and capture state SHALL live in the window content. The Client core SHALL hold whether the screen is open, the selected section, the section's data, and the actions. A section update from the Client core SHALL NOT reset the window content's state for that section.

#### Scenario: A section update keeps a draft

- **GIVEN** the Settings screen shows Feeds with a half-typed URL
- **WHEN** an update of the Feeds section arrives from the Client core
- **THEN** the screen shows Feeds with the half-typed URL
