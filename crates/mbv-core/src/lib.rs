//! Application logging and Service runtime state.
//!
//! `applog` installs the one `tracing` dispatcher; `service_runtime` tracks per-Service
//! availability (`ServiceState`) and setup ordering (`SetupGeneration`). Config file
//! handling lives in `mbv-config`; this crate only logs and tracks runtime state.

pub mod applog;
pub mod service_runtime;
