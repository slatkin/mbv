//! The queue card's core part (design D3): the artwork cache keys, the fetch
//! chains, and the title overlay's logo owner. It builds no ratatui value, so
//! it stays in `state`; the projection that reads it is the shell's
//! (`shell::queue_card`), which re-exports these items.

pub(in crate::app) mod artwork;
