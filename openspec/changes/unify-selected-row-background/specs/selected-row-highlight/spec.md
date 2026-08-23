## Purpose

Defines the single selected-row background shared by every list surface, so
selection looks identical across screens and cannot drift per surface.

## ADDED Requirements

### Requirement: A selected row has one background definition

The selected-row background SHALL be defined in exactly one shared component.
Every surface that renders a list of selectable rows — the queue, Emby
libraries, the wide hero-on-left lists (music tracks, book browser, podcast
shows), the music album views, and the selection modal — SHALL obtain the
selected-row background from that component. No surface SHALL compute its own
selected-row rectangle or choose its own selection colour.

#### Scenario: Two surfaces show a selected row

- **WHEN** any two list surfaces each display a focused, selected row
- **THEN** the selected-row background is the same colour and the same shape on
  both, because both are painted by the shared component

#### Scenario: The selection appearance changes

- **WHEN** the selected-row colour or geometry is changed
- **THEN** the change is made in the one shared component
- **AND** every list surface reflects it without a per-surface edit

### Requirement: The selected-row background spans the full panel width

The shared component SHALL paint the selected row's background across the full
width of the row's parent panel (the enclosing list panel or inset box), one row
high, using the focus-resolved focused-surface colour role. Row content (marker,
text, trailing columns) is drawn over that background and MUST NOT reduce its
width; the background is not inset from the panel edges and is not shortened to
reserve space for images or columns.

#### Scenario: A row is selected in a panel

- **WHEN** a row is the focused selection within a list panel
- **THEN** its background fills the panel's full inner width for that row
- **AND** the background uses the focused-surface colour role

#### Scenario: The panel is unfocused

- **WHEN** the panel containing the selected row is not the focused pane
- **THEN** the selected-row background resolves to the unfocused surface colour
  through the shared focus resolution, not a surface-local override
