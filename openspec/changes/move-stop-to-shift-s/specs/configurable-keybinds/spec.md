# Spec Delta

## ADDED Requirements

### Requirement: The default Stop chord is not Escape and fires immediately

The default Stop chord SHALL NOT be `Esc`. With default bindings, `Esc` SHALL NOT
stop playback from any focus. It keeps its leaf meaning (back, close, clear) in
every focus and SHALL NOT carry timing state. Stop SHALL fire on the press of its
default chord, with no second press, wherever the transport gate allows it. That
includes a focus that holds a Visual selection. It SHALL NOT fire while a text
entry surface owns the keyboard or a blocking overlay is open.

#### Scenario: Escape does not stop with default bindings

- **WHEN** playback is active, no assignment is configured, and the user presses `Esc` once or twice in succession
- **THEN** no Stop is dispatched
- **AND** the focused component's own Escape meaning applies on each press

#### Scenario: The default Stop chord stops on the first press

- **WHEN** playback is active and the user presses the default Stop chord once from a focus where the transport gate is open
- **THEN** Stop is dispatched on that press

#### Scenario: Stop during a Visual selection

- **WHEN** a focused media list holds an active Visual selection and the user presses the default Stop chord
- **THEN** Stop is dispatched
- **AND** the Visual selection is unchanged

#### Scenario: Text entry keeps the character

- **WHEN** a text entry surface owns the keyboard and the user types the default Stop chord's character
- **THEN** no Stop is dispatched
- **AND** the character reaches the field

#### Scenario: A configured Escape stop chord still loads

- **WHEN** the config assigns `Esc` to the `stop` action
- **THEN** the config loads without error
- **AND** `Esc` follows the deferred-candidate rule of `semantic-input-arbitration`
