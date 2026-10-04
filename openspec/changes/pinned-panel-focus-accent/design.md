# Design

## Context

pinwin `c64af49` (the pin landed by `panel-expand-toggle`) ships the focus accent:
`PinwinAccent { int32_t enabled; uint8_t r, g, b; int32_t width; }` as the last field of
`PinwinStartup`, which grew the struct from 32 to 44 bytes.
- `startup_valid` returns `PINWIN_ERR_INVALID` when `enabled` is not 0 or 1, when `width` is
  outside 0..=65535, or when `enabled` is 1 and `width` is 0.
- The strip is drawn on the workspace-facing edge in raw surface coordinates, and it stays
  correct during a width tween.
- Focus comes from the drawing area's `GtkEventControllerFocus`.
- There is no runtime API to change the accent; it is fixed at `pinwin_start`.

In mbv today:
- `crates/mbv-pinwin/src/ffi.rs` already mirrors `PinwinAccent` and the `accent` field of
  `PinwinStartup` (`panel-expand-toggle`), and `Panel::start` fills it with `enabled: 0`.
- `Panel::start(master, layout, keyboard_mode)` has no accent parameter.
- `src/pin.rs::start_panel` calls `Panel::start` with a layout built from `PanelConfig`.

## Goals / Non-Goals

**Goals:**
- The pinned panel shows the pinwin focus accent by default, and the user can configure it.
- The FFI mirror matches the pinned header exactly.

**Non-Goals:**
- Changing the accent while the panel runs (pinwin has no API for it).
- A free-text colour editor in F2.
- Making the accent colour follow the mbv theme.
- Any pinwin change.

## Decisions

**D1: Typed accent at the crate boundary.** `mbv_pinwin::Accent { color: Rgb, width: NonZeroU16 }`
with `Rgb { r: u8, g: u8, b: u8 }`. `Panel::start(master, layout, keyboard_mode, accent:
Option<Accent>)`. `to_abi` maps `None` to `enabled: 0, width: 1`, and `Some` to `enabled: 1` with
the width widened to `i32`. Every state pinwin would reject is unrepresentable on the Rust side:
`enabled` is always 0 or 1, and an enabled accent never has width 0.

**D2: Mirror layout.** `#[repr(C)] struct PinwinAccent { enabled: i32, r: u8, g: u8, b: u8,
width: i32 }`, appended to `PinwinStartup` as `accent`, which matches the C field order. No
`Pod`/`bytemuck` derive, so the padding byte after `b` is harmless. The mirror and the field
landed with `panel-expand-toggle`; what remains is a `const _:` size assertion that
`size_of::<PinwinStartup>() == 44`, so the next pinwin bump that changes the struct fails at
compile time instead of becoming UB.

**D3: Config shape.** `PanelConfig` gains:
- `accent: bool`, default `true`.
- `accent_color: PanelAccentColor`, a newtype over `[u8; 3]` with `parse("#RRGGBB" | "RRGGBB")`
  and `Display` as `#rrggbb`, default `#dabc7f`.
- `accent_width: NonZeroU16`, default 1.

Parse and save follow the existing per-key fallback with a logged warning. `src/pin.rs` builds
`Option<Accent>` from these three fields at start.

**D4: F2 rows, effective at the next launch.** The F2 Panel page gets three rows after the
gutters:
- `Accent`: Boolean.
- `Accent width`: Stepper, 1..=65535, ±1 or ±10 with Shift.
- `Accent color`: Text, cycling a fixed list of colours plus the configured value.

The cycle list is `#dabc7f` (the default), `#7fc8da`, `#a3be8c`, `#bf616a`, `#ffffff`. When the
configured colour is not in the list, it appears first so it is never lost.

`apply_panel_setting` does not call `pin::apply_layout` for these keys, since the accent is not
part of the layout. It saves the value, and while pinned it shows a `Neutral` toast: "Accent
changes apply on the next mbv --pin launch". Unpinned, it saves without a toast, like other Panel
rows.

## Risks / Trade-offs

- [ABI drift on a future pin bump] → mitigated by the D2 size assertion.
- [Colour cycling is limited] → any hex value can still be set in `config.toml`, and the cycle
  keeps that value.
- [Ordering against `panel-expand-toggle`] → its pin `c64af49` already carries the accent, so this
  change bumps nothing; implement it only after that change merges.
