## Purpose

Defines one shared source of truth for the TUI's colour roles, so that a visual decision is made
once and applies everywhere, and so a screen cannot invent its own answer to a question the design
language already answers.

## ADDED Requirements

### Requirement: Colour roles have one definition

The TUI SHALL define its colours as named roles, and every surface, text run, rule, and indicator
SHALL derive its colour from a role rather than from a literal colour value. Changing a role's
definition SHALL change every place that role is used, with no per-screen implementation of the same
role.

#### Scenario: A role definition changes

- **WHEN** the definition of a colour role is changed
- **THEN** every screen using that role renders the changed colour
- **AND** no screen continues to render the previous colour

#### Scenario: Two screens present the same concept

- **WHEN** two screens display the same concept, such as a selected row or a resting panel
- **THEN** both derive that colour from the same role and render identically

### Requirement: Focus state colouring is centrally controlled

The focused and unfocused appearance of every panel, sub-panel, list, and component SHALL be
determined in one place from a focus state supplied by the caller. A screen SHALL supply the
panel's focus state — the existing `PanelFocus` plus, for hero-on-left screens, a pane bit — and
SHALL NOT name the colour used for any state. This SHALL apply to the left panel's card and queue
as well as to right-panel content.

#### Scenario: The focused appearance is changed

- **WHEN** the definition of the focused appearance is changed in one place
- **THEN** every panel and sub-panel in the application renders the changed appearance, including
  the queue and card
- **AND** no screen requires an individual edit

#### Scenario: A screen reports its focus state

- **WHEN** a screen renders with a given panel and pane focus state
- **THEN** its appearance is chosen by the shared definition from that focus state alone

### Requirement: Per-screen colour exceptions are named variants

A screen MAY deviate from a default colour role only by opting into a variant that is itself defined
once alongside the roles. A screen SHALL NOT supply a literal colour value at the point of use. A
second screen needing the same deviation SHALL reuse the existing variant rather than introduce
another.

#### Scenario: A screen needs a colour that differs from the default

- **WHEN** a screen requires an appearance the default role does not provide
- **THEN** it opts into a named variant defined with the roles
- **AND** the variant is available to any other screen by the same name

#### Scenario: A variant definition changes

- **WHEN** a variant's definition is changed
- **THEN** every screen opted into that variant renders the change
