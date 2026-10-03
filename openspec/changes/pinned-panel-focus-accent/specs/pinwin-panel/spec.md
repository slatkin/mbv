## ADDED Requirements

### Requirement: Focus accent
`PinwinStartup` SHALL carry a `PinwinAccent { enabled, r, g, b, width }`. When `enabled` is 1, the
panel SHALL draw a `width`-pixel strip in colour `r,g,b` on its workspace-facing edge (the right
edge of a left-docked panel, the left edge of a right-docked one) while it holds keyboard focus,
including during a width tween, and SHALL remove the strip when it loses focus. When `enabled` is 0,
no strip is drawn. `pinwin_start` SHALL return `PINWIN_ERR_INVALID` and open nothing when `enabled`
is not 0 or 1, when `width` is outside 0 through 65535, or when `enabled` is 1 and `width` is 0. The
accent is fixed at `pinwin_start`; there is no runtime override.

#### Scenario: Strip while focused
- **WHEN** the host starts a left-docked panel with the accent enabled and the user clicks inside it
- **THEN** the strip is drawn on the panel's right edge until the user clicks a tiled window

#### Scenario: Right-docked panel during a tween
- **WHEN** a focused right-docked panel animates a width change
- **THEN** the strip stays on the panel's left edge throughout the tween

#### Scenario: Invalid accent
- **WHEN** the host calls `pinwin_start` with the accent enabled and width 0
- **THEN** the call returns `PINWIN_ERR_INVALID` and nothing opens
