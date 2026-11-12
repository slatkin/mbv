# Spec Delta

## ADDED Requirements

### Requirement: The window content holds the screen's interaction state

The Settings screen's cursor, keyboard position, scroll, form drafts, choice marks before apply, and capture state SHALL live in the window content. The Client core SHALL hold whether the screen is open, the selected section, the section's data, and the actions. An update of the section's data SHALL NOT reset the window content's state for that section.

#### Scenario: An update keeps a draft

- **GIVEN** the Settings screen shows Feeds with a half-typed URL
- **WHEN** an update carries new Feeds section data
- **THEN** the screen shows Feeds with the half-typed URL

## REMOVED Requirements

### Requirement: The window content owns the screen's interaction state

**Reason**: The GPUI window has no content that reloads apart from the program, so the reload rule and its scenario have no meaning. The ownership rule stays.
**Migration**: The ADDED requirement "The window content holds the screen's interaction state" carries the same ownership rule with an update scenario.
