# semantic-input-arbitration Specification

## Purpose
Defines how one terminal key becomes a single debuggable result when central keyboard policy and focused component-local interpretation both participate, without mirroring component-local state into the router.

## Requirements

### Requirement: One arbiter combines policy and leaf disposition

For each terminal key event, the system SHALL combine the Keyboard Router's ordered policy result and the focused Interactive Component's explicit local disposition at one central arbitration site. The disposition SHALL distinguish input that was unhandled from input consumed by local behavior, whether or not that behavior also emits an external request. No destination, shell compatibility handler, or subscription SHALL perform a second arbitration.

#### Scenario: Local behavior consumes without an external effect

- **WHEN** the focused component recognizes a key and changes only its private interaction state
- **THEN** it reports that the key was consumed without emitting an effect request
- **AND** arbitration does not treat the absence of an effect request as unhandled input

#### Scenario: Leaf does not recognize a key

- **WHEN** the focused component does not recognize a key
- **THEN** it reports the key as unhandled
- **AND** central policy alone determines whether a global candidate runs, is swallowed, or falls through

#### Scenario: One event has one final disposition

- **WHEN** the focused component and Keyboard Router both observe the same key event
- **THEN** the central arbiter records one final disposition and applies at most one global command
- **AND** no destination independently resolves the competition

### Requirement: Local-state projections are not routing authority

A component MAY publish a read-only summary of private interaction state when another component must present it. Such a summary SHALL NOT become writable selection state, SHALL NOT be pushed back into the owner, and SHALL NOT determine keyboard precedence. Routing SHALL use the focused component's disposition for the current event rather than a cached summary.

#### Scenario: Status summary is stale

- **WHEN** a rendered selection-count summary has not yet caught up with the owning list
- **THEN** the focused list's current disposition still determines whether its key is consumed
- **AND** keyboard behavior does not depend on the stale count

#### Scenario: Summary changes status presentation only

- **WHEN** a list's multi-selection count changes
- **THEN** the corresponding status presentation may update from the emitted summary
- **AND** the shell does not gain authority to mutate or reconstruct the selected targets

### Requirement: Interaction origin survives focus changes

An interaction that outlives its originating input event SHALL carry a stable origin identifying the owning list. Later effects, dismissal, and local-state clearing SHALL address that origin rather than infer an owner from current panel focus.

#### Scenario: Context menu outlives its source focus

- **WHEN** a context menu opens for a Library or Queue multi-selection and focus subsequently belongs to the overlay
- **THEN** the menu retains the originating list identity and ordered resolved targets
- **AND** executing an action clears only the originating list's multi-selection

#### Scenario: Both panels retain selections

- **WHEN** Library and Queue each have a multi-selection and panel focus changes between them
- **THEN** both owners retain their independent selections
- **AND** the status presentation and keyboard interaction reflect only the currently focused list

#### Scenario: Focused selection is cleared from status

- **WHEN** the user invokes the status presentation's clear control
- **THEN** the clear request is sent to the list identified as focused when the control was invoked
- **AND** a selection retained by the other visible panel is unchanged

### Requirement: Context-sensitive global actions are deferred candidates without timing state

A global action whose eligibility depends on whether the focused component consumes the same key SHALL remain a deferred candidate until the focused component's disposition is known. Consumed local input SHALL cancel the candidate. Unhandled local input SHALL fire the candidate on that press; there SHALL be no candidate timing state, repeated-press window, or deferred arming. Immediate global commands and blocking-overlay swallows SHALL retain their existing precedence.

#### Scenario: Visual Space suppresses playback

- **WHEN** a focused media list consumes `Space` for an active local Visual selection
- **THEN** the playback candidate does not fire
- **AND** the row-local toggle remains applied

#### Scenario: Visual Escape suppresses stop

- **WHEN** a focused media list consumes `Esc` to clear its active Visual selection
- **THEN** the stop candidate does not fire
- **AND** the local selection clearing remains applied

#### Scenario: Unhandled Space fires immediately

- **WHEN** the focused component reports `Space` as unhandled and a playback candidate is eligible
- **THEN** the playback action fires on that press
- **AND** no timing state is recorded for a later press

#### Scenario: Unhandled Escape fires immediately

- **WHEN** the focused component reports `Esc` as unhandled and a stop candidate is eligible
- **THEN** the stop action fires on that press
- **AND** no timing state is recorded for a later press

#### Scenario: No candidate timing survives a consumed press

- **WHEN** the focused component consumes a context-sensitive key and a later press of the same key is unhandled
- **THEN** the later press behaves as a first press, with no inherited candidate state
