## 1. Previous transport control

- [x] 1.1 Retain the prev glyph's hit geometry in the painter beside stop and next, at every paint and clear site, and give the glyph the availability-driven role next already uses; verify with the plain and nerd-font strip buffer tests that the prev area matches the painted glyph width
- [x] 1.2 Thread prev availability through the panel projection and resolve a prev click in both panels to the typed previous-transport intent; verify with a click test in each panel asserting the prev and next areas stay distinct and emit their own intents

## 2. Bounded event observation

- [x] 2.1 Bound the run loop's idle wakeup wait and document it as the event-latency ceiling, sharing the constant with the no-pipe fallback; measure a live mpv-initiated navigation and confirm the run observes it within the ceiling instead of the previous two seconds

## 3. Close-out

- [x] 3.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv -p mbv-core`, then archive the change so its deltas land in `openspec/specs/`
