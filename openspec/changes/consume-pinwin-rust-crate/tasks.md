# Tasks

## 0. Gate (upstream — no mbv work until done)

- [x] 0.1 Confirm port tasks **4.1** (`panel` + `lib.rs` public API) and **6.3** (`cargo test`
  passes, examples build) are complete in `feat/port`, and record the exact upstream rev the
  cutover pins (design D2). If the public names differ from design D4–D7 (`Panel`, `Layout`,
  `Side`, `Keyboard`, `Startup`, `Accent`, `PinwinError`), update design D3 first.

## 1. Swap the dependency

- [x] 1.1 Delete the wrapper: `crates/mbv-pinwin/src/ffi.rs`, the Zig build in `build.rs`,
  `build.zig`, `build.zig.zon`, `zig-pkg/`; remove `mbv-pinwin` from `Cargo.toml` workspace
  members and add the `pinwin` crate dependency at the rev from 0.1 (design D1/D2). Verify
  `cargo check -p pinwin` resolves and builds.
- [x] 1.2 Remove the now-dead `crates/mbv-pinwin/` remainder (`panel.rs`, `layout.rs`,
  `error.rs`, `lib.rs`, `Cargo.toml`). `src/pin.rs` still references the wrapper until 2.1, so
  the workspace does not build yet; verification moves to the end of group 2.

## 2. Adapt `src/pin.rs` (owns the whole boundary, design D3)

- [x] 2.1 `PinnedPanel = pinwin::Panel` (lifetime gone; the `Box::leak` stays, comment reworded —
  `Panel`'s `Drop` never closes the fd). Start with
  `Panel::start(Startup { fd: master.as_raw_fd(), layout, keyboard: Keyboard::OnDemand,
  accent: None })`. Build `Layout` via `Layout::new(side, cols, top, bottom, left, right)` with
  `NonZeroU16::new(c).unwrap_or(NonZeroU16::MIN)` from `PanelConfig` widths (design D3). Define
  `const ANIM_DEFAULT_MS: u32 = 200` in `pin.rs` and use it in `apply_layout`. Map the
  reshaped `PinwinError` (`Unknown` arm deleted, `InvalidFd` into `PinStartError::Panel` with
  the existing one-line reporting). Verify `cargo check -p mbv`.
- [x] 2.2 Re-express the `pin.rs` tests on the new types: `layout_from_config` still owns the
  collapsed/expanded width-selection contract (cases unchanged in meaning, columns now
  `NonZeroU16`); the start-failure fallback test is untouched. If another test already covers
  the mapping, extend it instead of adding one. Verify `cargo nextest run -p mbv pin::`.
- [x] 2.3 Verify no `mbv-pinwin` / `mbv_pinwin` reference remains outside archived changes
  (`rg` exact-string check) and `cargo check --workspace` passes.

## 3. Call sites (moved paths only)

- [x] 3.1 Expected: no edits — `src/main.rs`, `src/app/state/app_struct.rs`,
  `src/app/dispatch/action.rs` and `src/app/dispatch/settings.rs` only name `crate::pin::*`.
  Fix only what the compiler flags. No logic change. Verify
  `cargo clippy --workspace --all-targets -- -D warnings`.

## 4. Docs, spec retirement and close-out

- [x] 4.1 Rewrite `AGENTS.md`'s `crates/mbv-pinwin/` repository-map entry and its
  `zig fetch --save=pinwin` pin procedure to the `pinwin` git-rev dependency (design D5);
  reword `README.md`'s "needs Zig" to Zig-for-ghostty via the pinwin crate; `PKGBUILD-git`
  unchanged. `CONTEXT.md` needs no term change. Verify `cargo fmt --all -- --check`.
- [x] 4.2 Delete `openspec/specs/pinwin-panel/` (design D6) and confirm
  `openspec validate --all` passes and nothing under `openspec/specs/` still names
  `pinwin_start` / `PINWIN_ERR_*`.
- [ ] 4.3 Integration check (manual, needs user approval for the live run): `mbv --pin` in a
  niri session — panel opens collapsed, `Ctrl+e` toggles with animation both ways, F2 Panel
  edits still apply live, stopping mbv removes the panel. Record the upstream rev verified
  against. `pinned-panel-focus-accent` rebases onto `Option<Accent>` afterwards (design D4).
