//! `[keys]` config save (change add-configurable-keybinds U2, and
//! `add-gui-settings-screen` D8, task 1.5).
//!
//! Save goes through the read-patch-write path. Only the Keys editor's
//! `ConfigSection::Keys` write touches `[keys]`; a settings save leaves the
//! table exactly as the file has it. The `[keys]` parse and rejection tests
//! own the crate's public contract and live in the `tests/config` integration
//! binary.

#[cfg(test)]
use crate::{
    Config, ConfigSection, config_path, parse_config, save_config_section, save_config_settings,
};

#[test]
fn keys_section_save_round_trips_unrelated_sections_and_keys() {
    let _guard = crate::TestStateDirGuard::new();
    let original = r#"
[server]
url = "http://localhost:8096/"
user_id = "user-1"
legacy_note = "keep me"

[keys]
prefix = "Ctrl+b"

[keys.global]
help_open = "F9"

[keys.library.prefix]
search_open = "s"
"#;
    std::fs::write(config_path(), original).unwrap();
    let text = std::fs::read_to_string(config_path()).unwrap();
    let cfg = parse_config(&text).unwrap();
    save_config_section(&cfg, ConfigSection::Keys).unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();

    // The unrelated section and its untouched key survive the patch.
    assert!(saved.contains("[server]"));
    assert!(saved.contains("legacy_note = \"keep me\""));

    // The whole `[keys]` shape survives, entry for entry.
    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.keybinds, cfg.keybinds);
    assert_eq!(
        reparsed.keybinds.prefix,
        Some(mbv_keybinds::Chord::parse("Ctrl+b").unwrap())
    );
    assert_eq!(
        reparsed.keybinds.router_override("help_open"),
        Some(mbv_keybinds::Chord::parse("F9").unwrap())
    );
    assert_eq!(
        reparsed.keybinds.prefix_assignment("search_open"),
        Some(mbv_keybinds::Chord::parse("s").unwrap())
    );
    assert_eq!(reparsed.server_url, "http://localhost:8096");
}

#[test]
fn keys_default_configuration_prunes_the_keys_table() {
    let _guard = crate::TestStateDirGuard::new();
    std::fs::write(
        config_path(),
        "[keys]\nprefix = \"Ctrl+b\"\n\n[keys.global]\nhelp_open = \"F9\"\n",
    )
    .unwrap();
    let cfg = Config {
        server_url: "http://localhost:8096".into(),
        ..Default::default()
    };
    save_config_section(&cfg, ConfigSection::Keys).unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(
        !saved.contains("[keys"),
        "pruned table must be gone: {saved}"
    );
    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.keybinds, mbv_keybinds::Keybinds::default());
}

#[test]
fn keys_saved_section_names_are_lowercase_file_shape() {
    let _guard = crate::TestStateDirGuard::new();
    let cfg = Config {
        keybinds: mbv_keybinds::load(&mbv_keybinds::RawKeybinds {
            prefix: Some("Ctrl+b".into()),
            sections: vec![(
                "GLOBAL".into(),
                mbv_keybinds::RawSection {
                    router: vec![("help_open".into(), "F9".into())],
                    prefix: vec![],
                },
            )],
        })
        .unwrap(),
        ..Default::default()
    };
    save_config_section(&cfg, ConfigSection::Keys).unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(saved.contains("[keys.global]"), "lowercase shape: {saved}");
    // And the saved spelling reads back through the parser.
    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.keybinds, cfg.keybinds);
}

/// D8's contract: a settings save never rewrites `[keys]`. A running Client
/// that loaded its bindings before another Client's accepted edit must not
/// put back its own copy at its next settings save, so the hand-written
/// table survives byte for byte.
#[test]
fn settings_save_leaves_a_hand_written_keys_table_byte_for_byte() {
    let _guard = crate::TestStateDirGuard::new();
    let hand_written = "[keys]\nprefix = \"Ctrl+b\"\n\n[keys.global]\nhelp_open = \"F9\"\n";
    std::fs::write(
        config_path(),
        format!("[server]\nurl = \"http://localhost:8096\"\n\n{hand_written}"),
    )
    .unwrap();
    let cfg = parse_config(&std::fs::read_to_string(config_path()).unwrap()).unwrap();
    save_config_settings(&cfg).unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();

    let start = saved
        .find("[keys")
        .unwrap_or_else(|| panic!("settings save must keep the table: {saved}"));
    // The table runs to the next header that is not one of its own
    // sub-tables (`[keys.global]`); keep the block's own trailing newline
    // and compare it byte for byte.
    let region = &saved[start..];
    let mut region_end = region.len();
    for (index, _) in region.match_indices("\n\n[") {
        if !region[index + 3..].starts_with("keys") {
            region_end = index + 1;
            break;
        }
    }
    assert_eq!(&region[..region_end], hand_written);
}

/// The other half of D8: the Keys section save writes the in-memory
/// bindings over whatever the file holds.
#[test]
fn keys_section_save_writes_the_in_memory_bindings() {
    let _guard = crate::TestStateDirGuard::new();
    std::fs::write(
        config_path(),
        "[keys]\nprefix = \"Ctrl+b\"\n\n[keys.global]\nhelp_open = \"F9\"\n",
    )
    .unwrap();
    let cfg = Config {
        keybinds: mbv_keybinds::load(&mbv_keybinds::RawKeybinds {
            prefix: Some("Ctrl+g".into()),
            sections: vec![(
                "session".into(),
                mbv_keybinds::RawSection {
                    router: vec![("quit".into(), "Q".into())],
                    prefix: vec![],
                },
            )],
        })
        .unwrap(),
        ..Default::default()
    };
    save_config_section(&cfg, ConfigSection::Keys).unwrap();

    let reparsed = parse_config(&std::fs::read_to_string(config_path()).unwrap()).unwrap();
    assert_eq!(reparsed.keybinds, cfg.keybinds);
    assert_eq!(
        reparsed.keybinds.prefix,
        Some(mbv_keybinds::Chord::parse("Ctrl+g").unwrap())
    );
    assert_eq!(
        reparsed.keybinds.router_override("quit"),
        Some(mbv_keybinds::Chord::parse("Q").unwrap())
    );
    assert_eq!(reparsed.keybinds.router_override("help_open"), None);
}
