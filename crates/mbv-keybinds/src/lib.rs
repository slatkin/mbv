//! Configurable keyboard bindings: the declared action registry, the chord
//! grammar, and load-time compilation/validation of the `[keys]` config.
//!
//! The submodules each own one reason to change: `registry` declares every
//! configurable action, `chord` parses and renders chord strings, and
//! `config` compiles raw file entries into validated `Keybinds`.

mod chord;
mod config;
mod registry;

pub use chord::{Chord, ChordParseError, Key, KeyMods};
pub use config::{Keybinds, KeybindsError, RawKeybinds, RawSection, SectionBindings, load};
pub use registry::{
    KEY_SECTIONS, KEYBIND_ACTIONS, KeyGate, KeySection, KeybindAction, RESERVED_CHORDS,
    action_by_id,
};

#[cfg(test)]
mod tests;
