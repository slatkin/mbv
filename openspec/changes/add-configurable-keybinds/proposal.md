# Proposal: add-configurable-keybinds

## Why

Every chord in mbv is hard-coded: the router-owned globals in `key_policy.rs`, and the leaf-local keys across ~25 components. Users cannot adapt the keyboard to their muscle memory, and there is no room to add new global commands without colliding with the saturated single-letter keyspace (playback letters, `1-9` tab jumps, `q`/`c`/`v`/`x`). Multiplexers solved this same saturation with a user-defined prefix namespace; mbv has neither the namespace nor any configuration surface.

## What Changes

- New `[keys]` section in the user config file (mbv-core `Config`):
  - `prefix` — the prefix chord that arms prefix mode (sticky, tmux-style: no timer).
  - `[keys.bind]` — prefix-namespace assignments mapping chords to existing `Command` variants.
  - `[keys.rebind]` — overrides for the router-owned **global** chords (the `global: true` `KEY_POLICY` entries: F1–F4, `q`, Tab/BackTab, `x`, `c`, Ctrl+L, F5, Alt+arrows, Alt-swallow, `1-9` tab jump).
- Prefix mode in the central keyboard router (ADR 0023 surface, no new routing site):
  - Arming is a top policy layer gated on `!text_entry_focused` and no blocking overlay; arming Swallows the prefix chord.
  - While armed, the next chord resolves against `[keys.bind]` only — full namespace capture, nothing falls through to leaf components. Mapped chord → `Command` → disarm; Esc or unmapped chord → disarm + Swallow; repeated prefix → re-arm.
  - Any mouse event disarms silently (mouse delivery unchanged).
  - Armed state is App-owned plain data mirrored into `RouterSnapshot` like `space_double_tap`/`esc_double_tap`.
- Reserved chords: `Ctrl+q` is rejected at config load (reserved for future hard binding; the reserved list is a single extensible const). Collisions between `prefix` and a rebound global are validated and rejected at load.
- Exposability: the help sidebar renders current bindings (defaults + overrides) from the same struct the router resolves; the status bar shows a prefix-armed pill in the existing `chrome_status` span-pill idiom, visible only while armed.
- Out of scope: leaf-local key rebinding (~470 `Key::` matches across components) — the prefix namespace is the answer to keyspace saturation, matching the multiplexer model; playback letters, Esc behavior, visualizer, Shift+arrows, and the selection-modal catch-all stay hard-coded; new `Command` variants beyond today's ~26; timed prefix expiry; multi-chord sequences.

## Capabilities

### New Capabilities

- `configurable-keybinds`: user-configurable keyboard bindings — the `[keys]` config section (prefix, prefix-namespace assignments, global rebinds), reserved-chord and collision validation at load, sticky prefix mode semantics (arming gates, namespace capture, disarm rules), preserved router guarantees under rebinds (text-entry protection, blocking overlays, ordered precedence), the status-bar armed indicator, and help-sidebar exposure of current bindings.

### Modified Capabilities

<!-- None: no existing spec pins concrete chords or status-bar contents. -->

## Impact

- `crates/mbv-core` — new `Keybinds` config type (source-of-truth types before callers): prefix chord, prefix map, global rebind map; chord-string parsing (`"Ctrl+b"`, `"F8"`, `"Shift+Left"`); reserved/collision validation; `Config` wiring and save round-trip via the existing TOML section-patch path.
- `src/app/key_policy.rs` — policy entries consult configured chords for the rebindable globals; two new prefix layers (arm, armed-dispatch); policy remains a pure function of plain-data inputs (`Keybinds` passed in alongside `RouterSnapshot`, no global state — ADR 0023 preserved).
- `src/app/router.rs` — armed handling in outcome selection; arming gates.
- `src/app/shell*.rs` — App-owned armed flag (arm/disarm transitions, mouse disarm), config load plumbing.
- `src/app/render/components/chrome_status.rs` — armed-status pill following the existing `*_status_spans` pattern.
- `src/app/components/help.rs` + `render/` help panel — bindings section rendered from the live `Keybinds`.
- Tests — routing-matrix fixtures gain `Keybinds` inputs; new tick-integration coverage for arm/disarm namespace capture through `Application::tick()`.
- Dependencies: none added (config parsing reuses serde/toml already in the tree; no crossterm-keybind — its flat enum + process-global `init_and_load` model conflicts with the ordered, gated, pure policy).
