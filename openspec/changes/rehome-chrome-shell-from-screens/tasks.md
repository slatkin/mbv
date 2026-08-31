> **Path disambiguation — read before editing.** This change touches files whose
> basenames repeat. Always use the full path:
>
> | Path | Role in this change |
> |---|---|
> | `src/app/render/screens/root.rs` | legacy chrome shell — **source**, emptied by unit 3 |
> | `src/app/render/screens/queue.rs` | legacy queue-title underpaint — **deleted** in unit 2 |
> | `src/app/render/components/queue.rs` | migrated queue painter (`queue_title_model`, `render_queue_title_content`) — **do not delete** |
> | `src/app/components/queue.rs` | `QueueComponent` (TuiRealm) — **do not delete** |
> | `src/app/components/root.rs` | TuiRealm root component — **not touched** |
> | `src/app/render/components/chrome.rs` | destination for the column backdrops |
> | `src/app/render/arrangements/chrome.rs` | **new** in unit 1 |
>
> **Tool notes.** Run `ast-grep scan` from the repo root with no path argument —
> a path argument silences the `frontend-boundary` rules and makes every check
> below pass falsely. Prefix cargo/make with `rtk`. Run `rtk cargo fmt` after
> every task and accept all reflow.

## 1. Unit 1 — chrome geometry to an arrangement

- [ ] 1.1 Record the starting count: `ast-grep scan --json | jq 'length'` reports
      **52**. If it does not, stop and report — the plan is sized against 52.
- [ ] 1.2 Create `src/app/render/arrangements/chrome.rs` with a
      `pub(in crate::app) struct ChromeGeometryInput` carrying the values
      `App::compute_chrome_geometry` (`src/app/render/screens/root.rs:75-182`)
      reads from `self`: effective panel mode, terminal width/height, queue column
      width, and panel focus. Verify `rtk cargo check -p mbv` compiles with the
      struct unused.
- [ ] 1.3 Move the body of `compute_chrome_geometry` into
      `pub(in crate::app) fn chrome_geometry(input: ChromeGeometryInput) -> FrameChromeGeometry`
      in that file — a free function, **not** an `impl App` block (design D1).
      Register the module in `src/app/render/arrangements/mod.rs`. Verify
      `rtk cargo check -p mbv` passes.
- [ ] 1.4 Replace `compute_chrome_geometry` in
      `src/app/render/screens/root.rs` with a thin `&self` shim that builds
      `ChromeGeometryInput` and calls `chrome_geometry`. Keep its existing
      `pub(in crate::app::render)` visibility so `src/app/render/test_helpers.rs:298`
      still resolves. Verify `rtk cargo nextest run -p mbv` is green.
- [ ] 1.5 Delete the identity split at `src/app/render/screens/root.rs:232`
      (`let [main_area] = Layout::vertical([Constraint::Min(0)]).areas(area);`) and
      use `area` directly at its one use site. Verify `rtk cargo nextest run -p mbv`
      is green and `ast-grep scan --json | jq '[.[]|select(.ruleId=="no-layout-in-screens")]|length'`
      reports **0**.
- [ ] 1.6 Drop any `ratatui` imports in `src/app/render/screens/root.rs` that 1.3
      and 1.5 orphaned. Verify `rtk cargo clippy --workspace --all-targets` is warning-free
      and `ast-grep scan --json | jq 'length'` reports **≈39** (down from 52).

## 2. Unit 2, commit A — characterization tests for the queue title

These pin current output *before* the painter flips. They must pass against
unmodified code; if one fails, the test is wrong, not the code.

- [ ] 2.1 Add `TestBackend` tests capturing the queue title row's current rendered
      output with **no remote session** (`RemoteSlotState::Off`), where
      `src/app/render/screens/queue.rs` is the sole painter and `QueueComponent`
      paints no title. Cover nerd-fonts on and off. Verify they pass on the
      current tree with no source changes.
- [ ] 2.2 Add the same coverage for `RemoteSlotState::DirectRemote`,
      `AttachedSession` with an mbv client, and `AttachedSession` with a non-mbv
      client — the three states where `show_split` is true and both painters run.
      Verify they pass on the current tree with no source changes.
- [ ] 2.3 Commit 2.1 and 2.2 on their own, with no production change in the
      commit. Verify `git show --stat` lists only test files.

## 3. Unit 2, commit B — delete the legacy queue-title underpaint

- [ ] 3.1 In `src/app/shell_queue.rs:50`, derive `title_area` from
      `self.app.layout.main.queue_area` unconditionally, dropping the
      `(…queue_scope_local_area.height > 0).then(…)` gate. Keep the rect
      arithmetic identical (`x + 2`, `y - 2`, `width - 4`, `height 1`). Apply the
      same change to the second gate at `src/app/shell_queue.rs:78`. Verify
      `rtk cargo check -p mbv` passes.
- [ ] 3.2 Delete `src/app/render/screens/queue.rs` and its `queue` entry in
      `src/app/render/screens/mod.rs`, and remove the
      `self.render_queue_title(...)` call at `src/app/render/screens/root.rs:492`
      along with the now-unused `title_overhead` computation feeding only it.
      Do **not** touch `src/app/render/components/queue.rs` or
      `src/app/components/queue.rs`. Verify `rtk cargo check -p mbv` passes.
- [ ] 3.3 Run the unit-2A tests. If any fail, the two painters diverge: stop,
      report the exact diff in rendered output and which state produced it, and
      get a decision on which rendering is correct before changing either the
      test or the painter (design D2, first Risk). Verify the suite's outcome is
      recorded either way.
- [ ] 3.4 Delete `queue_scope_local_area` and `queue_scope_remote_area` from
      `LayoutMain` in `src/app/layout.rs:113-114` and fix any test-only readers to
      use `QueueComponent::test_scope_pill_areas` instead. Verify
      `rtk cargo nextest run -p mbv` is green.
- [ ] 3.5 Verify `ast-grep scan --json | jq 'length'` reports **≈19** and
      `rtk cargo clippy --workspace --all-targets` is warning-free.

## 4. Unit 3 — queue panel arrangement, backdrops, and the shell move

Order is load-bearing: 4.1-4.2 must precede 4.4 (design D3).

- [ ] 4.1 Move the queue-panel geometry from `render_main`
      (`src/app/render/screens/root.rs:479-546`: `render_queue_panel_frame` call,
      content-area carve, status-pill row placement) into a queue panel
      arrangement function alongside `src/app/render/arrangements/chrome.rs`,
      returning typed rects. Verify `rtk cargo nextest run -p mbv` is green with
      output unchanged.
- [ ] 4.2 Move the playlist/autosave status pill *painting* from that same block
      into `src/app/render/components/queue.rs` as a content-model painter taking
      the rect from 4.1. Verify `rtk cargo nextest run -p mbv` is green.
- [ ] 4.3 Move `paint_legacy_chrome`'s two backdrop `render_widget` calls
      (`src/app/render/screens/root.rs:590,598`) into
      `src/app/render/components/chrome.rs`. Verify `rtk cargo nextest run -p mbv`
      is green.
- [ ] 4.4 Move `compose_base_frame`, `render_main`, and `compute_frame_layout` out
      of `src/app/render/screens/root.rs` into a shell-side module next to
      `src/app/shell_run.rs`, preserving the `AppLayout` atomic-swap ordering and
      the `layout.main.browse_destination` tag exactly. Keep the public name
      `compose_base_frame` (design Open Questions). Verify all ~20 existing call
      sites compile and `rtk cargo nextest run -p mbv` is green — in particular
      `tests_conformance_matrix.rs` and `shell_browser_tests.rs`.
- [ ] 4.5 Delete `src/app/render/screens/root.rs` and its `root` entry in
      `src/app/render/screens/mod.rs`; prune orphaned re-exports in
      `src/app/render/mod.rs`. Verify `rtk cargo clippy --workspace --all-targets`
      is warning-free.
- [ ] 4.6 Verify `ast-grep scan` (no path argument) exits 0 with **no findings**.
      Do not run or gate on `rtk make check-code-file-lines` here — the 800-line
      cap is checked once, pre-PR, at 5.4.

## 5. Close the ratchet

- [ ] 5.1 In `.github/workflows/architecture-boundaries.yml`, replace
      `ast-grep scan src/app/components/` with bare `ast-grep scan` and delete the
      comment block explaining the scoped path. Verify the job's script is a bare
      scan and the `0.44.1` pin is untouched.
- [ ] 5.2 Update `docs/architecture/interactive-surface-ledger.md` where it
      describes the frontend-boundary checks as a ratchet on touched code, and
      note that the chrome shell was rehomed rather than migrated. Verify the file
      no longer claims a standing violation baseline.
- [ ] 5.3 Update `.opencode/skills/mbv-frontend/SKILL.md` and its `.agents/` and
      `.codex/` mirrors to state that the checks gate the whole tree. Verify all
      three copies match.
- [ ] 5.4 Run the full pre-PR gate: `ast-grep scan`,
      `rtk cargo clippy --workspace --all-targets`,
      `rtk cargo nextest run -p mbv`, `rtk cargo fmt --check`, and
      `rtk make check-code-file-lines`. This is the only point the 800-line cap
      is enforced; split the shell draw module by concern only if it fails here.
      Verify all five pass.
- [ ] 5.5 Close issue #635 referencing this change. Verify the issue is closed.
