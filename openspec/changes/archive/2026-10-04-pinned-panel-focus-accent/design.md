# Design

## Context

The `pinwin` crate at pin `0d9be0b` ships the focus accent:
`pinwin::layout::Accent::new(rgb: [u8; 3], width: NonZeroU16)` and `Startup.accent:
Option<Accent>`, where `None` is "disabled".
- The accent is a stroke around the whole panel window, inset by half its width, drawn only while
  the panel holds keyboard focus.
- Focus comes from the drawing area's focus controller.
- There is no runtime API to change the accent; it is fixed at `Panel::start`.

In mbv today, `src/pin.rs::start_panel` builds `Startup { .., accent: None }` with a layout from
`PanelConfig`.

## Goals / Non-Goals

**Goals:**
- The pinned panel shows the pinwin focus accent by default, and the user can configure it.

**Non-Goals:**
- Changing the accent while the panel runs (pinwin has no API for it).
- A free-text colour editor in F2.
- Making the accent colour follow the mbv theme.
- Any pinwin change.

## Decisions

**D1: Use pinwin's `Accent` directly.** It already makes invalid states unrepresentable (non-zero
width, "disabled" is `None`), so mbv adds no accent type of its own at the pinwin boundary.

**D2: Config shape.** `PanelConfig` gains:
- `accent: bool`, default `true`.
- `accent_color: PanelAccentColor`, a newtype over `[u8; 3]` with `parse("#RRGGBB" | "RRGGBB")`
  and `Display` as `#rrggbb`, default `#dabc7f`.
- `accent_width: NonZeroU16`, default 1.

Parse and save follow the existing per-key fallback with a logged warning. `src/pin.rs` builds
`Option<Accent>` from these three fields at start: `accent.then(|| Accent::new(color.0, width))`.

**D3: F2 rows, effective at the next launch.** The F2 Panel page gets three rows after the
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

- [Colour cycling is limited] → any hex value can still be set in `config.toml`, and the cycle
  keeps that value.
