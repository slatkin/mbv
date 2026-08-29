## Context

`Application::tick()` is called in exactly one place: `src/app/shell_run.rs:383`,
inside `Model::run`'s `'outer` loop. That loop also owns the terminal, spawns
workers, and blocks on real time, so no test has ever entered it. Every
TuiRealm test in the repo therefore stops one layer short of the framework:

| Test style | Example | What it cannot observe |
| --- | --- | --- |
| `component.on(&event)` | `queue_component_tests.rs` | who was focused, whether the event would have been delivered at all |
| shell seam called directly | `shell_overlays_tests.rs:264` (`tick_search_clock`) | whether the run loop actually calls that seam |
| hand-built message list | `tests_routing_matrix.rs` (`fold_tick`) | whether `tick()` produces that list, in that order |

#607's two runtime defects both live precisely in that gap. #609: the Search
component's debounce was correct and nothing ever drove it. #610: `sync_queue`
activated Queue and `sync_active_destination`, four lines later, activated the
Library child instead. Both are composition defects with no defective
component.

`tests_routing_matrix.rs:3-7` documents why tick-level testing was ruled out —
that `with_test_barrier` is crate-private to tuirealm. The conclusion drawn
from it (that events cannot be injected) does not follow.
`EventListenerCfg::add_port(Box<dyn Poll<UserEvent>>, Duration, usize)` is
public (`listener/builder.rs:220`), `Poll` is a public trait
(`listener/mod.rs:75`), and `PollStrategy::Once(Duration)` waits up to the
given timeout for one event (`core/application.rs:456`). A test port is ~20
lines.

## Goals / Non-Goals

**Goals**

- One harness that drives a real `Application::tick()` in-process, with no
  terminal, no `Model::run`, and no wall-clock sleeps beyond the debounce test
  that already exists.
- Coverage of the four behaviours #607 names: delivery, final focus after the
  full sync sequence, Search clock delivery, overlay blocking.
- A test that would have failed on the #610 defect and on the #609 defect.

**Non-Goals**

- No runtime behaviour change. This change adds seams and tests; it does not
  alter what the app does.
- Not replacing `tests_routing_matrix.rs`. Table-driven precedence rows are
  cheap and cover combinations a tick harness would be wasteful for. The
  matrix keeps its rows; only its stale premise comment changes.
- Not wiring a production `UserEvent::Clock` publisher. The shipped fix for
  #609 is the shell-side sweep; replacing it is a runtime change, out of scope
  for a test issue (see D3).
- Not repairing mouse. D16 stands: mouse is accepted-broken for the alpha and
  gets no rows here.

## Decisions

### D1 — Substitutable listener via a second constructor

`Model::new(app)` builds its `EventListenerCfg` inline
(`shell.rs:248-250`). Split it:

pub fn new(app: App) -> Self {
    Self::new_with_listener(
        app,
        EventListenerCfg::default().crossterm_input_listener(
            TERMINAL_LISTENER_INTERVAL, TERMINAL_LISTENER_MAX_POLL),
    )
}

pub(in crate::app) fn new_with_listener(
    app: App,
    listener_cfg: EventListenerCfg<UserEvent>,
) -> Self {
    /* today's body */
}

*Alternatives rejected.* A `#[cfg(test)]` field on `Model` — invisible in
production reading, and the mount sequence in `new` would still have to be
duplicated. A trait-abstracted listener — an interface with one production
implementation, for a seam a second constructor already provides.

`new_with_listener` is `pub(in crate::app)`, not `pub`: only tests in this
module tree need it, and keeping it crate-internal means no external caller can
start a `Model` with a listener that never delivers input.

### D2 — The sync sequence becomes one callable unit

The run-loop block from `self.update_settings_content()` through
`self.sync_active_destination()` (`shell_run.rs:426-445`) moves verbatim into
`Model::sync_mounted_surfaces(&mut self)`; `run()` calls it in the same place.
The block touches only `&mut self` — no loop-local bindings — so the extraction
is mechanical.

This is the load-bearing half of the change. If the focus test instead called
`sync_queue()` and `sync_active_destination()` itself, it would be asserting
against the order the *test author* believed in, which is exactly the belief
#610 falsified. Calling the shell's real sequence means a future reorder — a
new `sync_*` appended after `sync_active_destination` that activates something
else — fails the test.

The extracted method keeps the block's existing comments, including the
`reconcile_destination_mounts()` ordering note.

### D3 — Search clock: cover both paths, change neither

`UserEvent::Clock` exists in the enum and `SearchSidebarComponent` handles it
(`search_sidebar.rs:196`), but production drives the debounce through the
shell sweep `tick_search_clock` instead (`shell_run.rs:221-229`, added for
#609). Two paths, one shipped.

Both get coverage, for different reasons:

- The **framework contract**: injecting `Event::User(UserEvent::Clock)` through
  the port and calling `tick()` proves TuiRealm delivers user events to the
  mounted component — the D5/D12 mechanism the whole `UserEvent` enum exists
  for, currently unproven anywhere in the suite.
- The **production path**: the harness `step()` runs the sweep, so the
  debounced query dispatches exactly as it does in `run()`.

Covering both means a future change that deletes the sweep in favour of a real
Clock port has to delete a test that names the sweep — a visible decision
rather than silent drift. Rejected alternative: wiring the production Clock
port now. It is the design-truer answer and it is a runtime rewrite of a
working fix, filed under a test issue; if wanted, it is its own change with its
own risk.

### D4 — Harness `step()` mirrors the run loop, in the run loop's order

sync_mounted_surfaces()
tick_search_clock(now) -> dispatch any Msg
focus = application.focus().cloned()      // before the fold, as run() does
messages = application.tick(PollStrategy::Once(TICK_TIMEOUT))
outcome = router_outcome(&messages)
apply_router_outcome(messages, focus, &outcome)

Note the order difference from `run()`: `run()` ticks first and syncs
afterwards, because it is a loop and the previous iteration's sync has already
happened. `step()` syncs first so that a single call is a complete
"state settled, then input arrives" unit. This is stated because it is the one
place the harness is not a literal transcription of the loop body.

The focus snapshot is taken before the fold, matching `shell_run.rs:396` and
its comment about a legacy key mounting an overlay mid-fold.

Returning both the surviving messages and the pre-fold focus lets one assertion
cover "who had focus" and "what survived" — the two halves of an overlay-blocking
claim.

### D5 — Determinism: bounded wait, not sleep

The listener is a background thread polling ports at an interval, so delivery
is asynchronous by construction. `PollStrategy::Once(Duration)` blocks up to
its timeout for one event, so a generous test timeout (500 ms) makes delivery
deterministic without a `sleep`: the injected event arrives on the port's first
poll, typically within a millisecond, and the timeout is only ever paid on
genuine failure.

`BlockCollectUpTo` was rejected — it blocks unbounded, so a wiring regression
would hang CI instead of failing it.

Repository rule: flaky tests get replaced, not debugged. A test that asserts on
"no message arrived" is the flaky shape here, since it can only ever wait a
finite time and pass early. Negative assertions are therefore phrased as
"delivered to X" (a positive claim about a different target) rather than "not
 delivered to Y", except where the target list is closed and every member is
checked.

### D6 — Two files, not one

`tests_tick_harness.rs` holds the port and `TickHarness`; `tests_tick_integration.rs`
holds the four test groups. Splitting them keeps each under the 800-line cap
with room for the groups to grow, and lets a later test file reuse the harness
without importing an assertion suite.

Both are `#[cfg(test)]` modules registered in `src/app/mod.rs`, matching the
existing `tests_*.rs` convention (`tests_panel_focus.rs`, `tests_routing_matrix.rs`).

### D7 — Fixtures reuse `make_app_stub`

`Model::new(make_app_stub())` is the established shell-test fixture
(`shell_tests.rs:8`, `shell_home.rs`, `tests_feeds_manage.rs`). The harness
takes an `App` so a test needing a populated library passes
`render::make_movie_app()` (already used at `actions_tests_letter.rs:154`)
instead. No new fixture is introduced.

## Risks / Trade-offs

- **A background listener thread per harness instance.** Each `TickHarness`
  starts one. With a handful of tests this is negligible; if the count grows
  past a few dozen, the port's poll interval is the knob. Accepted.
- **The harness can drift from `run()`.** `step()` transcribes the loop body;
  if `run()` gains a step, `step()` does not follow automatically. Mitigated by
  D2 (the largest block is now shared code, not transcription) and by a comment
  in `run()` pointing at the harness. Not fully solvable short of extracting
the entire loop body, which would drag terminal and worker lifecycle into the
seam.
- **`new_with_listener` widens `Model`'s constructor surface.** One extra
  crate-internal function, and `new` is a two-line delegate. Cheap.
- **Timeout choice.** 500 ms is ~500× the expected delivery latency and is only
  paid when a test is already failing. If CI is slow enough to make that tight,
  the wiring is not what is being measured.

## Migration Plan

Ordered so each unit compiles and the suite passes:

1. D1 constructor split — no test uses it yet; suite must stay green.
2. D2 sync extraction — pure move; suite must stay green.
3. Harness (port + `TickHarness`) with one smoke test proving an injected key
   comes back out of `tick()`. This is the go/no-go: if injection does not
   work, nothing after it is worth building.
4. The four test groups, one commit each.
5. Correct the stale comment in `tests_routing_matrix.rs`.

## Open Questions

None. The three scope decisions (both seams, cover both clock paths, assert
through the router fold) were settled with the user before this change was
written.
