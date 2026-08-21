## Purpose

The UI design-system capability keeps mbv's terminal surfaces visually consistent while allowing screens to provide their own content and explicitly approved semantic variations. These are normative requirements for every existing and new surface, not conventions or optional guidance; existing surfaces are not grandfathered.

## ADDED Requirements

### Requirement: Screens use canonical UI ownership boundaries

The UI SHALL separate screen content, arrangement geometry, and component painting. Screens SHALL provide semantic content and approved variants; arrangements SHALL own shared geometry; components SHALL own their painting, styling, and interaction geometry. Screen code SHALL NOT call Ratatui, construct layout rectangles, or calculate hit targets. These ownership requirements SHALL apply to every current and future surface without a convention-based or grandfathered exception.

#### Scenario: A screen adds content without duplicating a painter
- **WHEN** a screen needs different titles, metadata, rows, or images
- **THEN** it supplies a screen model to an existing arrangement or component
- **AND** it does not copy the arrangement's geometry or painter

#### Scenario: A component exposes interaction targets
- **WHEN** an interactive component is rendered
- **THEN** its hit targets are derived from the same layout used for painting
- **AND** callers do not independently reconstruct those targets from screen-specific coordinates

#### Scenario: A hero uses an approved additional-content style
- **WHEN** a hero supplies the Movie overview/detail block, the TV season/pill and episode workspace, the Music track-list workspace, or another centrally mapped provider-specific style, including its preview versus focusable child state
- **THEN** the screen supplies the typed content and interaction state to the shared arrangement or component
- **AND** the arrangement owns the style's pane placement, sizing, spacing, responsive layout, and hit-target aggregation
- **AND** the screen does not invent another additional-content style or supply screen-local rectangles, row arithmetic, breakpoints, or renderer callbacks
- **AND** any approved customisation is implemented by the central owning component or arrangement, not by the surface

#### Scenario: Every current hero has an approved style
- **WHEN** the current render tree is classified
- **THEN** every hero-bearing surface is mapped to a centrally defined additional-content style and any named content or row policy it uses
- **AND** an unmapped surface is non-conforming rather than deferred to a child issue

### Requirement: Structural variation uses an approved vocabulary

Structural or visual differences SHALL be represented by centrally defined named variants or policies. A screen SHALL NOT introduce arbitrary component geometry, colours, styles, borders, or renderer callbacks as a local override. Any approved override SHALL live in the central component, arrangement, theme, or future bespoke component/arrangement that owns it; surface code may only select the named option and provide semantic data.

#### Scenario: An existing component has a legitimate structural difference
- **WHEN** a screen requires a difference in layout, spacing, image placement, or decoration
- **THEN** the difference is selected through an existing approved policy or added as a centrally defined variant
- **AND** the component continues to own the resulting geometry and painting
- **AND** the policy or variant implementation lives in the central component or arrangement rather than in screen code

#### Scenario: A requested difference is content-only
- **WHEN** a screen differs only in displayed semantic content
- **THEN** the difference is represented in the screen model
- **AND** no new visual variant is created

### Requirement: Semantic styling is centrally defined

UI components SHALL consume semantic theme roles or component style policies rather than requiring callers to provide arbitrary raw colours or styles. Primitive palette values SHALL NOT be the normal screen-level styling API.

#### Scenario: A component renders focused and unfocused states
- **WHEN** the component receives its focus state
- **THEN** it resolves the appropriate semantic surface and text styles through the design system
- **AND** the screen does not select independent foreground and background colours

#### Scenario: A new semantic role is needed
- **WHEN** an existing role cannot express a required visual meaning
- **THEN** the role is added to the central theme vocabulary with component-level tests
- **AND** callers do not create a screen-local colour alias

### Requirement: Bespoke rendering is explicit

Rendering that cannot use an existing component, arrangement, or approved variant SHALL be isolated as an explicitly identified bespoke surface and SHALL include a documented reason and focused verification. An explicitly bespoke surface is still centralised in a named component or arrangement and remains subject to the same ownership, semantic styling, interaction geometry, and verification requirements.

#### Scenario: A new surface cannot use the design system
- **WHEN** implementation requires a bespoke painter
- **THEN** the surface is placed in an explicitly designated bespoke area
- **AND** its design-system exception, visual contract, and test coverage are reviewable

### Requirement: UI development guidance is discoverable

The repository SHALL provide mandatory UI rules in `AGENTS.md` and a committed `mbv-frontend` skill that directs agents to the component ownership model, controlled override vocabulary, reuse workflow, and verification expectations.

#### Scenario: An agent starts a TUI change
- **WHEN** an agent begins modifying a terminal UI screen
- **THEN** the repository guidance directs it to the `mbv-frontend` workflow
- **AND** the workflow requires checking for an existing component or arrangement before adding rendering code

#### Scenario: An agent completes a UI change
- **WHEN** an agent reports a UI change as complete
- **THEN** the guidance requires checking component ownership, narrow-width behaviour, interaction targets, and focused Ratatui buffer tests where applicable

### Requirement: Common bypasses are mechanically visible

The development workflow SHALL include code-based source or module checks that make direct screen painting and arbitrary screen-level styling visible before implementation is considered complete. Unit and buffer tests MAY verify component behavior, but test assertions SHALL NOT be the sole mechanism for establishing design-system conformance.

#### Scenario: A screen bypasses a canonical painter
- **WHEN** a change adds direct rendering or raw styling in a screen module outside an approved UI boundary
- **THEN** the relevant check or review checklist identifies the bypass
- **AND** the change cannot be treated as conforming without an explicit exception
