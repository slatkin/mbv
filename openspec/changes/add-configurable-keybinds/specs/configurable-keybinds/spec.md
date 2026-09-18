# configurable-keybinds

## Purpose

Lets users adapt mbv's keyboard to their own muscle memory and add commands without colliding with the hard-coded single-letter keyspace: one declared keybind action table is the single source for defaults, configuration, routing, and presentation; a user-defined prefix key opens a personal binding namespace (multiplexer-style); the router-owned global and transport chords are rebindable from the config file; and the current bindings are discoverable in the settings panel and the help sidebar, grouped by the same sections the settings UI already uses.

## ADDED Requirements

### Requirement: Keybind actions are declared once

Every configurable keyboard action SHALL be declared in a single action table that carries, for each action, a stable identifier, its owning settings section, its default chord or chords, the activation it uses, and the gate that limits when it is eligible. The application SHALL derive from that declaration, without a second hand-maintained list: the compiled defaults, the config keys accepted at load, the chord matching used for routing, and the bindings presented to the user. An action that is present in routing but absent from the declaration, or presented to the user but absent from routing, SHALL NOT be possible.

Rendered key labels presented to the user for a declared action SHALL be the chords that action actually fires on for the loaded configuration.

#### Scenario: A declared default is the routed default

- **WHEN** no key configuration is present and the user presses a declared action's default chord in a state where its gate allows it
- **THEN** that action executes

#### Scenario: Presentation follows the declaration

- **WHEN** a declared action's configured chord is changed and the user views the bindings in the settings panel or help
- **THEN** the presented chord for that action is the configured chord

#### Scenario: Undeclared binding is not presented

- **WHEN** the set of configurable actions is read from the declaration
- **THEN** the presented binding list contains exactly those actions, with no entry that routing does not honor

### Requirement: Keys configuration section

The user config SHALL support a `[keys]` section containing a configurable prefix chord, per-section assignments that override an action's router-scope chord, and per-section prefix-namespace assignments. When the section is absent or partial, every action SHALL fall back to its declared default. Chord names SHALL use one documented textual chord grammar (modifiers plus key, e.g. `"Ctrl+b"`, `"F8"`, `"Shift+Left"`), with modifier order not significant.

An action SHALL be configured by its declared identifier. An assignment SHALL use a single configured chord for that action, replacing the declared default chord or chords.

#### Scenario: Defaults without a keys section

- **WHEN** the config file contains no `[keys]` section
- **THEN** every binding behaves as declared, including the absence of a prefix

#### Scenario: Partial override patches defaults

- **WHEN** the config overrides one action's chord
- **THEN** that action follows the configured chord and every other action keeps its declared default

#### Scenario: Alias defaults are replaced, not extended

- **WHEN** an action whose declared default is more than one chord is configured with one chord, and the user presses one of the declared default chords
- **THEN** the configured chord fires the action and the declared default chords do not

### Requirement: Bindings are grouped by the shared settings sections

The configured bindings SHALL be grouped by the same sections that the settings UI uses, in the same order, plus a `Global` section for chords whose behavior is not tied to one settings domain. The config file's per-action tables and the bindings presented in the settings UI SHALL use that one section vocabulary; a section SHALL NOT appear in one and not the other.

A `[keys]` assignment placed under a section other than its action's declared section SHALL be rejected at load.

#### Scenario: File sections match presented sections

- **WHEN** the user configures bindings in several sections and opens the bindings view in the settings panel
- **THEN** the presented groups are the same sections in the same order as the config file

#### Scenario: Section mismatch is rejected

- **WHEN** a `[keys]` assignment places an action under a section other than its declared section
- **THEN** configuration fails to load with an error naming the action and the section

#### Scenario: A section with no configured bindings is absent

- **WHEN** no binding in a section is configured and no declared action in it exists
- **THEN** that section contributes no table to the config file and no group to the presented bindings

### Requirement: Reserved, unknown, and colliding entries are rejected at load

Config validation SHALL reject, at load time with an error naming the offending entry: a reserved chord (initially `Ctrl+q`), an unparseable chord string, an unknown action identifier, a prefix-namespace assignment for an action that is not prefix-addressable, reuse of the prefix chord for another binding, two router-scope-configured actions resolving to the same chord, and two prefix-namespace-configured actions assigned the same chord. The reserved-chord set SHALL be a fixed, extensible list; reserving a chord SHALL NOT require rejecting every other chord.

#### Scenario: Prefix set to a reserved chord

- **WHEN** the config sets the prefix to `Ctrl+q`
- **THEN** configuration fails to load with an error naming `Ctrl+q` as reserved

#### Scenario: Unknown action

- **WHEN** a `[keys]` entry names an action the application does not offer
- **THEN** configuration fails to load with an error naming the unknown action

#### Scenario: Prefix collides with another binding

- **WHEN** the config sets the prefix to a chord already used by another configured or declared binding
- **THEN** configuration fails to load with an error naming the collision

#### Scenario: Two router-scope bindings collide

- **WHEN** the config assigns the same chord to two different actions in router scope
- **THEN** configuration fails to load with an error naming both actions and the shared chord

#### Scenario: Two prefix-namespace bindings collide

- **WHEN** the config assigns the same chord to two different actions in the prefix namespace
- **THEN** configuration fails to load with an error naming both actions and the shared chord

### Requirement: Rebindable action set

The rebindable actions SHALL be the router-owned global chords and the router-owned transport chords. The following SHALL NOT be rebindable: leaf-local keys owned by focused components, Escape's dismiss/back behavior in leaf contexts, the selection-modal handling, and policy entries that block rather than command.

A rebound action SHALL fire on its configured chord and SHALL no longer fire on its declared default chord.

#### Scenario: Rebound action fires on the new chord

- **WHEN** a rebindable action is configured to a new chord and the user presses it in a state where the action is normally available
- **THEN** the action executes

#### Scenario: Declared default of a rebound action is inert

- **WHEN** an action is rebound away from its declared default chord and the user presses that default chord
- **THEN** that action does not execute and the chord follows ordinary routing for an unbound key

#### Scenario: Leaf-local keys are not rebindable

- **WHEN** the config names a leaf-local key as an action
- **THEN** configuration fails to load because no such action is declared

### Requirement: Prefix mode arming

Pressing the configured prefix chord SHALL arm prefix mode, and the prefix chord itself SHALL have no other effect. Arming SHALL be suppressed while a text-entry surface owns focus and while a blocking overlay is open; in those states the prefix chord SHALL route as it does without this change. Armed state SHALL persist until a disarm event occurs; there SHALL be no timed expiry.

#### Scenario: Prefix arms and is consumed

- **WHEN** the user presses the configured prefix chord with no text entry focused and no blocking overlay open
- **THEN** prefix mode is armed and the chord performs no other action

#### Scenario: Prefix does not arm while typing

- **WHEN** the prefix chord is pressed while a text-entry surface owns focus
- **THEN** prefix mode is not armed and the chord reaches the text entry as an ordinary character

#### Scenario: Prefix does not arm under a blocking overlay

- **WHEN** the prefix chord is pressed while a blocking overlay is open
- **THEN** prefix mode is not armed and the chord is handled by the overlay routing rules

### Requirement: Prefix namespace capture and disarm

While prefix mode is armed, the next keyboard chord SHALL resolve only against the prefix-namespace assignments: a mapped chord whose action's declared gate currently allows it SHALL execute its action and disarm; a mapped chord whose action's declared gate does not currently allow it SHALL be treated as unmapped; an unmapped chord or Escape SHALL be consumed and disarm; the prefix chord itself SHALL re-arm and remain consumed. While armed, no keyboard chord SHALL reach the focused component or any other surface, regardless of focus. Any mouse event SHALL silently disarm prefix mode without altering the mouse event's normal handling.

#### Scenario: Mapped prefix chord runs its action

- **WHEN** a chord assigned in the prefix namespace is pressed while armed and the assigned action's gate currently allows it
- **THEN** the assigned action executes and prefix mode disarms

#### Scenario: Mapped prefix chord with a closed gate disarms without effect

- **WHEN** a chord assigned in the prefix namespace is pressed while armed and the assigned action's gate does not currently allow it
- **THEN** the assigned action does not execute, the chord does not reach any surface, and prefix mode disarms

#### Scenario: Unmapped chord disarms without effect

- **WHEN** an unassigned chord (or Escape) is pressed while armed
- **THEN** nothing executes, the chord does not reach any surface, and prefix mode disarms

#### Scenario: Escape while armed does not reach the stop action

- **WHEN** Escape is pressed while prefix mode is armed
- **THEN** prefix mode disarms and no transport action executes

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

### Requirement: Router guarantees hold under configured bindings

With any valid `[keys]` configuration, the router's existing guarantees SHALL be unchanged: a global or transport action SHALL NOT fire while a text-entry surface owns focus (the character reaches the field, except the sidebar-opening function keys that remain router-owned), blocking overlays SHALL withhold underlying input, actions whose eligibility depends on the focused component SHALL remain deferred until that component's disposition is known, and policy layers SHALL keep their documented precedence.

#### Scenario: Rebound action respects text entry

- **WHEN** a rebound action's chord is pressed while a text-entry surface owns focus
- **THEN** the action does not fire and the character reaches the text entry

#### Scenario: Rebound action respects blocking overlays

- **WHEN** a rebound action's chord is pressed while a blocking overlay is open
- **THEN** the action does not fire and the overlay routing rules apply

#### Scenario: A rebound transport action stays a deferred candidate

- **WHEN** a rebound transport action's chord is pressed while the focused component consumes that chord
- **THEN** the transport action does not fire and the component's local behavior stands

### Requirement: Bindings are exposed in the settings panel and help

The settings panel SHALL present the configured bindings in a dedicated destination, grouped by the shared section vocabulary, listing the configurable actions with their router-scope chord and, where assigned, their prefix-namespace chord. The settings panel's main list SHALL summarize the live configuration (the configured prefix and the number of actions whose binding deviates from the declared default). The help sidebar SHALL present the current bindings, including the configured prefix and any overrides, generated from the same declaration the router resolves.

The presented bindings SHALL match actual behavior: after any valid configuration load, every displayed chord for an action SHALL be the chord that fires it.

#### Scenario: Settings destination lists the configurable set

- **WHEN** the user opens the bindings destination from the settings panel
- **THEN** it lists exactly the configurable actions, grouped by the shared sections, and no action that routing does not honor

#### Scenario: Summary reflects the live configuration

- **WHEN** the config sets a prefix and overrides two actions, and the user views the settings main list
- **THEN** the bindings row shows the configured prefix and that two actions deviate from their declared defaults

#### Scenario: Help reflects an override

- **WHEN** a router-owned action is rebound and the user opens help for a destination that lists that action
- **THEN** help shows the configured chord, not the declared default

#### Scenario: Help reflects the prefix namespace

- **WHEN** a prefix is configured and the user opens help
- **THEN** help lists the prefix and its assigned actions
