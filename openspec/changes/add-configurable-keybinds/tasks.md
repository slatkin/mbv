# Tasks: add-configurable-keybinds

## 1. Keybind action registry in mbv-core

- [x] 1.1 Add the declared action table (`KeybindAction`: id, section, `default_chords`, gate, policy identity, `rebindable`, `prefix_addressable`), the `KeySection` enum mirroring `SETTING_SECTIONS` order plus `Global`, and `Keybinds` (prefix + per-section router overrides + per-section prefix assignments) in a new mbv-core module. Verify: unit tests assert every id is unique, every section is a `KeySection`, and `Rebindable` includes the router globals and transport actions.
- [x] 1.2 Add the chord-string parser (`"Ctrl+b"`, `"F8"`, `"Shift+Left"`, modifier-order-insensitive) and `Keybinds::defaults()` compiled from the declaration. Verify: parser tests cover canonical and reordered forms plus every rejection; `defaults()` reproduces today's hard-coded chords for each declared action.
- [x] 1.3 Add load-time validation: reserved chords (`RESERVED_CHORDS = ["Ctrl+q"]`), unparseable chord, unknown action id, section mismatch, non-`prefix_addressable` action in a prefix table, prefix collision, router-scope action-vs-action collision, and prefix-namespace action-vs-action collision. Verify: table-driven unit tests cover every rejection class and the accept case, each error naming the offending entry (both entries for the two collision classes).

## 2. Keys configuration in `Config`

- [x] 2.1 Parse `[keys]`, `[keys.<section>]`, `[keys.<section>.prefix]` in `crates/mbv-core/src/config_parse.rs`, routing every entry through the registry validator, including the router-scope and prefix-namespace collision checks (both need the full parsed table before they can run, not per-entry). Verify: parse tests for absent section, partial override, per-section tables, and each rejection — including both collision classes — surfacing through the existing config error path.
- [x] 2.2 Save the `[keys]` tables through the existing read-patch-write path in `config_save.rs`, pruning empty tables. Verify: round-trip test — a config with unrelated sections plus `[keys]` survives save with both preserved and untouched keys intact.
- [x] 2.3 Wire the parsed `Keybinds` onto the shell `Model` from config load. Verify: `cargo check -p mbv -p mbv-core`; a unit test that a loaded override reaches the stored `Keybinds`.

## 3. Policy parameterization

- [x] 3.1 Thread `&Keybinds` through `resolve_policy`/`command_for_policy`; each declared action matches its configured chord instead of `KeyPolicyBinding::matches()`'s literals, with `Keybinds::defaults()` byte-identical to today. Verify: `key_policy` unit tests — rebound action fires on its new chord, its default is inert, defaults reproduce today's resolution.
- [x] 3.2 Update routing-matrix fixtures to construct registry defaults and add rows proving the text-entry and blocking-overlay rules hold for rebound chords. Verify: `cargo nextest run -p mbv` routing matrix green.
- [x] 3.3 Pass the loaded `Keybinds` into production routing through the shell. Verify: tick-integration test — a configured rebind fires through `Application::tick()`.

## 4. Double-tap removal

- [x] 4.1 Remove the candidate timing branch from `apply_deferred_candidate` and the `App.last_space_press`/`last_esc_press` fields; `Deferred` dispatches immediately when the leaf did not consume the chord. Verify: `shell_tests.rs` deferred cases rewritten — unhandled `Space`/`Esc` fire once, a consumed press leaves no state.
- [x] 4.2 Remove `last_space`/`last_escape` and `double_tap()` from `components/library_playback_panel.rs`; `Space` and `Esc` fire single-press when the panel holds focus. Verify: component test — one press produces `TogglePlayPause`/`Stop`.
- [x] 4.3 Drop `App.last_space_press`/`last_esc_press` and the timing branch that reads them in `apply_deferred_candidate` (`shell.rs`) — these fields were never mirrored into `RouterSnapshot`, so there is nothing to remove there; add the new `RouterSnapshot.prefix_armed` field (task 6.1) as its own addition, not a replacement of these. Verify: `cargo nextest run -p mbv` green.
- [x] 4.4 Rewrite `live_tick_characterizes_space_double_tap_lifecycle` and `live_tick_characterizes_escape_double_tap_lifecycle` as single-press behavior records, covering the consumed-chord still suppressing the candidate. Verify: the rewritten integration tests pass and match the modified `semantic-input-arbitration` scenarios.

## 5. Transport actions become declared and rebindable

- [x] 5.1 Split the `Playback` bucket into one registry action per transport behavior (`toggle_play_pause`, `stop`, `seek_back`/`seek_forward`, `next_track`, `previous_track`, `volume_down`/`volume_up`, `toggle_mute`, `toggle_mute_or_cycle_audio`, `cycle_subtitle`, `open_idle_feed_link`), each gated `Playback`; bind payload commands to their fixed call-site values. `gate: Playback` is a routing bucket, not a shared eligibility condition — today's `playback_command_for_key` gates each key individually (`Space`/`Esc`/`<`/`>`/`N`/`P`/`a` require `active || has_remote_session`; `z` only excludes Ctrl; `m`/`-`/`+`/`=` are ungated); each split action must carry forward its own key's condition, not a uniform one. Verify: key_policy unit tests — each action resolves from its default chord under its own (not a shared) eligibility condition, and a rebound transport action fires on the configured chord only.
- [x] 5.2 Exclude `alt_swallow` (and any future blocking-only entry) from the registry, keeping its literal match. Verify: a test asserts it is not configurable, not listed, and still swallows.
- [x] 5.3 Render help's Playback rows from the registry and delete the hand-written `PLAYBACK_HELP_BINDINGS.keys` strings (the table lives in `src/app/action.rs`, not `help.rs`; `render/components/help.rs` only renders it). Rewrite `action_tests.rs`'s `playback_help_bindings_match_playback_command_for_key` characterization test, which locks the deleted table today. Verify: help tests assert the rendered keys equal the registry defaults and follow an override.

## 6. Prefix mode

- [ ] 6.1 Add the App-owned armed flag mirrored as `RouterSnapshot.prefix_armed`; arm/disarm transitions from router outcomes; silent mouse disarm in the mouse path. Verify: tick-integration — arming consumes the prefix chord; a mouse event disarms without altering the event's handling.
- [ ] 6.2 Add the `prefix_arm` top layer (gated `!text_entry_focused` + `!blocking_overlay_open`, blocking semantics) and the armed-dispatch layer (mapped chord whose action's gate currently allows it → action + disarm; mapped chord whose action's gate does not currently allow it → treated as unmapped → Swallow + disarm; Esc/unmapped → Swallow + disarm; prefix → re-arm; no FallThrough while armed). Verify: `key_policy` unit tests for every state-machine arm, including F-keys captured while armed, arming suppressed under text entry and overlays, Esc-while-armed not reaching stop, and a prefix-mapped gated action swallowed (not fired) when its gate is currently closed.
- [ ] 6.3 Prove the full state machine through `Application::tick()`: mapped action executes, unmapped chord swallowed and disarmed, double-prefix re-arms, no chord reaches any component while armed (component counters). Verify: new tick-integration test file green.

## 7. Presentation

- [ ] 7.1 Add the `Keys` destination to the settings panel (component content + render), grouped by `KeySection`, listing the configurable set with router and prefix chords, following the `Services` child-destination precedent. Verify: component/buffer tests — groups equal the shared section list, rows equal the registry set, and an override renders the configured chord.
- [ ] 7.2 Add the `Keys` row to the settings main list with the live summary (prefix and override count), extending `SETTING_SECTIONS`/`SettingKey`. Verify: snapshot test — the row value follows the loaded configuration.
- [ ] 7.3 Add the armed pill in the `chrome_status` idiom, rendered only while armed, following the width-pressure drop-order precedent. Verify: buffer test — pill present while armed, absent otherwise; existing status-bar tests green.
- [ ] 7.4 Render help's Global/configurable chords and the prefix list from the registry. Verify: help test — after a rebind, help shows the configured chord; with defaults, help matches current behavior.

## 8. Gates

- [ ] 8.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv -p mbv-core`. Verify: all green.
- [ ] 8.2 Run `openspec validate add-configurable-keybinds --strict`. Verify: passes with both the new and the modified capability deltas.
