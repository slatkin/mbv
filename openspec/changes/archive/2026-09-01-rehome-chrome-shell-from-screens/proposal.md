## Why

`ast-grep scan` reports 52 `rules/frontend-boundary/` violations, all in
`src/app/render/screens/root.rs` and `src/app/render/screens/queue.rs` (issue
#635). These are **not** unmigrated TuiRealm surfaces: the
`interactive-component-boundary` rules that gate #603 scan clean, and
`docs/architecture/interactive-surface-ledger.md` records every interactive row
as `migrated` on 2026-08-27.

The two files are the application **chrome shell** — frame column geometry, the
queue panel frame, the queue title/scope pill row, the column backdrops, and the
`compose_base_frame` draw orchestrator called from `shell_run.rs`. They were
file-moved into a directory named `screens/` by the 2026-08-23 design-system
change without a per-function split, so a path-scoped linter reports them as
screen bypasses. That change declared the debt explicitly and accepted it as a
ratchet (`archive/2026-08-23-enforce-mbv-ui-design-system/tasks.md` task 3.6;
`design.md` "Risks"), on the expectation that later surface migrations would
retire it incrementally. They did not, because the intervening week's work
(#603) was aimed at interaction ownership, not painting.

The ratchet has not closed on its own, and while it is open CI cannot run a bare
`ast-grep scan` — `.github/workflows/architecture-boundaries.yml` scopes to
`src/app/components/`, so the frontend-boundary rules gate nothing in CI at all.

## What Changes

Rehome the chrome shell to its owning arrangement, components, and shell, in
three independent units. No user-visible behaviour changes; this is a
conformance and ownership change.

- **Unit 1 — chrome geometry to an arrangement.** Move
  `App::compute_chrome_geometry` (`screens/root.rs:75-182`, paint-free: takes a
  `Rect`, returns `FrameChromeGeometry`, renders nothing) into a new
  `render/arrangements/chrome.rs` as a free function. It is an arrangement by
  the spec's own signature classification and is in `screens/` only by
  historical accident. Clears ~12 `no-rect-construction` hits.
- Delete the identity split at `screens/root.rs:232`
  (`Layout::vertical([Constraint::Min(0)]).areas(area)` over the full area
  yields `main_area == area`). Clears the sole `no-layout-in-screens` hit.
- **Unit 2 — queue panel chrome to components.** Move `render_queue_title`
  (all of `screens/queue.rs`, one function: title pill + Local/Remote scope
  pills) and `render_main`'s queue-panel block (title placement, content-area
  carve, playlist/autosave status pill row, `screens/root.rs:479-546`) into the
  existing `render/components/queue.rs` / a queue panel arrangement. Clears
  ~35 hits and deletes `screens/queue.rs` entirely.
- **Unit 3 — remaining chrome and the draw orchestrator.** Move
  `paint_legacy_chrome`'s two column backdrops into `render/components/chrome.rs`,
  and settle where `compose_base_frame` and `render_main` live: they are the
  shell's draw entry (called from `shell_run.rs:84`), not screen code. Clears the
  remaining hits and empties `screens/root.rs` of Ratatui imports.
- **Broaden CI to the full tree.** Replace
  `ast-grep scan src/app/components/` with a bare `ast-grep scan` in
  `.github/workflows/architecture-boundaries.yml` and delete the comment
  explaining why the scan is scoped. This lands only after unit 3.
- Update `docs/architecture/interactive-surface-ledger.md` and
  `.opencode/skills/mbv-frontend/SKILL.md` (plus its `.agents/` and `.codex/`
  mirrors) where they describe the frontend-boundary checks as a ratchet on
  touched code rather than a whole-tree gate.
- **Non-goals:** no change to painted output, frame composition order, hit
  geometry, `AppLayout` publication, queue/playback behaviour, or the
  `interactive-component-boundary` rules. No new arrangement or component
  vocabulary beyond the existing `chrome*`/`queue` modules. `render_library`,
  `render_status_bar`, `render_tabs`, and the other already-extracted painters
  are untouched.

## Capabilities

### New Capabilities
<!-- None. This change brings existing code into conformance with an existing
     requirement; it introduces no new behaviour. -->

### Modified Capabilities
- `ui-design-system`: the "Common bypasses are mechanically visible" requirement
  currently mandates that path-scoped checks exist. It does not require them to
  pass over the whole tree, which is what permitted the scoped CI job and the
  open ratchet. Add the whole-tree enforcement obligation: the checks SHALL run
  unscoped in CI and the tree SHALL be clean, so a new bypass fails the build
  rather than being absorbed into a standing violation count.

## Impact

- **Code:** `src/app/render/screens/root.rs` (609 lines, reduced to zero or
  deleted), `src/app/render/screens/queue.rs` (243 lines, deleted),
  `src/app/render/screens/mod.rs`, new `src/app/render/arrangements/chrome.rs`,
  `src/app/render/arrangements/mod.rs`, `src/app/render/components/chrome.rs`,
  `src/app/render/components/queue.rs`, `src/app/render/mod.rs` (re-export
  seam), and `src/app/shell_run.rs` / `src/app/shell_browser.rs` if
  `compose_base_frame` moves. `src/app/layout.rs` if `FrameChromeGeometry`'s
  doc comments name the old owner.
- **Tests:** the ~20 call sites of `compose_base_frame` across
  `src/app/render/test_helpers.rs`, `shell_browser_tests.rs`,
  `tests_conformance_matrix.rs`, `tests_context_menu_placement.rs`, and
  siblings must keep compiling and passing byte-identical output. These are the
  characterization coverage this change relies on; no new characterization
  tests are required because the moved code is already covered.
- **Tooling/CI:** `.github/workflows/architecture-boundaries.yml` broadens to a
  full-repo scan. `sgconfig.yml` and `rules/frontend-boundary/*.yml` are
  unchanged.
- **Docs:** `docs/architecture/interactive-surface-ledger.md`,
  `.opencode/skills/mbv-frontend/SKILL.md` and mirrors.
- **Issues:** closes #635; relates to #607 (`compose_base_frame`'s naming and
  home).
