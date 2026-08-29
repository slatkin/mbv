## Why

Every current TuiRealm test stops short of the framework. Component tests call
`component.on(&event)` directly; the "production-style" shell tests
(`shell_overlays_tests.rs:264`, `tests_routing_matrix.rs`) call the seam
under test — `tick_search_clock`, `apply_router_outcome` — with hand-built
message lists. Nothing in the suite calls `Application::tick()`.

That is why the two runtime defects in #607 shipped. #609 (Search debounce
never armed a dispatcher) and #610 (`sync_active_destination` stealing focus
back from Queue) are both *wiring* defects: each component behaved correctly in
isolation, and the composition was wrong. A test that hand-assembles the
message list or calls one `sync_*` method cannot observe either.

`tests_routing_matrix.rs:3-7` records the reason this was skipped: "TuiRealm's
`with_test_barrier` is `#[cfg(test)]` inside the tuirealm crate, so mbv's tests
cannot inject terminal events into a live `Application::tick`". That premise is
false. `EventListenerCfg::add_port(Box<dyn Poll<UserEvent>>, interval,
max_poll)` (`tuirealm-4.1.0/src/listener/builder.rs:220`) is public, and
`Poll::poll(&mut self) -> PortResult<Option<Event<UserEvent>>>` is a public
trait. A channel-backed test port injects both `Event::Keyboard` and
`Event::User(UserEvent::Clock)` into a live `Application`, and
`PollStrategy::Once(Duration)` waits for that event. Real tick coverage is
available and always was.

This is #612, the last open acceptance criterion of #607: "Framework-level
tests prove focus, delivery, overlay blocking, and runtime event behavior."

## What Changes

- **Two production seams**, both behaviour-preserving:
  - `Model::new_with_listener(app, EventListenerCfg)` in `src/app/shell.rs`;
    `Model::new` becomes a thin delegate that supplies the crossterm listener.
    Tests substitute an injecting port.
  - `Model::sync_mounted_surfaces()` extracted verbatim from the run-loop body
    in `src/app/shell_run.rs` (`update_settings_content()` through
    `sync_active_destination()`, `reconcile_destination_mounts()` included, in
    that order). `run()` calls it. Tests get the *real* order, so an ordering
    regression of the #610 class is observable.
- **A test harness** (`src/app/tests_tick_harness.rs`): an mpsc-backed
  `Poll<UserEvent>` port, a `TickHarness` owning a `Model` built on it, and one
  `step()` that reproduces the run-loop's own sequence —
  `sync_mounted_surfaces()`, then the search clock sweep, then
  `application.tick(PollStrategy::Once(..))`, then `router_outcome()` +
  `apply_router_outcome()` — returning the surviving `Msg` list and the focus
  snapshot taken before the fold.
- **Four integration test groups** (`src/app/tests_tick_integration.rs`):
  1. *Delivery.* An injected key reaches the focused component and the UiRoot
     observer, in tick's documented order (leaf message first, observer
     second), exactly once.
  2. *Final focus after the full sync sequence.* After
     `sync_mounted_surfaces()`, focus is `ComponentId::Queue` when
     `PanelFocus::Queue` holds (the #610 regression guard), and the Library
     child (or `UiRoot`) when it does not.
  3. *Search clock delivery.* An injected `Event::User(UserEvent::Clock)`
     reaches the mounted `SearchSidebarComponent` through tick (the framework
     contract, design D5/D12); separately, the shipped shell sweep dispatches
     the debounced query through `handle_service_request` (the production path,
     #609).
  4. *Overlay blocking.* With a blocking overlay mounted,
     `sync_mounted_surfaces()` does not steal focus from it, tick delivers the
     key to the overlay rather than Queue or the destination, and the router
     resolves `Swallow` for a global chord.

## Capabilities

### Modified Capabilities

- `interactive-component-framework`: the "Complete conversion with no
  mixed-framework endpoint" requirement gains a verification clause — focus,
  event delivery, overlay blocking, and runtime user-event behaviour SHALL be
  proven at the `Application::tick()` level against the shell's real sync
  order, not by hand-assembled message lists or direct `component.on()` calls.

## Impact

- `src/app/shell.rs` (constructor split), `src/app/shell_run.rs` (sync block
  extracted to a method), `src/app/mod.rs` (two new `#[cfg(test)]` modules).
- New: `src/app/tests_tick_harness.rs`, `src/app/tests_tick_integration.rs`.
- `src/app/tests_routing_matrix.rs`: the stale module comment claiming tick
  injection is impossible is corrected to point at the new harness. The matrix
  itself stays — it is cheap table-driven precedence coverage that the
  integration tests do not replace.
- No runtime behaviour change. `Model::new`'s production listener config is
  unchanged, and the extracted sync block keeps its exact call order.
- Closes the last open acceptance criterion of #607; unblocks #614's
  ledger/ADR/source reconciliation.
