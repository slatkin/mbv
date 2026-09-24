use crate::daemon::*;
// Unit tests for the extracted `DaemonLoop` (change split-daemon-event-loop,
// §3). These build a loop with a recording store so the per-pass persistence
// can be asserted without touching real state files, and drive one event at a
// time through `handle_event` so the process never exits.

