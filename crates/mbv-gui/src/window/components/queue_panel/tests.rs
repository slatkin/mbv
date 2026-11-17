//! Headless tests of the Queue panel (D16): the contracts of
//! `QueuePanel.test.ts` the window's port keeps — the frame's viewport rows
//! with their watched, active, and partial-progress states, the empty-queue
//! state, the rows' right column under the summary's mode, the hovered row's
//! grip and actions button, the open row menu's chrome and hints, the drag's
//! ghost and insertion line, and the capacity and region targets the panel
//! reports — plus the two region-order contracts
//! `QueuePlaybackPanel.test.ts` deferred until this panel existed (an open
//! chooser's rows reporting after the queue's, and the full player panel's
//! region order). The header's own contracts (the header tests of
//! `QueuePanel.test.ts`) arrived with the header's port (task 5.6). The
//! presses are not ported: they are the root input layer's contract
//! (`window/input.rs`), which the panel forwards to untouched. The core
//! channel is a mock pair; the clocks are the injected recorders (D10); no
//! test opens a window, sleeps, or reaches the core.

mod chooser;
mod cross_panel;
mod harness;
mod header;
mod menu_drag;
mod regions;
mod rows;
mod theme;
