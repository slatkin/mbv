# Tasks

## 1. Dispatch

- [x] 1.1 In `src/app/dispatch/action.rs`: branch `cycle_panel_mode` on `self.pinned_panel.is_some()`
  and add `pinned_view_toggle` per design D2/D3, reusing `toggle_pinned_width`'s apply/flash
  structure, assigning `panel_mode`, `mini_view_focus`, and `panel_focus` directly (never via
  `set_panel_focus`, whose width branch reads the stale pre-toggle `terminal_width`), and the
  mini-view `focus_queue_initial_item` path on collapse. Not-pinned is unreachable from the new
  fn (D1 guards the call site); no new `Command`, no key-policy or registry change.

## 2. Tests

Name the contract and layer for each; extend an existing test rather than adding a near-duplicate.
Follow `docs/architecture/tui-frontend.md`'s test-layer matrix and the existing pinned test homes
(`src/app/tests/`).

- [x] 2.1 Contract (App dispatch, pinned): with the panel pinned and collapsed, `x` applies the
  expanded width and shows library-only with library focus — assert stored `panel_focus`
  (not just the mini-view derivation) with `terminal_width` held at the pre-toggle collapsed
  width, so the test observes the real stale-width dispatch (design D2).
- [x] 2.2 Contract (App dispatch, pinned): with the panel pinned and expanded, `x` applies the
  collapsed width and shows queue-only with queue focus (mini view at a sub-80 collapsed width);
  assert stored `panel_focus` too, mirroring 2.1.
- [x] 2.3 Contract (App dispatch, pinned): when the expanded width is rejected by the panel, `x`
  flashes the warning toast, keeps the collapsed width, and leaves the panel mode unchanged.
- [x] 2.4 Contract (App dispatch, pinned): `Ctrl+e` changes only the width — the displayed panel
  mode survives the toggle in both directions.
- [x] 2.5 Confirm the existing unpinned `x` tests still pass unchanged (no new test needed; the
  branch must not alter non-pinned behaviour).

## 3. Docs and specs

- [ ] 3.1 Check the F1 help label and `README.md` wording for `panel_mode_cycle_x`/`x`; wherever
  the cycle is described unconditionally, note the pinned behaviour (or reference the pinned view
  toggle) instead of implying one universal `x` behaviour.
- [ ] 3.2 Sync the delta specs into `openspec/specs/` and archive the change when done.
