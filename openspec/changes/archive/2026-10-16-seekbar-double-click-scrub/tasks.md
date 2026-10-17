# Tasks

Package for every command: `mbv-components` unless stated. Test contracts are in `design.md` D4.
Add no test beyond the ones named here.

## 1. Queue playback panel

- [x] 1.1 In `crates/mbv-components/src/queue_playback_panel.rs`, add `mouse_gestures: MouseGestureState` and `scrub_at: Option<Instant>` fields, the `SEEKBAR_SCRUB_INTERVAL` constant with its doc line, and the `crate::mouse::gesture` import. Change `mouse(&self, ..)` to `mouse(&mut self, ..)` and feed every event to `self.mouse_gestures.recognize(event)` first. Change the seekbar arm to require the recognized double-click, and add the drag arm (in-bar anchor on both ends, throttled). Extract the shared fraction arithmetic into a private helper both arms call. Verify: `cargo nextest run -p mbv-components queue_playback_panel`.
- [x] 1.2 In `crates/mbv-components/src/queue_playback_panel/tests.rs`, extend `transport_clicks_resolve_against_retained_geometry` (contract: the retained transport geometry resolves each intent): the first press on the seekbar resolves nothing and the second resolves the fraction. Add one test for the scrub admission rule (contract: a scrub needs an in-bar anchor and one admitted seek per interval). Verify: `cargo nextest run -p mbv-components queue_playback_panel`.

## 2. Library playback panel

- [x] 2.1 In `crates/mbv-components/src/library_playback_panel.rs`, apply the same fields, import, `&mut self` change, recognizer call, double-click seekbar arm, and throttled drag arm. Verify: `cargo nextest run -p mbv-components library_playback_panel`.
- [x] 2.2 In its test module, extend `seekbar_click_resolves_a_fraction_against_the_painted_seekbar_area` to the double-click gate and rename it for that contract. Add one test for the scrub admission rule. Verify: `cargo nextest run -p mbv-components library_playback_panel`.

## 3. Shell tick integration

- [x] 3.1 In `src/app/tests/tick_integration/queue_playback.rs`, make `tick_clicks_play_pause_and_the_seekbar_in_both` and `tick_clicks_play_pause_and_the_seekbar_in_mini_view_queue_only` inject the pair in two steps, assert the first step's seek is absent, and keep the fraction assertion on the second. Verify: `cargo nextest run -p mbv queue_playback`.
- [x] 3.2 In `src/app/tests/tick_integration.rs`, make `tick_delivers_seekbar_click_to_unfocused_playback_as_a_fraction` inject the pair in two steps. Its contract stays delivery to the unfocused strip. Verify: `cargo nextest run -p mbv tick_delivers_seekbar`.

## 4. Spec sync and gates

- [x] 4.1 Sync the delta into `openspec/specs/queue-playback-panel/spec.md` (`openspec archive` guidance: sync without asking) and check `docs/architecture/tui-frontend.md`, `docs/architecture/interactive-surface-ledger.md`, and `openspec/specs/mouse-input/spec.md` for text that still describes a single-click seek. Verify: `rg -n "click on its seekbar" openspec/specs docs` shows only the double-click wording.
- [x] 4.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv-components -p mbv`. Verify all three pass.
