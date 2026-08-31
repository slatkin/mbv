# interactive-component-framework Specification (delta)

## MODIFIED Requirements

### Requirement: Input precedence preserved through focus and subscriptions

The input-resolution model of ADR 0002 SHALL be preserved: a priority-ordered
stack of active contexts in which each context resolves a key to `Command`,
`Swallow`, or `FallThrough`, and the first context returning `Command` or
`Swallow` claims the key. Only `FallThrough` SHALL allow a lower-priority context
to receive the key. The active interactive leaf is selected by TuiRealm focus;
blocking overlays are active components that `Swallow` bound and unbound keys;
parent and global bindings are delivered through TuiRealm subscriptions plus mbv
key-policy code, without broadcasting state-changing events to every component.
The `CONTEXT_STACK` loop SHALL NOT be retained as a parallel routing endpoint, but
the precedence order and the Command/Swallow/FallThrough semantics it encodes
SHALL be preserved and remain locked by the existing input characterization tests.

Global bindings (those whose `KeyPolicyOwner::Sub` is `ComponentId::UiRoot`)
require that the focused leaf is not a text-entry component; otherwise the
focused leaf's character input stands as a typed request. The text-entry
condition is a plain-data projection of the focused leaf id, set by the
shell on the router's `RouterSnapshot`. A blocking overlay is itself the
focused leaf; the policy SHALL NOT silence the overlay's own typed
requests.

#### Scenario: A global binding does not fire when the focused leaf is a text-entry component

- **WHEN** the user types a character that matches a global binding (Quit on
  `q`, PanelModeCycle on `x`, Visualizer on `v`, LibraryTabJump on `1`–`9`,
  NextLibraryTab on `Tab`, PreviousLibraryTab on `BackTab`, CtrlL on
  `Ctrl+l`, F5 on `F5`, SearchOpen on `Ctrl+/` or `Ctrl+_`)
- **AND** the focused leaf is the search sidebar, the inline library search,
  or the settings sidebar's text-input fields
- **THEN** the leaf's character input stands as a typed request
- **AND** the global binding does not fire
