# Design

## Context

`src/pin.rs` builds a `mbv_pinwin::Layout` from `PanelConfig` (`layout_from_config`) at start and
in `apply_layout`, which `App::apply_panel_setting` calls to validate an F2 edit before saving it.
pinwin's `apply_layout` already resizes the live panel, its reservation and the pty winsize in
place, so a width toggle is two `apply_layout` calls with different `cols`. pinwin stays stateless
about layout; the host owns both widths and which one is active.

## Goals / Non-Goals

**Goals:**
- One rebindable key flips the pinned panel between two saved widths, retiling.
- F2 edits stay validated by the live panel and never snap the panel to the wrong width.

**Non-Goals:**
- CLI flag / WM global key, IPC, pidfile (possible follow-up; one `--pin` process per user makes a
  pidfile + signal enough then).
- Overlay expand (reservation stays narrow), animation, in-panel click affordance, pinwin changes.
- Persisting the active width across launches.

## Decisions

**D1 — Active width is a typed runtime value on `App`.** `enum PinnedWidth { Collapsed, Expanded }`
(in `src/pin.rs`), held next to `pinned_panel`, default `Collapsed`. Not a `bool`, not in
`PanelConfig` (config holds saved values only; the active width is never saved).

**D2 — Layout = config + width.** `layout_from_config(config, width)` picks `config.cols` or
`config.cols_expanded`; `pin::start` passes `Collapsed`, `pin::apply_layout` takes the width.
`PanelConfig` gains `cols_expanded: u16` with `DEFAULT_PANEL_COLS_EXPANDED = 80`; parse and save
mirror `cols` exactly (same range, same fallback warning).

**D3 — Toggle path.** Registry entry `pinned_width_toggle` (Global, `Ctrl+e`,
`KeyGate::NoBlockingOverlay`, rebindable, prefix-addressable) → `KEY_POLICY` entry of the same
name → `KeyPolicyBinding::PinnedWidthToggle` → `Command::TogglePinnedWidth` → `App` handler:
no panel → `Neutral` toast; else apply the other width; `Ok` → store it; `Err` → `Warning` toast,
width unchanged. `Ctrl+e` is not a registry default or `KEY_POLICY` literal today; letter `e` is
leaf-local in feeds components, which is why a Ctrl chord is used.

**D4 — F2 edits pick the width they validate.** `apply_panel_setting` computes the width to apply:
`PanelCols` → `Collapsed`, `PanelColsExpanded` → `Expanded`, anything else → current. On `Ok` it
stores that width alongside the saved config, so the panel shows the value just edited.
Alternative rejected: applying at the current width always — editing the inactive width would then
be saved without pinwin ever validating it.

**D5 — Naming.** CONTEXT.md gains **Pinned panel width** (collapsed / expanded). Avoid "panel mode"
(reserved for the in-TUI Panel mode) and "expand mode".

## Risks / Trade-offs

- Every toggle makes the compositor reflow all tiled columns. Accepted: retile was chosen over
  overlay.
- Expanding can cross mbv's own Panel mode thresholds (e.g. Narrow → Wide) via the normal resize
  path. Intended — that is most of the value of a wide panel.
- `cols_expanded` may be smaller than `cols`; nothing forbids it, the key still just swaps them.
