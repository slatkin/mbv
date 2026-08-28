## MODIFIED Requirements

### Requirement: Complete conversion with no mixed-framework endpoint

The migration MAY use internal checkpoints, behaviour-preserving commits, and
temporary adapters, but a mixed TuiRealm/legacy framework SHALL NOT be a
completed or mergeable endpoint. Completion requires that every row in
`docs/architecture/interactive-surface-ledger.md` is `migrated`; every
independently interactive surface is a TuiRealm `AppComponent`; component-local
state, handlers, and render adapters are removed from `App` rather than mirrored;
`CONTEXT_STACK` interaction dispatch, the global mouse router and hit map, and
duplicated mouse paths are removed; render-only layout state MAY remain; all
temporary interaction adapters and state mirrors are removed; and no parallel
legacy interaction framework remains.

The ledger, ADR 0022, and the source SHALL NOT contradict one another. A
`ComponentId` variant, ledger row, or documented owner that names a component
module which does not exist is such a contradiction and SHALL be reconciled by
deleting the phantom reference or implementing the component — whichever
matches the mechanism the code actually uses.

A keyboard-policy owner tag SHALL name a component that is really mounted at
the time the chord can fire, or `UiRoot` for a global chord. The single
keyboard router classifies a binding as global-versus-focused by its owner
tag; an owner that names an unmounted or non-existent component misclassifies
the binding and can leak a global chord into a focused text-entry surface.

#### Scenario: A converted surface does not regain App-owned state

- **WHEN** a surface has been marked `migrated`
- **THEN** it does not reintroduce `App`-owned local state, input handling, or
  rendering
- **AND** the old fields and handlers are deleted, not synchronised with a mirror

#### Scenario: A mid-migration surface is tracked as `component`, not `migrated`

- **WHEN** a surface's Interactive Component has landed and paints the surface,
  but the shell still mirrors `App` state into it or legacy input still
  forwards to an `impl App` handler
- **THEN** its ledger row reads `component`, which is a permitted internal
  checkpoint and not a completed conversion
- **AND** the row becomes `migrated` only once the `App` state and handlers are
  deleted and the mirror is removed
- **AND** the completion gate requires no `legacy` and no `component` row to
  remain

#### Scenario: Migration preserves existing contracts

- **WHEN** any surface is converted
- **THEN** existing keyboard precedence, responsive behaviour, images-disabled
  behaviour, and render characterization coverage remain satisfied
- **AND** mouse parity is required only for the alpha-supported mouse paths named
  by this capability

#### Scenario: A ledger row and ComponentId agree with the code

- **WHEN** a ledger row or `ComponentId` variant names a component module
- **THEN** that module exists and is mounted by the shell
- **AND** if the surface's ownership is instead pure derivation over shell
  state with no component, the ledger row describes that mechanism and no
  `ComponentId` variant is reserved for the absent component

#### Scenario: A global chord does not fire while a text field is focused

- **WHEN** a global chrome chord (for example the panel-mode cycle) is pressed
  while a text-entry surface owns focus
- **THEN** the router treats the chord as global input, suppresses the command,
  and the character reaches the focused field
- **AND** this holds because the chord's keyboard-policy owner is `UiRoot`, not
  a component that is never mounted
