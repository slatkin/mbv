//! Blitz spike: the Queue panel as a Dioxus-native window. Scaffold only; the
//! window, the core wiring, and the panels arrive in later tasks of
//! `blitz-queue-panel-spike`.

mod art_net;

// `profiling` drops the allocator so heaptrack sees the real heap (D16).
#[cfg(not(feature = "profiling"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {}
