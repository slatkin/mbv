# Proposal: add-configurable-keybinds

## Why

Every chord in mbv is hard-coded: the router-owned policy entries in `key_policy.rs`, and the leaf-local keys across ~25 components. Users cannot adapt the keyboard to their muscle memory, and there is no room to add commands without colliding with the saturated single-letter keyspace (playback transport letters, `1-9` tab jumps, `q`/`c`/`v`/`x`). Multiplexers solved this same saturation with a user-defined prefix namespace; mbv has neither the namespace nor any configuration surface.

The bindings are also *unorganized*: the policy entries carry no user-facing grouping, the help panel hand-writes its own key strings and labels, and no surface connects a binding to the settings domain it belongs to. Chords cannot be made configurable by adding one more hand-maintained list on top of that — the binding would then be written down in the policy matcher, the config parser, the config writer, the help text, and the settings UI.

## What Changes

- **One declared keybind action registry** (mbv-core, no UI dependencies) becomes the single source for every configurable binding: action id, owning settings section, default chords, scope, and gate. Config parsing/validation, policy matching, help, and the settings Keys surface all enumerate the same declaration, so they cannot drift.
  - The registry covers the router-owned globals **and** the router-owned transport actions (~31 actions), so the letters that actually collide become remappable.
  - Action ids reuse the policy entries' existing names (`settings_open`, `next_library_tab`, …). Load fails on an unknown id or a section mismatch.
  - `default_chords` is a list (aliases such as `+`/`=`, `<`/`>`); a configured action collapses to its one configured chord.
  - `alt_swallow` is excluded: it is a swallow guard with no `Command`, not an action.
- **Sectioned `[keys]` config section**, grouped by the same sections the settings UI already uses plus one `Global` section for chrome chords:
  - `prefix` — the prefix chord that arms prefix mode (sticky, tmux-style: no timer).
  - `[keys.<section>]` — router-scope overrides (`action = "chord"`).
  - `[keys.<section>.prefix]` — prefix-namespace assignments for the same actions.
  - Chord strings use one documented grammar (`"Ctrl+b"`, `"F8"`, `"Shift+Left"`, modifier-order-insensitive).
  - Reserved chords (`Ctrl+q`, extensible const), unknown action ids, unparseable chords, prefix/action collisions, and section mismatches are rejected at load with an error naming the offending entry.
- **Prefix mode in the central keyboard router** (ADR 0023 surface, no new routing site):
  - Arming is a top policy layer gated on `!text_entry_focused` and no blocking overlay; arming swallows the prefix chord.
  - While armed, the next chord resolves against the prefix assignments only — full namespace capture; mapped chord → `Command` → disarm; Esc or unmapped chord → disarm + Swallow; repeated prefix → re-arm. Any mouse event disarms silently.
  - Armed state is App-owned plain data mirrored into `RouterSnapshot`.
- **The settings UI gains a `Keys` destination** (F2 panel, read-only) that lists exactly the configurable set, grouped by the shared section list, showing each action's router chord and prefix chord. The settings main list carries a `Keys` row whose value summarizes the live configuration (prefix and override count). Chords change in the config file in this change; in-place capture is a follow-up.
- **Help renders from the registry**: the Playback section's key strings and the Global section's configurable chords come from the declaration instead of hand-written literals, so help matches behavior after any valid configuration load.
- **Double-tap deferral is removed** (prerequisite for remappable transport): `Space` toggles play/pause and `Esc` stops on a single press when the focused component does not claim the chord. The leaf-first `Deferred` outcome survives unchanged; only its 300 ms candidate timing state is deleted.
- Out of scope: leaf-local key rebinding (~470 `Key::` matches across components) — the prefix namespace is the answer to keyspace saturation, matching the multiplexer model; leaf-local actions are not `Command`s and have no action identity; new `Command` variants; in-place key capture in the settings UI; multi-chord (sequence) bindings; per-destination binding sets.

## Capabilities

### New Capabilities

- `configurable-keybinds`: the single keybind action declaration; the `[keys]` config section (prefix, sectioned router-scope overrides, sectioned prefix-namespace assignments); the shared section vocabulary (settings sections plus `Global`); reserved/unknown/collision validation at load; sticky prefix mode semantics (arming gates, namespace capture, disarm rules); the rebindable action set; preserved router guarantees under configured bindings; the settings Keys destination; the status-bar armed indicator; and help exposure of current bindings.

### Modified Capabilities

- `semantic-input-arbitration`: context-sensitive global actions stay deferred candidates resolved after leaf arbitration, but the double-tap candidate timing state is removed — an unhandled `Space`/`Esc` candidate fires on the press, and a consumed press leaves no inherited candidate state.

## Impact

- `crates/mbv-core` — new keybind registry and `Keybinds` config type (source-of-truth types before callers): action declaration, section vocabulary, chord-string parser, defaults, reserved-chord/collision/unknown-id validation; `Config` wiring and save round-trip through the existing read-patch-write TOML path.
- `src/app/key_policy.rs` — matching and defaults consult the registry; the `Playback` bucket splits into per-action entries; policy remains a pure function of plain-data inputs (`Keybinds`/registry passed in, no global state — ADR 0023 preserved).
- `src/app/router.rs` — armed handling in outcome selection; arming gates.
- `src/app/shell*.rs` — App-owned armed flag (arm/disarm transitions, mouse disarm), config load plumbing; double-tap candidate clocks and `apply_deferred_candidate`'s timing branch removed.
- `src/app/components/library_playback_panel.rs` — leaf-space/Esc double-tap state removed.
- `src/app/types_settings.rs`, `src/app/shell_settings.rs`, `src/app/settings.rs` — `SettingKey::Keys` section row and the Keys destination content.
- `src/app/render/components/settings*.rs` + `render/components/chrome_status.rs` — Keys destination painting, armed-status pill.
- `src/app/action.rs` — `PLAYBACK_HELP_BINDINGS` (the hand-written table this change deletes) lives here, not in `help.rs`; `action_tests.rs`'s existing characterization test `playback_help_bindings_match_playback_command_for_key` needs rewriting alongside it.
- `src/app/components/help.rs` + `render/components/help.rs` — bindings rendered from the registry.
- Tests — routing-matrix fixtures construct registry defaults; new tick-integration coverage for arm/disarm namespace capture through `Application::tick()`; the double-tap characterization families in `tests_tick_integration.rs`/`shell_tests.rs` are rewritten as behavior-change records.
- Dependencies: none added (no `crossterm-keybind` — its flat enum plus process-global initialization conflicts with the ordered, gated, pure policy).
