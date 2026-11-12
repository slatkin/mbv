# Invariant 32 — A section mark covers every source that can change the section

**Scope:** the GUI Client's section ledger (`crates/mbv-gui/src/sections.rs`), the drain-hook mark
table (`crates/mbv-gui/src/sections/marks.rs`, `marks_for_hook`), the deadline and playhead sampler
(`crates/mbv-gui/src/sections/deadlines.rs`), the mark calls in the GUI's `ClientTickHost` impl
(`crates/mbv-gui/src/core_thread/tick_host.rs`), and the test-only audit
(`CoreThread::set_section_audit_for_test`). Landed by the `gui-change-driven-sections` change.

## The invariant

A gated section builds only while a mark names it. A source that can change a gated section's built
value SHALL set that mark on the path that ran the change. A gated section whose source changed with
no mark stays stale in the window until the next input, with no error anywhere.

## Why it matters

The mark is the only signal a gated section build gets. There is no revision inside `App` and no
compare that runs without a mark. A missed mark is silent: the core builds nothing, sends nothing,
logs nothing, and the window keeps painting the old value for as long as no input arrives. An input
arrives only when the user acts, so a missed mark can hide for a whole session — a track change that
moves the playing row, a landed search result, or a passed deadline can each wait on the user's next
keystroke to appear.

## How the code upholds it today

- The mark table is one function, `marks_for_hook`, over a `DrainHook` enum with one variant for
  each `ClientTickHost` method, no-ops included. Its `match` is exhaustive, so the compiler forces a
  row for every variant that exists.
- Each drain method in the GUI's `ClientTickHost` impl calls `mark_hook` with its own variant when it
  reports work. The call sits inside the method, beside the drain it guards.
- The input phase (`tick_input`) marks every gated section for a key, a Library command, a Settings
  command, and a press or right-press gesture. A move, a wheel step, a drag, a release, a cancel, a
  layout report, a `sync`, and an art request mark none: they change only compared sections.
- The deadline sampler (`deadlines.rs`) marks the Library section when the navigation-idle gate
  opens, when an interrupted art retry falls in the tick-to-tick window, or when the playhead's
  playing track or book moved. It marks the Settings section when the playback target choices moved.
- The ledger marks every gated section once at construction, so a new window gets every gated
  section's first build without any input.
- The audit is the guard the compiler cannot give. `test_core_with` calls
  `CoreThread::set_section_audit_for_test(true)`, so every core-thread test builds each clean gated
  section on each tick and panics when the build differs from the last built value. Production never
  turns it on.

## Where it still fails

- A new `ClientTickHost` method gets no `DrainHook` variant for free. The trait impl forces only a
  body for the new method; nothing forces a variant, a mark-table row, or a mark call. The audit
  catches it only in a test that exercises the method's source change.
- A no-op row keeps its empty mark set after its method gains GUI work. The search clocks
  (`tick_search_clock`, `tick_inline_search_clock`), `handle_service_request`, and
  `run_periodic_maintenance` mark nothing today because their GUI bodies change nothing. A later
  change that adds GUI work to one of them must add the mark on the same change.
- An `App`-side drain with no host hook can change a gated section's source with no mark. The App
  drains that no GUI hook wraps (`drain_session_events`, `drain_cast_events`,
  `drain_playback_metadata_events`, `drain_trickplay_sheet_events`, `drain_notif_actions`,
  `drain_pinned_requests`, `settle_pending_queue_transfer`) feed compared sections today, or reach a
  gated section through the playhead mark. A new App-side drain that changes a gated source has no
  mark site.
- The audit's clean builds start the Hero neighbour warm, so a missed navigation-gate mark is
  invisible to the audit: the audit's own Library build runs `warm_gui_hero_neighbours` and can start
  the same fetch the missed mark would have started. The navigation-gate test turns the audit off for
  this reason (`a_passed_navigation_gate_starts_the_hero_neighbour_fetch_with_no_input`).
- A test that changes `App` state between ticks with no drain and no input reports as an audit
  failure, because the next clean build differs from the last built value. Such a seed goes through a
  real drain hook or a test-only mark seam (`mark_startup_landing_for_test`,
  `mark_library_landing_for_test`), never through bare App mutation.
