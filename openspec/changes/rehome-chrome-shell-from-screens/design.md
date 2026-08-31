## Context

See `proposal.md` — Why. The current state that shapes the approach:

`src/app/render/screens/` holds seven modules. Five are genuine screens and scan
clean. Two — `root.rs` (609 lines) and `queue.rs` (243) — carry all 52
`rules/frontend-boundary/` findings. They are not unmigrated surfaces; they are
the chrome shell, file-moved into `screens/` by the 2026-08-23 design-system
change without a per-function split.

Their contents, by what each function actually is:

```
screens/root.rs
  compute_frame_layout      &mut self  terminal normalization + delegate   → shell
  compute_chrome_geometry   &self      Rect -> FrameChromeGeometry, no paint → arrangement
  compose_base_frame        &mut self  draw orchestrator, sole caller        → shell
                                       shell_run.rs:84
  render_main               &mut self  dispatch + queue-panel painting       → split
  paint_legacy_chrome       &mut self  two column backdrops                  → component

screens/queue.rs
  render_queue_title        &mut self  title pill + Local/Remote scope pills → delete (see D2)
```

The spec's own classification rule (`ui-design-system`, "Screens use canonical UI
ownership boundaries") settles most of this by signature: *"a function placing
components within a `Rect` and owning breakpoints is an arrangement."*
`compute_chrome_geometry` takes a `Rect`, returns a typed geometry struct, and
paints nothing. It is an arrangement that has been sitting in `screens/`.

The destination modules already exist: `render/arrangements/`,
`render/components/chrome.rs`, `chrome_tabs.rs`, `chrome_status.rs`,
`chrome_player.rs`, and `render/components/queue.rs`.

## Goals / Non-Goals

**Goals**

- Every function ends up in the module its signature identifies as its owner.
- `ast-grep scan` (unscoped) is clean, and CI runs it unscoped.
- Painted output is byte-identical at every step, verified by the existing
  characterization suite, except where D2 documents a deliberate resolution of an
  existing divergence.

**Non-Goals**

- No new arrangement or component vocabulary. Everything lands in a module that
  already exists, or in one new `arrangements/chrome.rs`.
- Not resolving #607's naming question (`compose_base_frame` vs. a shell method).
  D3 moves the code; what it is called is left alone.
- No change to `AppLayout`'s role as render-only published geometry, beyond
  deleting the two fields D2 makes dead.

## Decisions

### D1: `compute_chrome_geometry` becomes a free function in `arrangements/chrome.rs`

It is `&self` and reads only `effective_panel_mode()`, `terminal_width/height`,
`queue_column_width`, `panel_mode`, and focus state. Extract it as
`pub(in crate::app) fn chrome_geometry(input: ChromeGeometryInput) -> FrameChromeGeometry`,
with a small input struct carrying those reads, and leave a thin `&self` shim at
the call site that builds the input.

**Why a free function over `impl App` in a new file:** an `impl App` block moved
verbatim to `arrangements/chrome.rs` clears the ast-grep findings (the rule is
path-scoped) while changing nothing about ownership — the exact move that created
this problem in the first place. `rules/interactive-component-boundary/no-impl-app.yml`
already encodes the project's position on this for `components/`. Doing the same
by hand here keeps the fix honest.

**Alternative rejected — widen the rule's `ignores` to exempt chrome files.**
Cheapest diff, and precisely the "narrowing the scan to pass" that the spec delta
now forbids.

**Alternative rejected — move the files and stop.** Clears 52 findings in one
commit with no ownership change. Same objection.

Also delete `screens/root.rs:232`:
`let [main_area] = Layout::vertical([Constraint::Min(0)]).areas(area);` — a
single `Min(0)` over the whole area is the identity split, so `main_area == area`.
This is the only `no-layout-in-screens` finding in the tree and it is dead code,
not a boundary violation to migrate.

### D2: `screens/queue.rs` is deleted, not migrated — `QueueComponent` is already its owner

This is the finding that changes the shape of the work. `render/components/queue.rs`
already exports `App::queue_title_model() -> QueueTitleModel` and
`render_queue_title_content()`, and `QueueComponent::view`
(`src/app/components/queue.rs:417`) already calls it. The migrated path exists and
is mounted.

What is left is an incomplete underpaint teardown — the same pattern the archived
`remove-queue-legacy-underpaint` and `remove-migrated-surface-underpaint` changes
handled for the queue *body*, stopped one function short:

```
shell_queue.rs:50
  title_area = (layout.main.queue_scope_local_area.height > 0).then(|| ...)
                       ▲
                       └── written ONLY by the legacy painter,
                           screens/queue.rs:228, and only after
                           its `if !show_split { return }` at :92

so:
  no remote session  → scope_local_area stays Rect::default()
                     → title_area = None
                     → QueueComponent paints NO title
                     → screens/queue.rs is the sole painter
  remote / attached  → legacy paints title + pills, publishes the areas
                     → component then repaints the SAME rect over it
```

The rects coincide: legacy uses `{x: qla.x+2, y: qla.y+1, w: qla.width-4, h:1}`
(`root.rs:494`); the shell derives `{x: queue_area.x+2, y: queue_area.y-2, ...}`
where `queue_area.y == qla.y + 3`. Same row.

The `title_overhead` reservation that makes `queue_area.y == qla.y + 3` remains
load-bearing until D3's queue-panel extraction; deleting the legacy title painter
must not remove or change that reservation early. So the fix is to derive
`title_area` from `layout.main.queue_area` unconditionally, delete
`screens/queue.rs` whole, and let `QueueComponent` be sole painter in both cases. `queue_scope_local_area` and
`queue_scope_remote_area` then have no non-test readers and come out of
`AppLayout` — the component already owns the equivalents in
`QueueRenderGeometry::scope_local_area/scope_remote_area`.

**This unit is a deletion of 243 lines and 20 findings, not a migration.** It is
also the only unit with real behavioural risk — see the first entry under Risks.

### D3: `compose_base_frame` and `render_main` move to the shell; the queue-panel block moves out first

`compose_base_frame` is `pub`, `&mut self`, and called from `shell_run.rs:84` and
~20 test helpers. It is the draw entry point, not screen content. Move it and
`render_main` to a shell-side module (`src/app/shell_draw.rs` or alongside
`shell_run.rs`), which takes them out of the ast-grep path entirely — legitimately,
because the shell is where a draw orchestrator belongs.

But move the queue-panel block (`root.rs:479-546`: panel frame, title placement,
content-area carve, playlist/autosave status pill row) into a queue panel
arrangement **before** the shell move, not after. Otherwise the move relocates
painting into the shell and the checks stop being able to see it — passing the
scan by leaving the boundary problem intact under a different path.

Order matters: D2 must land before D3's queue-panel extraction, because deleting
the legacy title painter removes the `title_overhead` coupling that the
content-area carve is written around.

`paint_legacy_chrome`'s two backdrop `render_widget` calls go to
`render/components/chrome.rs`, which already owns shared chrome painting.

### D4: CI broadens only after the tree is clean

`.github/workflows/architecture-boundaries.yml` keeps its `src/app/components/`
scope through units 1–3 and flips to a bare `ast-grep scan` in the final commit,
which also deletes the comment explaining the scope. Flipping earlier makes every
intermediate commit red.

## Risks / Trade-offs

- **[Risk] D2 flips the queue-title painter in the common no-remote case, and the
  two painters are not obviously equivalent.** The legacy path builds the local
  pill from `remote_status_spans(RemoteSlotState::Off, "")`; `queue_title_model`
  uses `remote_status_spans(remote_state, &daemon_endpoint)` and overwrites the
  label with `" Connected: "` when split. These may already diverge. →
  Characterization `TestBackend` tests land first, in their own commit, covering
  the title row in all four states (no remote / DirectRemote / AttachedSession
  mbv / AttachedSession non-mbv, × nerd-fonts on and off), per the
  `ui-design-system` "A surface is migrated" scenario. If they show a real
  divergence, resolve it explicitly and record which rendering wins — do not let
  the deletion silently change output.
- **[Risk] `compute_chrome_geometry`'s input struct drifts from `App` state.** →
  It is built at one call site (`compute_frame_layout`) and consumed at one
  (`chrome_geometry`); a struct with named fields makes a missed read a compile
  error rather than a stale value.
- **[Risk] The `AppLayout` publication ordering in `compose_base_frame` is
  load-bearing** — the atomic swap exists so `self.layout` never shows a
  half-updated frame, and `layout.main.browse_destination` gates stale-geometry
  mouse handling. → D3 is a module move with no reordering. The
  `tests_conformance_matrix.rs` and `shell_browser_tests.rs` suites exercise this
  and must pass unchanged.
- **[Trade-off] Three units means three PRs and three green-CI cycles for a change
  with no user-visible effect.** → Accepted. Unit 1 is mechanical and unit 2
  carries the behavioural risk; bundling them would put a risky deletion behind a
  large mechanical diff, which is how the original file-move produced this debt.
- **[Risk] `src/app/render/screens/root.rs` is 609 lines and the 800-line cap
  applies to whatever module absorbs `render_main`.** → Check
  `rtk make check-code-file-lines` before the final commit; split the shell draw
  module by concern if it lands over.

## Migration Plan

1. **Unit 1** — `arrangements/chrome.rs` extraction + identity-`Layout` deletion.
   Verify: `ast-grep scan` drops to ~39; `rtk cargo nextest run -p mbv` green.
2. **Unit 2, commit A** — queue-title characterization tests against current
   output, in all four remote states. Verify: they pass on unmodified code.
3. **Unit 2, commit B** — unconditional `title_area` derivation in `shell_queue.rs`;
   delete `screens/queue.rs`; delete the two dead `AppLayout` fields. Verify:
   commit A's tests unchanged and passing; `ast-grep scan` drops to ~19.
4. **Unit 3** — queue-panel block to a queue arrangement; backdrops to
   `components/chrome.rs`; `compose_base_frame`/`render_main` to the shell;
   `screens/root.rs` and `screens/mod.rs`'s `root`/`queue` entries deleted.
   Verify: `ast-grep scan` clean; full suite green; `rtk make check-code-file-lines`.
5. **Unit 3, final commit** — CI broadens to bare `ast-grep scan`; docs and the
   `mbv-frontend` skill mirrors updated; #635 closed.

Rollback is per-unit `git revert`; no data, protocol, or persisted-state surface
is touched.

## Open Questions

- Whether `compose_base_frame` keeps its name when it moves (#607 territory). It
  does not change this design or the task breakdown either way, so it is settled
  in whichever PR touches #607 rather than here.
