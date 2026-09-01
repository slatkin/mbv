## MODIFIED Requirements

### Requirement: Common bypasses are mechanically visible

The repository SHALL include path-scoped source checks that flag direct Ratatui
painting, layout-rect construction, and buffer access inside screen modules.

These checks SHALL run unscoped over the whole repository in continuous
integration, and the tree SHALL be clean of their findings. A build SHALL NOT
narrow the scanned path in order to pass, and a standing violation count SHALL
NOT be treated as an accepted baseline: a new bypass fails the build rather than
being absorbed into existing findings. Code that a check flags is either moved
to its owning component or arrangement, or moved out of the checked path because
it was never screen code — never left in place behind a narrowed scan.

These checks catch the common bypass only. Duplicated arrangement geometry and hit
targets that have drifted from their painting are not statically detectable and
SHALL be named in the review checklist as review's responsibility. Buffer tests
verify component behaviour and preserved output; they do not by themselves establish
conformance.

#### Scenario: A screen bypasses a canonical painter
- **WHEN** a change adds direct rendering or rect construction in a screen module
- **THEN** the source check identifies the bypass
- **AND** the change cannot be treated as conforming without moving the code to its
  owning component or arrangement

#### Scenario: The checks are enforced over the whole tree
- **WHEN** continuous integration runs the architecture-boundary job
- **THEN** it runs the source checks across the whole repository rather than a
  subset of paths
- **AND** the job fails if any check reports a finding anywhere in the tree

#### Scenario: A bypass cannot be absorbed into a standing baseline
- **WHEN** a change would add a finding to a check that already reports findings
  elsewhere
- **THEN** the build fails on the new finding
- **AND** narrowing the scanned path, suppressing the rule, or raising an accepted
  violation count is not a conforming resolution

#### Scenario: Flagged code is not screen code
- **WHEN** a check flags code that owns geometry or painting but sits in a screen
  module for historical reasons
- **THEN** the code is rehomed to the arrangement, component, or shell module that
  its signature identifies as its owner
- **AND** the observable painted output is unchanged
