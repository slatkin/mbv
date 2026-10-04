# Spec Delta

## ADDED Requirements

### Requirement: Pinned mini view follows window focus

When mbv runs in the pinned panel and the terminal is narrower than 80 columns, the displayed
panel SHALL render with the focused palette while the window holds focus and with the resting
palette while it does not. The window SHALL be treated as focused from launch until a focus-lost
report arrives. Outside the pinned panel, mini view SHALL keep rendering the resting palette
regardless of window focus. At 80+ columns, rendering SHALL NOT depend on window focus.

#### Scenario: Pinned mini view is focused while the panel holds the keyboard

- **WHEN** the pinned panel is narrower than 80 columns and the window holds focus
- **THEN** the displayed panel renders its focused palette

#### Scenario: Pinned mini view dims when focus leaves

- **WHEN** the pinned panel is narrower than 80 columns and a focus-lost report arrives
- **THEN** the displayed panel renders its resting palette until a focus-gained report arrives

#### Scenario: Launch starts focused

- **WHEN** a pinned launch starts narrower than 80 columns and no focus report has arrived
- **THEN** the displayed panel renders its focused palette

#### Scenario: Unpinned mini view is unchanged

- **WHEN** mbv is not pinned, the terminal is narrower than 80 columns, and any focus report arrives
- **THEN** the displayed panel renders its resting palette

#### Scenario: Wide view ignores window focus

- **WHEN** the terminal is 80 or more columns wide and a focus-lost report arrives
- **THEN** panels keep rendering by panel focus alone

## MODIFIED Requirements

### Requirement: Queue-only renders the queue panel focused when it holds focus

In queue-only state at 80+ columns the queue panel SHALL render with the same focused styling it has when focused in the `both` state whenever panel focus is on the queue: the focused background, cursor highlight, and scrollbar. Mini view is governed by `Pinned mini view follows window focus` instead: outside the pinned panel it renders the resting palette.

#### Scenario: Queue-only renders focused when the queue holds focus

- **WHEN** the layout is in queue-only state at 80+ columns and panel focus is on the queue
- **THEN** the queue panel SHALL render with its focused background, cursor highlight, and scrollbar

#### Scenario: Both keeps the focused appearance

- **WHEN** the layout is in `both` state and the queue panel is focused
- **THEN** the queue panel SHALL render with its focused background, cursor highlight, and scrollbar
