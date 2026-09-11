## REMOVED Requirements

### Requirement: Common bypasses are mechanically visible

## ADDED Requirements

### Requirement: Screen modules do not paint

Screen modules SHALL NOT paint directly: no direct Ratatui painting, no
layout-rect construction, and no buffer access. A screen supplies typed content;
painting belongs to an owning component or arrangement.

A call site SHALL state the styling role it sets explicitly. Supplying a bare
colour value where a style is expected silently sets the foreground and leaves
the intended background unpainted, so it is not conforming.

Nothing enforces this set of rules mechanically. It is a review obligation
carried by the module table and the `mbv-frontend` completion checklist; a green
build is not evidence that it holds.

Duplicated arrangement geometry and hit targets that have drifted from their
painting are review's responsibility for the same reason: they are not statically
detectable. Buffer tests verify component behaviour and preserved output; they do
not by themselves establish conformance.

#### Scenario: A screen bypasses a canonical painter
- **WHEN** a change adds direct rendering or rect construction in a screen module
- **THEN** review rejects the change
- **AND** the code moves to the component or arrangement that owns the geometry or
  painting, or out of `screens/` because it was never screen code

#### Scenario: A bare colour is supplied where a style is expected
- **WHEN** a change paints a block or widget by supplying a bare colour value in place of a style
- **THEN** the call site names the foreground or background role it intends to set
- **AND** the change is not conforming until it does

#### Scenario: Painting code that is not screen code
- **WHEN** code that owns geometry or painting sits in a screen module for
  historical reasons
- **THEN** it is rehomed to the arrangement, component, or shell module that its
  signature identifies as its owner
- **AND** the observable painted output is unchanged
