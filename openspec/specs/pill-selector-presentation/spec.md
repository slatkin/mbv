# pill-selector-presentation Specification

## Purpose
Defines a consistent, reusable visual language for interactive pill selectors across the terminal interface while preserving their existing navigation behavior.
## Requirements
### Requirement: Interactive pill selectors share one appearance
The TUI SHALL render Home section, feed group, music group, letter filter, and series season controls with the same pill-selector appearance. The shared appearance SHALL use joined angled pill edges, a green selected surface with white text, and a dark unselected surface with muted text.

#### Scenario: Selected and unselected choices render consistently
- **WHEN** any in-scope pill selector displays selected and unselected choices
- **THEN** its selected and unselected choices use the shared pill-selector appearance

### Requirement: Pill selector presentation has one source of truth
The TUI SHALL derive the visual shell and palette of every in-scope pill selector from shared presentation definitions so that changing those definitions updates all in-scope selectors.

#### Scenario: Shared appearance changes
- **WHEN** the shared pill-selector presentation definitions are changed
- **THEN** Home, library/filter, and series-season pill selectors render the changed appearance without context-specific appearance implementations

### Requirement: Existing pill selector interaction is preserved
The TUI SHALL preserve each pill selector's existing selected value, keyboard commands, mouse selection, caller-defined target identity, and overflow behavior while unifying presentation.

#### Scenario: Selected choice overflows available width
- **WHEN** a pill selector's choices exceed the available row width
- **THEN** the visible window includes the selected choice and indicates hidden choices

#### Scenario: User selects a visible pill with the mouse
- **WHEN** the user clicks a visible selectable pill
- **THEN** the existing target represented by that pill is selected

#### Scenario: User operates a pill selector with the keyboard
- **WHEN** the user invokes an existing keyboard command for a pill selector
- **THEN** selection changes according to the existing command behavior

### Requirement: Tabs and status pills remain distinct
The shared pill-selector appearance SHALL NOT be applied to primary Home/library navigation tabs or to pills that only report connection, session, playlist, or playback status.

#### Scenario: Non-selector chrome is rendered
- **WHEN** primary navigation tabs or non-interactive status pills are displayed
- **THEN** they retain their independently defined presentation

