//! The mbv Client core.
//!
//! One core answers for both mbv Clients: the terminal Client (`mbv`) and the
//! GUI Client (`mbv-gui`). It holds the Client-side Owner link, the Service
//! setup and queue authority, playback dispatch, and the keyboard routing that
//! both Clients share. It names no terminal or window toolkit, so neither
//! Client's dependencies reach the other's.
//!
//! The core's modules move in as the first step of the crate split; this
//! library root is a placeholder until then.
