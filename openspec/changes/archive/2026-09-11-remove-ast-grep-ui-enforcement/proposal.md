# Remove ast-grep UI enforcement

## Why

The three path-scoped ast-grep rule sets (`rules/frontend-boundary/`,
`rules/interactive-component-boundary/`, `rules/media-list-boundary/` — 18 rules
with fixtures and snapshots), `sgconfig.yml`, and the
`architecture-boundaries` CI job are removed. They were a source-scan hammer
standing in for structure the language can express or for a claim the tree
already held.

Every rule that prohibited naming or calling something — `impl App`, `App` as a
type, `wide_hero_split`, the media-list paint adapters, the carrier mutators,
`left_row_map` — re-checked a decision Rust visibility already makes, and only
the rules did not hold the boundary: the borrow checker would have. The rules
that covered what visibility cannot express (a `screens/` module importing
ratatui, a deleted legacy `Msg` shape returning) were ratchets over a state the
tree had already reached.

The cost was not the 18 files. It was that "scan is clean" read as proof.
`docs/architecture/interactive-surface-ledger.md` cited the scan as each
surface's verification, and a guard can only freeze the axis it measures. The
receipts were structural: six exempted files in `no-second-router-site`, four
inline `ast-grep-ignore` suppressions carrying prose adjudications in
`no-render-media-list-mutators`, five ignores in
`no-destination-row-map-reconstruction`. Each was a place where a single-owner
claim was false and someone recorded the exception instead of fixing it.
Deviations from a boundary survive a clean scan of that boundary by
construction, which is the property that made the gates worth deleting.

## What Changes

- Delete `rules/`, `sgconfig.yml`, and
  `.github/workflows/architecture-boundaries.yml`.
- Delete the four inline `ast-grep-ignore` comments and the one scan-rule
  reference in `wide_hero_split`'s doc comment.
- Rewrite the `ui-design-system` requirement: the screen-painting and
  explicit-style obligations stay; the source-check, whole-tree-CI,
  no-narrowing, and no-baseline mandates go.
- Drop the scan from `AGENTS.md`, both `mbv-frontend` skill copies, the
  component map, and the interactive-surface ledger, whose rows now record a
  review obligation instead of a scan command.
- Correct the live change plans that name `ast-grep scan` or edit a deleted rule
  file: `add-configurable-keybinds`, `add-now-playing-sidebar`,
  `remove-shared-central-storage`, `unify-wide-hero-content-box-frame`.

## Non-goals

- Not replacing the rules with a different automated gate (dylint, custom
  clippy lint, crate split, visibility hardening). Those are separate decisions.
  This change removes the mechanism and states what it leaves unguarded.
- Not removing the ast-grep *tool* or its agent skills; structural search stays
  in use.
- Not editing `openspec/changes/archive/**`; archived changes keep the record of
  a gate that existed.

## Impact

- The per-surface boundary is now carried by the module table, the component map
  `Enforcement` section, and the `mbv-frontend` completion checklist. Nothing
  fails a build when a component takes `App`, a Service client, or an `mpsc`
  channel again; the compiler sees those as ordinary code.
- `src/app/components/**` and `src/app/render/arrangements/wide_hero.rs` lose
  five comment lines. No behaviour change.
- CI loses one job; local gates lose one command.
