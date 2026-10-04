# Tasks

## 1. Window focus state

- [x] 1.1 Add `window_focused: bool` (default `true`) to `App` in `src/app/state/app_struct.rs` and `construct.rs`. Set it in `note_focus_gained` and `note_focus_lost` in `src/app/state/panel_focus.rs`. Verify `cargo check -p mbv`.
- [x] 1.2 Change `panel_appearance_focus` in the same file: in mini view return `self.pinned_panel.is_some() && self.window_focused && self.effective_panel_focus() == panel`; wide view unchanged. Update its doc comment. Confirm a focus event triggers a redraw (design D3). Verify `cargo check -p mbv`.
- [x] 1.3 Mirror the queue row-policy fix for the library panel (advisor gap): `crates/mbv-components/src/library_panel/panel/view.rs:212` passes `focused: self.focused && !self.mini_view` with no appearance projection. Add a `frame_focused`-style setter fed from the shell (`src/app/shell/library_panel.rs` currently pushes only `set_mini_view`) and compose it exactly as the queue does, only if library mini view is reachable (mini_view_focus == Library); if unreachable, report back and narrow the spec delta instead. Verify `cargo check -p mbv`.

## 2. Test

- [x] 2.1 Rewrite `mini_view_reports_no_panel_appearance_focus_while_wide_follows_focus` in `src/app/tests/panel_focus.rs`; it owns the appearance-focus contract. Cases: unpinned mini is never focused; pinned mini follows `window_focused` and the focused panel; wide ignores `window_focused`. Construct a pinned `App` without a live panel only if the stub allows it; otherwise extract the decision into a pure function over `(is_mini, pinned, window_focused, focus == panel)` and test that. Verify `cargo nextest run -p mbv`.
- [x] 2.2 Component test owning the library row contract (only if 1.3 lands as code): appearance-bit-focused mini library rows paint focused roles, resting mini rows do not. Verify `cargo nextest run -p mbv-components`.

## 3. Integration

- [ ] 3.1 Integration check (manual, user): `mbv --pin` on niri, panel narrower than 80 columns. Focused palette while the panel holds focus, resting palette after clicking a tile, focused again after clicking back. Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`; both must pass.
