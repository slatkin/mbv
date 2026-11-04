# Architecture decision records

An ADR records a decision and its rationale. It is not a status report. Specs in
`openspec/specs/` hold current behavior and are kept in sync. ADRs are not, so a
status sentence in an ADR becomes false and stays false.

## Amendments

An amendment records a change to the decision. Follow these rules:

- Do not state the state of `main`: no protocol version numbers, no "on `main`
  now", no "shipped" or "not yet implemented".
- Do not state PR, issue, or milestone status: no "open PR", "tracked in
  milestone", or "planned".
- Do not write file paths with line numbers (`file.rs:NNN`). To say where
  something lives, name the crate, type, or function. Use a bare path only when the
  location itself is the fact.
- Write every `openspec/changes/` reference with its dated `archive/` directory
  (`openspec/changes/archive/YYYY-MM-DD-<name>/`). An undated reference breaks when
  the change archives.
- When the surrounding world changes, name the superseding ADR or the owning spec.
  For current truth, point at that spec or at a code symbol. Do not restate it.

Keep superseded banners and dated historical statements as they are. They are
decision history.
