# Tasks: add-configurable-keybinds

## 1. Keys configuration in mbv-core

- [ ] 1.1 Add `Keybinds` type (prefix chord, prefix map chord→command name, rebind map action→chord), chord-string parser (`"Ctrl+b"`, `"F8"`, `"Shift+Left"`, modifier-order-insensitive), and `Keybinds::defaults()` compiled from today's policy chords. Verify: unit tests parse canonical and order-insensitive forms; defaults equal the current hard-coded chords.
- [ ] 1.2 Wire `[keys]` into `Config` (serde) and the existing read-patch-write save path. Verify: round-trip test — a config with unrelated sections plus `[keys]` survives save with both preserved.
- [ ] 1.3 Load-time validation: reserved chords (`Ctrl+q` extensible const), prefix/rebind collisions (including prefix vs any default global chord), unknown command names, unparseable chord strings — each rejects with a named error. Verify: table-driven unit tests covering every rejection class plus the accept case.

## 2. Rebindable global chords (Tier 2)

- [ ] 2.1 Thread `&Keybinds` through `resolve_policy`/`command_for_policy`; rebindable `global: true` entries match configured chords, defaults unchanged. Verify: key_policy unit tests — rebound action fires on its new chord, its default chord is inert, and `Keybinds::defaults()` reproduces today's resolution exactly.
- [ ] 2.2 Update routing-matrix fixtures to construct `Keybinds::defaults()`; add matrix rows proving the text-entry and blocking-overlay rules hold for rebound chords. Verify: `cargo nextest run -p mbv` routing matrix green.
- [ ] 2.3 Shell passes the parsed `Keybinds` from config load into routing. Verify: tick-integration test — a configured rebind fires through `Application::tick()`.

## 3. Prefix mode

- [ ] 3.1 App-owned armed flag mirrored as `RouterSnapshot.prefix_armed`; shell arm/disarm transitions from router outcomes; silent mouse disarm in the mouse path. Verify: tick-integration — arming consumes the prefix chord, mouse disarms without altering the event's handling.
- [ ] 3.2 Policy layers: `prefix_arm` top layer gated on `!text_entry_focused` + `!blocking_overlay_open`, and the armed-dispatch layer (mapped → Command + disarm; Esc/unmapped → Swallow + disarm; prefix → re-arm; no FallThrough while armed). Verify: key_policy unit tests for every state-machine arm, including F-keys captured while armed and arming suppressed under text entry/overlays.
- [ ] 3.3 Full state machine through `Application::tick()`: mapped command executes, unmapped chord swallowed and disarmed, double-prefix re-arms, no chord reaches any component while armed (component counters). Verify: new tick-integration test file green.

## 4. Indicator and help exposure

- [ ] 4.1 `prefix_status_spans()` armed pill in the `chrome_status` idiom, rendered only while armed, following the existing width-pressure drop-order precedent. Verify: buffer test — pill present while armed, absent otherwise; existing status-bar tests still green.
- [ ] 4.2 Help bindings section rendered from `Keybinds` (defaults merged with overrides) plus the prefix map. Verify: help test — after a rebind, help shows the configured chord; with defaults, help matches current behavior.

## 5. Gates

- [ ] 5.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, `cargo nextest run -p mbv -p mbv-core`. Verify: all green.
- [ ] 5.2 `openspec validate add-configurable-keybinds --strict`. Verify: passes.
