# Tasks

## 0. Gate (upstream — no mbv work until done)

- [ ] 0.1 Confirm port tasks **4.1** (`panel` + `lib.rs` public API) and **6.3** (`cargo test`
  passes, examples build) are complete in `feat/port`, and record the exact upstream rev the
  cutover pins (design D2). If the public names differ from design D4–D7 (`Panel`, `Layout`,
  `Side`, `Keyboard`, `Accent`, `PinwinError`, `ANIM_DEFAULT_MS`), update design D3 first.

## 1. Swap the dependency

- [ ] 1.1 Delete the wrapper: `crates/mbv-pinwin/src/ffi.rs`, the Zig build in `build.rs`,
  `build.zig`, `build.zig.zon`, `zig-pkg/`; remove `mbv-pinwin` from `Cargo.toml` workspace
  members and add the `pinwin` crate dependency at the rev from 0.1 (design D1/D2). Verify
  `cargo check -p pinwin` resolves and builds.
- [ ] 1.2 Remove the now-dead `crates/mbv-pinwin/` remainder (`panel.rs`, `layout.rs`,
  `error.rs`, `lib.rs`, `Cargo.toml`). Verify no `mbv-pinwin` / `mbv_pinwin` reference remains
  (`rg` exact-string check) and `cargo check --workspace` passes.

## 2. Adapt `src/pin.rs` (owns the whole boundary, design D3)

- [ ] 2.1 `PinnedPanel = pinwin::Panel` (lifetime gone; the `Box::leak` stays with a comment —
  the lib still never closes the fd). Build `Layout` through the new constructor with
  `NonZeroU16` from `PanelConfig` widths; pass `Keyboard::OnDemand` and accent `None`;
  map the reshaped `PinwinError` (`Unknown` arm deleted, `InvalidFd` into `PinStartError::Panel`
  with the existing one-line reporting). Fix the moved `ANIM_DEFAULT_MS` path in
  `apply_layout`. Verify `cargo check -p mbv`.
- [ ] 2.2 Re-express the `pin.rs` tests on the new types: `layout_from_config` still owns the
  collapsed/expanded width-selection contract (cases unchanged in meaning, columns now
  `NonZeroU16`); the start-failure fallback test is untouched. If another test already covers
  the mapping, extend it instead of adding one. Verify `cargo nextest run -p mbv pin::`.

## 3. Call sites (moved paths only)

- [ ] 3.1 Fix every exhaustive match and import the compiler flags in `src/main.rs`,
  `src/app/state/app_struct.rs`, `src/app/dispatch/action.rs`,
  `src/app/dispatch/settings.rs`. No logic change. Verify
  `cargo clippy --workspace --all-targets -- -D warnings`.

## 4. Docs and close-out

- [ ] 4.1 Update any doc that names the Zig panel build (per D5; if none names it, record that
  here and change nothing). `CONTEXT.md` needs no term change (the **Pinned panel** concept is
  unchanged). Verify `cargo fmt --all -- --check`.
- [ ] 4.2 Integration check (manual, needs user approval for the live run): `mbv --pin` in a
  niri session — panel opens collapsed, `Ctrl+e` toggles with animation both ways, F2 Panel
  edits still apply live, stopping mbv removes the panel. Record the upstream rev verified
  against. `pinned-panel-focus-accent` rebases onto `Option<Accent>` afterwards (design D4).
