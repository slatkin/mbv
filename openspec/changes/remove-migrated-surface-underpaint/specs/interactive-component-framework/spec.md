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

A `migrated` surface SHALL have exactly one painter for each frame at its
active layout breakpoint. The shell SHALL NOT run a legacy surface painter for
a surface body that a mounted component paints in the same frame. Verification
is execution ownership — the legacy painter is demonstrably not reached for
that surface at that breakpoint — not final-buffer similarity.

The per-frame geometry computation that components read (the `AppLayout` and
equivalent facts) MAY be shared shell code and is not a "parallel legacy
framework"; it paints nothing that a component owns.

Where a surface has a component variant at one breakpoint and only a legacy
renderer at another (for example a wide workspace component and a narrow
legacy body), the legacy renderer at the breakpoint with no component is the
**sole** painter for that breakpoint. The ledger row SHALL state this
explicitly so it is not mistaken for an underpaint.

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

#### Scenario: A migrated surface body is painted once per frame

- **WHEN** a mounted component is the active painter for a surface at the
  current layout breakpoint
- **THEN** the legacy renderer for that surface body is not reached that frame
- **AND** a debug assertion or test counter that fires when the legacy painter
  runs while the component is active stays silent across the surface's render
  characterization tests

#### Scenario: Geometry is computed without painting owned surfaces

- **WHEN** the shell computes the per-frame layout that components read
- **THEN** that computation produces the `AppLayout` and equivalent facts
- **AND** it paints no surface body that a component owns this frame

#### Scenario: Startup and steady-state frames paint identically

- **WHEN** the first full frame is drawn at startup and any later frame is drawn
- **THEN** both go through the same shell draw entry point
- **AND** the first frame includes the component views, showing loading
  affordances rather than a chrome-only frame followed by a component pop-in

#### Scenario: A breakpoint with no component keeps a sole legacy painter

- **WHEN** a surface is shown at a layout breakpoint for which no component
  variant exists (for example narrow TV or narrow Music)
- **THEN** the legacy renderer is the only painter for that surface at that
  breakpoint
- **AND** the ledger row records "wide: component; narrow: sole legacy
  renderer" so the endpoint is unambiguous
