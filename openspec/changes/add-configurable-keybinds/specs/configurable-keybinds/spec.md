# configurable-keybinds

## Purpose

Lets users adapt mbv's keyboard to their own muscle memory and add commands without colliding with the hard-coded single-letter keyspace: a user-defined prefix key opens a personal binding namespace (multiplexer-style), router-owned global chords are rebindable from the config file, and the current bindings are discoverable in the help sidebar and the status bar.

## ADDED Requirements

### Requirement: Keys configuration section

The user config SHALL support a `[keys]` section with a configurable prefix chord, prefix-namespace chord assignments, and overrides for the router-owned global chords. When the section is absent or partial, every binding SHALL fall back to today's hard-coded defaults. Chord names in the section SHALL use a documented textual chord grammar (modifiers plus key, e.g. `"Ctrl+b"`, `"F8"`, `"Shift+Left"`).

#### Scenario: Defaults without a keys section

- **WHEN** the config file contains no `[keys]` section
- **THEN** every binding behaves exactly as before this change, including the absence of a prefix

#### Scenario: Partial override patches defaults

- **WHEN** the config overrides only one global action's chord
- **THEN** that action follows the configured chord and every other binding keeps its default chord

### Requirement: Reserved and colliding chords are rejected at load

Config validation SHALL reject, at load time with an error naming the offending entry, a `[keys]` value that assigns a reserved chord (initially `Ctrl+q`), reuses the prefix chord for another global action, or maps a prefix chord to an unknown command. The reserved-chord set SHALL be a fixed, extensible list; reserving a chord SHALL NOT require rejecting every other chord.

#### Scenario: Prefix set to a reserved chord

- **WHEN** the config sets the prefix to `Ctrl+q`
- **THEN** configuration fails to load with an error naming `Ctrl+q` as reserved

#### Scenario: Unknown prefix command

- **WHEN** a `[keys.bind]` entry names a command the application does not offer
- **THEN** configuration fails to load with an error naming the unknown command

### Requirement: Prefix mode arming

Pressing the configured prefix chord SHALL arm prefix mode, and the prefix chord itself SHALL have no other effect (it is consumed). Arming SHALL be suppressed while a text-entry surface owns focus and while a blocking overlay is open; in those states the prefix chord SHALL route exactly as it does without this change. Armed state SHALL persist until a disarm event occurs; there SHALL be no timed expiry.

#### Scenario: Prefix arms and is consumed

- **WHEN** the user presses the configured prefix chord with no text entry focused and no blocking overlay open
- **THEN** prefix mode is armed and the chord performs no other action

#### Scenario: Prefix does not arm while typing

- **WHEN** the prefix chord is pressed while a text-entry surface owns focus
- **THEN** prefix mode is not armed and the chord reaches the text entry as an ordinary character

#### Scenario: Prefix does not arm under a blocking overlay

- **WHEN** the prefix chord is pressed while a blocking overlay is open
- **THEN** prefix mode is not armed and the chord is handled by the overlay routing rules as before

### Requirement: Prefix namespace capture and disarm

While prefix mode is armed, the next keyboard chord SHALL resolve only against the prefix-namespace assignments: a mapped chord SHALL execute its command and disarm; an unmapped chord or Escape SHALL be consumed and disarm; the prefix chord itself SHALL re-arm and remain consumed. While armed, no keyboard chord SHALL reach the focused component or any other surface, regardless of focus. Any mouse event SHALL silently disarm prefix mode without altering the mouse event's normal handling.

#### Scenario: Mapped prefix chord runs its command

- **WHEN** a chord assigned in `[keys.bind]` is pressed while armed
- **THEN** the assigned command executes and prefix mode disarms

#### Scenario: Unmapped chord disarms without effect

- **WHEN** an unassigned chord (or Escape) is pressed while armed
- **THEN** nothing executes, the chord does not reach any surface, and prefix mode disarms

#### Scenario: Double prefix re-arms

- **WHEN** the prefix chord is pressed while already armed
- **THEN** prefix mode stays armed and the chord performs no other action

#### Scenario: Mouse disarms silently

- **WHEN** any mouse event arrives while armed
- **THEN** prefix mode disarms and the mouse event is handled exactly as if prefix mode had never been armed

### Requirement: Armed state is visible in the status bar

While prefix mode is armed, the status bar SHALL display an armed indicator; when not armed, the indicator SHALL NOT be displayed. The indicator SHALL NOT alter any other status-bar element's behavior.

#### Scenario: Indicator follows armed state

- **WHEN** prefix mode arms and then disarms
- **THEN** the status bar shows the armed indicator while armed and does not show it after disarming

### Requirement: Global chord rebinding

The router-owned global actions SHALL be rebindable to a single configured chord each. A rebound action SHALL fire on its configured chord and SHALL no longer fire on its default chord. Keys that are not router-owned globals — playback letters, Escape behavior, the visualizer toggle, queue column width, selection-modal handling, and all leaf-component keys — SHALL NOT be rebindable.

#### Scenario: Rebound action fires on the new chord

- **WHEN** a global action is rebound to a new chord and the user presses that chord in a state where the action is normally available
- **THEN** the action executes

#### Scenario: Default chord of a rebound action is inert

- **WHEN** a global action is rebound away from its default chord and the user presses the default chord
- **THEN** that action does not execute (the chord follows ordinary routing for an unbound key)

### Requirement: Router guarantees hold under configured bindings

With any valid `[keys]` configuration, the router's existing guarantees SHALL be unchanged: a global chord SHALL NOT fire while a text-entry surface owns focus (the character reaches the field, except the sidebar-opening function keys that remain router-owned), blocking overlays SHALL withhold underlying input, and policy layers SHALL keep their documented precedence.

#### Scenario: Rebound global respects text entry

- **WHEN** a rebound global chord is pressed while a text-entry surface owns focus
- **THEN** the rebound action does not fire and the character reaches the text entry

#### Scenario: Rebound global respects blocking overlays

- **WHEN** a rebound global chord is pressed while a blocking overlay is open
- **THEN** the rebound action does not fire and the overlay routing rules apply

### Requirement: Bindings are exposed to the user

The help sidebar SHALL present the current keyboard bindings, including the configured prefix and any overrides, generated from the same configuration the router resolves. The presented bindings SHALL match actual behavior: after any valid configuration load, every displayed chord for an action SHALL be the chord that fires it.

#### Scenario: Help reflects an override

- **WHEN** a global action is rebound and the user opens help for a destination that lists that action
- **THEN** help shows the configured chord, not the default

#### Scenario: Help reflects the prefix namespace

- **WHEN** a prefix is configured and the user opens help
- **THEN** help lists the prefix and its assigned commands
