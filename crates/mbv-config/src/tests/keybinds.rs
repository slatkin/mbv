#[cfg(test)]
use crate::{Config, config_path, parse_config, save_config_settings};

// ── `[keys]` config save round-trips (change add-configurable-keybinds, U2) ──
//
// Save goes through the read-patch-write path (`save_config_settings_at`).
// The `[keys]` parse and rejection tests own the crate's public contract
// and live in the `tests/config` integration binary.

#[test]
fn keys_round_trip_preserves_unrelated_sections_and_keys() {
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
    save_config_settings(&cfg).unwrap();
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
    save_config_settings(&cfg).unwrap();
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
    save_config_settings(&cfg).unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(saved.contains("[keys.global]"), "lowercase shape: {saved}");
    // And the saved spelling reads back through the parser.
    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.keybinds, cfg.keybinds);
}
