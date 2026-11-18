//! Headless tests of the player panel (D16): the contracts of
//! `Panel.test.ts`. `Panel` itself owns the four states the container size
//! and the core's two toggles pick, and the composition each state paints;
//! this module also carries the 5.7 children's seeded painted-content
//! contracts — the compact strip's meta line with and without a next item,
//! the folded queue's remaining count and finish clock, the rail's
//! remaining time, expand labels, and dim fill — and the region orders the
//! the former tests awaited. The footer line's own painted content and its
//! regionless report are `queue_footer/tests.rs`; the Queue rows' are
//! `queue_panel/tests/`. The clocks are the injected recorders over a fixed
//! wall clock (D10); the core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.
//!
//! `harness` holds the shared fixture, `states` the panel-state and region
//! contracts, and `painted` the seeded painted-content contracts.

mod harness;
mod painted;
mod states;
