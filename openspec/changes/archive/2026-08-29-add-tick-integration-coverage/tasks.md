## 1. Production seams (D1, D2 — behaviour-preserving, no new tests)

- [x] 1.1 Split `Model::new` in `src/app/shell.rs`. Add
  `pub(in crate::app) fn new_with_listener(app: App, listener_cfg:
  EventListenerCfg<UserEvent>) -> Self` holding today's body minus the
  `EventListenerCfg::default().crossterm_input_listener(...)` construction;
  `new(app)` becomes a delegate that builds that config and calls it. The
  mount sequence (UiRoot, Home, Feeds, Playback, `update_settings_content`)
  stays in `new_with_listener` — verify: `rtk cargo check -p mbv`, full
  suite green, and `git show` shows no line of the mount sequence changed.
- [x] 1.2 Extract `Model::sync_mounted_surfaces(&mut self)` from
  `src/app/shell_run.rs`. Move the contiguous block from
  `self.update_settings_content();` through `self.sync_active_destination();`
  — including `reconcile_destination_mounts()` and every comment in the block
  — into the new method, and call it from that exact spot in `run()`. Do not
  reorder, add, or drop a call. Put the method next to `run()` in
  `shell_run.rs` unless the file exceeds the 800-line cap, in which case
  `shell_sync.rs`. Verify: `rtk cargo nextest run -p mbv` green, and
  `git show` proves the call list is identical in order and membership.
- [x] 1.3 Add a one-line comment above the `sync_mounted_surfaces()` call in
  `run()` naming `tests_tick_harness.rs` as the other caller, so a future
  reorder is noticed. Verify: `rtk cargo fmt`, `rtk cargo clippy
  --workspace --all-targets` clean.

## 2. Harness (D4, D5, D6 — go/no-go for the rest of the change)

- [x] 2.1 Create `src/app/tests_tick_harness.rs` and register it in
  `src/app/mod.rs` as a `#[cfg(test)]` module, matching the existing
  `tests_*.rs` registrations. Implement `struct InjectPort` holding an
  `std::sync::mpsc::Receiver<Event<UserEvent>>` with
  `impl Poll<UserEvent> for InjectPort { fn poll(&mut self) ->
  PortResult<Option<Event<UserEvent>>> { Ok(self.rx.try_recv().ok()) } }`.
  Note: `poll` must not block — returning `Ok(None)` on an empty channel is
  the contract (`tuirealm-4.1.0/src/listener/mod.rs:84`).
- [x] 2.2 Implement `TickHarness` in the same file: `new(app: App) -> Self`
  builds the injecting `EventListenerCfg` (`add_port(Box::new(port),
  interval, max_poll)`) and calls `Model::new_with_listener`, keeping the
  `Sender` alongside the `Model`. Expose `model()`/`model_mut()` accessors
  and `inject(&self, event: Event<UserEvent>)`.
- [x] 2.3 Implement `TickHarness::step(&mut self) -> StepOutcome` per design
  D4, in this order: `sync_mounted_surfaces()`; `tick_search_clock(Instant::
  now())` dispatching any returned `Msg::Service` through
  `handle_service_request`; snapshot `application.focus().cloned()`;
  `application.tick(PollStrategy::Once(TICK_TIMEOUT))`; `router_outcome`;
  `apply_router_outcome`. `StepOutcome` carries the surviving `Vec<Msg>`, the
  pre-fold focus, and the `RouterOutcome`. `TICK_TIMEOUT` is 500 ms with a
  comment citing D5 (bounded wait, never a `sleep`).
- [x] 2.4 Smoke test in `tests_tick_harness.rs`: inject one
  `Event::Keyboard`, call `step()`, assert at least one `Msg` comes back.
  **This is the go/no-go gate** — if injection does not reach `tick()`, stop
  and report before writing section 3. Verify: `rtk cargo nextest run -p mbv`.

## 3. Integration coverage (D3, four groups — one commit each)

- [x] 3.1 Create `src/app/tests_tick_integration.rs`, register it in
  `src/app/mod.rs`.
- [x] 3.2 **Delivery.** With Queue focused, inject a key Queue handles and
  assert `step()` returns the Queue leaf message first and the `UiRoot`
  observer's `Msg::TerminalEvent` second (`shell_run.rs`'s documented order),
  each exactly once. Assert the same key injected twice yields two ticks'
  worth, not one tick's doubled — guarding the double-delivery hazard the
  focus-snapshot comment names.
- [x] 3.3 **Final focus after the full sync sequence (#610 regression
  guard).** Set `PanelFocus::Queue` on the stub `App`, call
  `sync_mounted_surfaces()` once, assert `application.focus() ==
  Some(&ComponentId::Queue)` *after the whole sequence*. Then set panel focus
  to Library and assert focus lands on the Library child id (or
  `ComponentId::UiRoot` when no child is mounted, per `shell_library.rs:35`).
  The assertion must follow the full sequence, never an individual `sync_*`.
- [x] 3.4 **Search clock — framework path.** Mount the Search sidebar
  (`mount_sidebar(SidebarId::Search)`), inject
  `Event::User(UserEvent::Clock(Instant::now()))`, `step()`, and assert the
  mounted `SearchSidebarComponent` observed it. Assert on component-visible
  effect, not on the port.
- [x] 3.5 **Search clock — production path.** Arm the debounce through the
  component's keyboard arm, then assert `step()`'s sweep dispatches the
  query through `handle_service_request` after the deadline. Reuse the
  310 ms real-sleep approach and the rationale already documented at
  `shell_overlays_tests.rs:255-262` rather than inventing a second timing
  strategy.
- [x] 3.6 **Overlay blocking.** Mount a blocking overlay from
  `blocking_overlay_active()`'s list (`shell_root.rs:48-58`) — Confirm is the
  cheapest to construct. Then assert all three: (a) `sync_mounted_surfaces()`
  leaves focus on the overlay, not Queue or the destination —
  `sync_active_destination`'s `library_overlay_mounted()` guard
  (`shell_library.rs:24`) is what this pins; (b) an injected key's leaf
  message comes from the overlay; (c) a global chord resolves to
  `RouterOutcome::Swallow` and the leaf message does not survive the fold.
- [x] 3.7 After each group: `rtk cargo nextest run -p mbv` full suite.

## 4. Close-out

- [x] 4.1 Correct `src/app/tests_routing_matrix.rs`'s module comment
  (lines 3-7): the claim that mbv cannot inject terminal events into a live
  `Application::tick` is false — `EventListenerCfg::add_port` is public.
  Replace it with a pointer to `tests_tick_harness.rs` and a one-line note on
  why the table-driven matrix is still kept (cheap precedence combinations).
  Do not delete or change any matrix row.
- [x] 4.2 `rtk cargo fmt`, `rtk cargo clippy --workspace --all-targets`,
  `rtk ast-grep scan`, `rtk make check-code-file-lines`.
- [x] 4.3 `rtk cargo nextest run -p mbv` full suite green.
- [x] 4.4 `openspec validate add-tick-integration-coverage --strict`.
- [x] 4.5 Tick #612's "Done when" boxes and #607's "Framework-level tests
  prove focus, delivery, overlay blocking, and runtime event behavior"
  acceptance criterion. Note in #614 that the last #607 criterion is closed.
