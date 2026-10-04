# Proposal

## Why

pinwin is being ported from Zig/C to a single Rust crate (`feat/port` in the pinwin
workout; package `pinwin`). The port **removes the C ABI with no replacement C surface**:
`pinwin_api.h`, `pinwin_start` / `pinwin_apply_layout` / `pinwin_apply_layout_animated` /
`pinwin_stop`, `PinwinStartup` and `PINWIN_ERR_*` all go away, replaced by a Rust `Panel`
handle and `PinwinError` (port `port-to-rust`, task 4.1).

mbv consumes pinwin today through `crates/mbv-pinwin`, a hand-kept mirror of that C ABI
(`ffi.rs` repr(C) structs, `build.rs` Zig build, `build.zig`/`build.zig.zon` pin, vendored
`zig-pkg/`). Once the port lands, there is nothing left for that wrapper to mirror: the
mirror structs, the size assumptions, the Zig build and the vendored tree all become dead
weight that can only drift from the real interface. This change cuts mbv over to the Rust
crate directly.

## What Changes

- Delete the C-ABI wrapper: `crates/mbv-pinwin/src/ffi.rs`, the `zig build` in `build.rs`,
  `build.zig`, `build.zig.zon` (pin `c64af49`), and `zig-pkg/`; remove `mbv-pinwin` from the
  workspace and depend on the `pinwin` Rust crate instead (D2 pins how).
- Adapt the only wrapper-touching file, `src/pin.rs`, plus its call sites (`src/main.rs`,
  `src/app/state/app_struct.rs`, `src/app/dispatch/action.rs`, `src/app/dispatch/settings.rs`)
  to the new types (D3): lifetime-free `Panel`, `NonZeroU16` columns, `Option<Accent>`,
  reshaped `PinwinError` (`Unknown` gone, `InvalidFd` new).
- No behaviour change: the panel starts, resizes (animated), and stops exactly as today; the
  accent stays off until `pinned-panel-focus-accent` lands on top (D4).

## Capabilities

(none — pure dependency swap; no new or modified user-visible capabilities, so this change
carries no spec delta.)

## Impact

- Crates and files that change:
  - `crates/mbv-pinwin`: deleted as a wrapper (or reduced to nothing — D1).
  - `Cargo.toml` / `Cargo.lock`: workspace membership, `pinwin` dependency + pin.
  - `src/pin.rs`: `PinnedPanel` type, `Layout`/`Keyboard` construction, error mapping, tests.
  - `src/main.rs`, `src/app/state/app_struct.rs`, `src/app/dispatch/action.rs`,
    `src/app/dispatch/settings.rs`: moved paths/constants only.
  - Build/packaging notes if they name Zig for the panel (D5).
- No protocol, daemon, config, or theme changes.
- Blocked on upstream: port tasks **4.1** (`panel` + `lib.rs` public API) and **6.3**
  (`cargo test` passes). Implement only after both are done.
- Sequencing with `pinned-panel-focus-accent` (D4): that change targets `Some(Accent)` on the
  new API; either it rebases onto this change or this change lands first — decided in D4.
