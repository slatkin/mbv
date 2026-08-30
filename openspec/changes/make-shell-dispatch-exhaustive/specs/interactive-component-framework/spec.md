# interactive-component-framework Specification (delta)

## ADDED Requirements

### Requirement: Every component request has a handler or a documented no-op, enforced by exhaustive matching

Every typed request a component emits across its authority boundary — each
`ShellRequest` variant and each intent sub-enum variant reaching the shell —
SHALL resolve in the shell's dispatch to either a real handler or an explicit
arm whose comment names why it is deliberately inert (mouse-only under D16, the
component owns the effect, consumed synchronously elsewhere, or the issue that
owns the missing wiring).

The shell's top-level request dispatch (`Model::handle_terminal_message`,
destructuring `Msg::Shell(request)`) SHALL be an exhaustive `match` over
`ShellRequest` with no wildcard arm, so that a request variant with no arm is a
compile error rather than a silent fall-through. A wildcard arm that catches
"unhandled request variant" and a documented no-op arm are not equivalent: the
first is an accident that repeats, the second is a recorded decision.

A wildcard (`_`) arm is permitted only in an inner sub-dispatcher that the
exhaustive top-level match has already narrowed to a fixed OR-group of variants,
and only with a comment stating the closed set it matches and why the wildcard
is unreachable. Such an arm SHALL NOT be the enforcement mechanism for handler
coverage — the top-level exhaustive match is.

#### Scenario: A new request variant without a dispatch arm fails compilation

- **WHEN** a `ShellRequest` variant is added and no arm is added to
  `Model::handle_terminal_message`
- **THEN** `cargo check -p mbv` fails, naming the unhandled variant
- **AND** the build cannot be made to pass by relying on a wildcard arm, because
  the top-level match has none

#### Scenario: A deliberately inert request is an explicit arm

- **WHEN** a request variant is emitted only from a mouse path, is fully handled
  by the emitting component, or is consumed by a synchronous handler before
  `handle_terminal_message`
- **THEN** its arm in the dispatch is an explicit no-op whose comment states
  that reason and the issue or precedent that owns it
- **AND** it is not folded into a catch-all arm

#### Scenario: An inner sub-dispatcher wildcard matches a proven closed set

- **WHEN** a shell sub-dispatcher (for example `handle_browser_request`) is
  reached only for a fixed OR-group of `ShellRequest` variants routed by the
  exhaustive top-level match
- **THEN** any `_` arm it carries has a comment naming that closed set and why
  the arm is unreachable
- **AND** removing or reordering the top-level OR-group that feeds it is what
  would change its reachability, not an unnoticed new variant
