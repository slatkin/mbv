//! The intent methods the TUI shell calls to change what the core holds.
//!
//! A Client never reaches an `App` field (change `split-tui-gui-clients`,
//! design D4/D4b rules 1–3). A read names the fact the Client asks for
//! (`shell_reads`); a write names the intent the Client performs, and the
//! core keeps the logic and the field. So the shell passes what it decided —
//! the width it clamped, the row the control resolved, the fact its pickers
//! measure — and never a field, and no method here hands back a `&mut`.
//!
//! The methods that already answered an intent before this module existed
//! stay where they are (`set_library_tab`, `set_panel_focus`,
//! `close_settings`, `accept_home_recently_added`, ...); this module adds only
//! the writes the shell made directly until now.
//!
//! The field groups follow `shell_reads`': the panel, presentation, and
//! launch facts are in `presentation`, the Settings, Service, and feed facts
//! in `config`, the catalogue and playlist facts in `browse`, and the
//! Player-link, queue, and Home facts in `playback`.

mod browse;
mod config;
mod playback;
mod presentation;
