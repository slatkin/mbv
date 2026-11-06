# Invariant 29 — Each GUI input event reaches the router exactly once

**Scope:** `tick_input` (`crates/mbv-gui/src/tick_host.rs`), the headless harness and its frame runners (`crates/mbv-gui/src/queue/tests.rs`).

## The invariant

1. A raw input event egui delivers on a shown frame reaches the keyboard router (or a pointer-gesture recognizer) exactly once, no matter how many further passes eframe runs for that frame.

## Why it matters

eframe 0.37 runs `App::logic` without an `App::ui` pass whenever the window cannot paint — a minimized or fully occluded window, or a Wayland LogicOnly pass. On those passes `ctx.input()` still holds the last shown pass's events, because egui clears them only when a `ui` pass consumes the frame. A reader that clones the event list there re-routed a `Space` press or a click again on every logic-only pass, about every 50 ms while the window stayed hidden: Visual-mode selection flashed a play/pause toast, and a click replayed. Found in review of row 6.10 of `add-gui-front-end`; fixed in 9ea0bfc99.

## How the code maintains it today

- `tick_input` reads the frame's events with `ctx.input_mut` and takes them (`std::mem::take`), never clones. A logic-only pass then sees an empty event list, and egui refills it only on the next real delivery.
- The headless harness exposes a logic-only frame runner (`run_logic_only`), and `a_space_press_routes_once_across_logic_only_passes` (`crates/mbv-gui/src/queue/tests.rs`) proves a shown-frame `Space` routes once: the mark count and the status projection stay unchanged across two logic-only passes.

## Where it currently fails / how it could regress

- Any new reader of `ctx.input().events` that copies instead of takes — in `logic`, in a helper, or in a test harness runner — re-routes the last shown frame's events on every pass without a `ui`. Read the events through `tick_input`'s take, or take them the same way.
- A change that moves the take out of the per-frame tick path (for example, caching the event list across frames) reintroduces the same replay.
