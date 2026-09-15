## ADDED Requirements

### Requirement: Mouse capture follows the mouse_support setting

Mouse capture at the terminal SHALL be gated on a `mouse_support` setting
whose default value is `true` (mouse on). When the setting is `false`, the
TUI SHALL NOT enable terminal mouse capture at launch, and consequently no
mouse events SHALL be delivered to any component; keyboard behavior and all
non-mouse terminal setup (raw mode, alternate screen, focus-change,
keyboard-enhancement flags) SHALL be unaffected. Terminal teardown SHALL
remain unchanged: disabling capture on restore is unconditional and SHALL
remain a harmless no-op when capture was never enabled.

Changing the setting from the Settings panel's Display section SHALL apply
the new value to the live session immediately without a restart: enabling
starts mouse-event delivery, disabling stops it. The changed setting SHALL
persist through the existing settings-save path so it survives restart.

#### Scenario: Launch with mouse support disabled

- **WHEN** the TUI launches with `mouse_support = false`
- **THEN** terminal mouse capture is not enabled
- **AND** no mouse events are delivered to any component
- **AND** keyboard navigation behaves as it does when mouse support is on

#### Scenario: Default is mouse support enabled

- **WHEN** the TUI launches and no `mouse_support` value is configured
- **THEN** terminal mouse capture is enabled and mouse events are delivered
  exactly as before this capability existed

#### Scenario: Toggle off from the Settings panel

- **WHEN** the user toggles the mouse-support row in the Settings panel's
  Display section to off during a live session
- **THEN** mouse capture is disabled immediately without a restart
- **AND** no further mouse events are delivered for the remainder of the
  session
- **AND** the keyboard remains fully functional
- **AND** the new value is persisted to configuration

#### Scenario: Toggle on from the Settings panel

- **WHEN** the user toggles the mouse-support row in the Settings panel's
  Display section to on during a live session that started with mouse
  support off
- **THEN** mouse capture is enabled immediately without a restart
- **AND** subsequently delivered mouse events reach components under the
  existing delivery rules
- **AND** the new value is persisted to configuration
