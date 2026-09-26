//! Shared builders for the key-policy tests: default and rebound keybinds, a Both-panels snapshot, and the normalizing chord constructor.
use super::*;
use mbv_core::keybinds::{Chord, KeySection, SectionBindings};

pub(super) fn keybinds() -> Keybinds {
    Keybinds::default()
}

/// A configuration with one router-scope override: `id` fires on `chord`
/// instead of its declared default.
pub(super) fn rebound(id: &'static str, chord: &str) -> Keybinds {
    Keybinds {
        prefix: None,
        sections: vec![(
            KeySection::Global,
            SectionBindings {
                router: vec![(id, Chord::parse(chord).expect("test chord must parse"))],
                prefix: vec![],
            },
        )],
    }
}

pub(super) fn snapshot() -> RouterSnapshot {
    RouterSnapshot {
        panel_mode: PanelMode::Both,
        ..RouterSnapshot::default()
    }
}

pub(super) fn chord(code: KeyCode, mods: KeyModifiers) -> KeyChord {
    // Through the normalizing constructor, mirroring how the router
    // derives a chord from a pressed key (`KeyChord::from_key`).
    KeyChord::new(code, mods)
}
