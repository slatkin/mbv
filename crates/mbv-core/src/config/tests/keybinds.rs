#[cfg(test)]
use crate::config::{config_path, parse_config, save_config_settings, Config};

// ── `[keys]` config parse + save (change add-configurable-keybinds, U2) ──
//
// Parse routes every entry through the registry validator
// (`crate::keybinds::load`), so the rejection tests here pin the surface
// through the existing config error path (`parse_config` -> `Err(String)`)
// while `keybinds_tests.rs` owns the validator's own unit coverage. Save
// goes through the read-patch-write path (`save_config_settings_at`).

#[test]
fn keys_section_absent_yields_default_keybinds() {
    let cfg = parse_config("[server]\nurl = \"http://localhost:8096/\"\n").unwrap();
    assert_eq!(cfg.keybinds, crate::keybinds::Keybinds::default());
    assert!(cfg.keybinds.prefix.is_none());
    assert!(cfg.keybinds.router_override("help_open").is_none());
    // Absent section falls back to the declared default chords.
    let help = crate::keybinds::action_by_id("help_open").unwrap();
    assert_eq!(
        cfg.keybinds.router_chords(help),
        help.parsed_default_chords()
    );
}

#[test]
fn keys_partial_override_patches_only_that_action() {
    let toml = r#"
[keys.global]
help_open = "F9"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(
        cfg.keybinds.router_override("help_open"),
        Some(crate::keybinds::Chord::parse("F9").unwrap())
    );
    // Untouched actions keep no override: they resolve to their defaults.
    assert!(cfg.keybinds.router_override("settings_open").is_none());
    assert!(cfg.keybinds.prefix.is_none());
}

#[test]
fn keys_per_section_tables_and_prefix_namespace_parse() {
    let toml = r#"
[keys]
prefix = "ctrl+b"

[keys.library]
next_library_tab = "T"

[keys.library.prefix]
search_open = "s"

[keys.global]
help_open = "F9"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(
        cfg.keybinds.prefix,
        Some(crate::keybinds::Chord::parse("Ctrl+b").unwrap())
    );
    assert_eq!(
        cfg.keybinds.router_override("next_library_tab"),
        Some(crate::keybinds::Chord::parse("T").unwrap())
    );
    assert_eq!(
        cfg.keybinds.router_override("help_open"),
        Some(crate::keybinds::Chord::parse("F9").unwrap())
    );
    assert_eq!(
        cfg.keybinds.prefix_assignment("search_open"),
        Some(crate::keybinds::Chord::parse("s").unwrap())
    );
    // A rebound action no longer fires its declared default chord.
    let next_tab = crate::keybinds::action_by_id("next_library_tab").unwrap();
    assert!(!cfg
        .keybinds
        .router_chords(next_tab)
        .contains(&crate::keybinds::Chord::parse("Tab").unwrap()));
}

#[test]
fn keys_section_names_match_case_insensitively() {
    let toml = r#"
[keys.Global]
help_open = "F9"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(
        cfg.keybinds.router_override("help_open"),
        Some(crate::keybinds::Chord::parse("F9").unwrap())
    );
}

#[test]
fn keys_alias_default_is_replaced_not_extended() {
    // volume_up declares ["+", "="]; one configured chord replaces both.
    let toml = r#"
[keys.playback]
volume_up = "w"
"#;
    let cfg = parse_config(toml).unwrap();
    let volume_up = crate::keybinds::action_by_id("volume_up").unwrap();
    assert_eq!(
        cfg.keybinds.router_chords(volume_up),
        vec![crate::keybinds::Chord::parse("w").unwrap()]
    );
}

#[test]
fn keys_round_trip_preserves_unrelated_sections_and_keys() {
    let _guard = crate::config::TestStateDirGuard::new();
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
        Some(crate::keybinds::Chord::parse("Ctrl+b").unwrap())
    );
    assert_eq!(
        reparsed.keybinds.router_override("help_open"),
        Some(crate::keybinds::Chord::parse("F9").unwrap())
    );
    assert_eq!(
        reparsed.keybinds.prefix_assignment("search_open"),
        Some(crate::keybinds::Chord::parse("s").unwrap())
    );
    assert_eq!(reparsed.server_url, "http://localhost:8096");
}

#[test]
fn keys_default_configuration_prunes_the_keys_table() {
    let _guard = crate::config::TestStateDirGuard::new();
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
    assert_eq!(reparsed.keybinds, crate::keybinds::Keybinds::default());
}

#[test]
fn keys_saved_section_names_are_lowercase_file_shape() {
    let _guard = crate::config::TestStateDirGuard::new();
    let cfg = Config {
        keybinds: crate::keybinds::load(&crate::keybinds::RawKeybinds {
            prefix: Some("Ctrl+b".into()),
            sections: vec![(
                "GLOBAL".into(),
                crate::keybinds::RawSection {
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

#[cfg(test)]
mod keys_rejections {
    use super::parse_config;
    use rstest::rstest;

    /// Each rejection surfaces through the existing config error path with
    /// the registry's message naming the offending entry (both entries for
    /// the collision classes).
    #[rstest]
    #[case::unknown_section("[keys.bogus]\nhelp_open = \"F9\"\n", "unknown section `keys.bogus`")]
    #[case::case_variant_duplicate_section(
        "[keys.Library]\nprevious_library_tab = \"Shift+Tab\"\n\n[keys.library]\nnext_library_tab = \"n\"\n",
        "same section under different spellings"
    )]
    #[case::unknown_action(
        "[keys.global]\nnot_an_action = \"F9\"\n",
        "unknown action `not_an_action`"
    )]
    #[case::section_mismatch(
        "[keys.queue]\nhelp_open = \"F9\"\n",
        "declared in section `Global` but configured under `keys.queue`"
    )]
    #[case::unparseable_chord(
        "[keys.global]\nhelp_open = \"Ctrl+Nope\"\n",
        "unparseable chord `Ctrl+Nope`"
    )]
    #[case::reserved_chord(
        "[keys.session]\nquit = \"Ctrl+q\"\n",
        "chord `Ctrl+q` in `keys.session.quit` is reserved"
    )]
    #[case::prefix_is_reserved(
        "[keys]\nprefix = \"Ctrl+q\"\n",
        "chord `Ctrl+q` in `keys.prefix` is reserved"
    )]
    #[case::prefix_collides_with_default("[keys]\nprefix = \"F1\"\n", "prefix chord `F1` collides")]
    #[case::prefix_collides_with_configured(
        "[keys]\nprefix = \"F9\"\n\n[keys.global]\nhelp_open = \"F9\"\n",
        "prefix chord `F9` collides"
    )]
    #[case::router_scope_collision(
        "[keys.global]\nhelp_open = \"F9\"\nsettings_open = \"F9\"\n",
        "`help_open` and `settings_open` share router-scope chord `F9`"
    )]
    #[case::prefix_namespace_collision(
        "[keys.library.prefix]\nsearch_open = \"s\"\nnext_library_tab = \"s\"\n",
        "share prefix-namespace chord `s`"
    )]
    #[case::not_prefix_addressable(
        "[keys.playback.prefix]\nstop = \"x\"\n",
        "not prefix-addressable"
    )]
    #[case::prefix_entry_not_a_string("[keys]\nprefix = 7\n", "keys.prefix must be a string chord")]
    #[case::section_entry_not_a_table(
        "[keys]\nhelp_open = \"F9\"\n",
        "keys.help_open must be a table"
    )]
    fn keys_rejection_surfaces(#[case] toml: &str, #[case] expected: &str) {
        let err = parse_config(toml).unwrap_err();
        assert!(
            err.contains(expected),
            "expected {expected:?} in error: {err}"
        );
    }
}
