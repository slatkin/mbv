use mbv_config::parse_config;

// ── `[keys]` config parse (change add-configurable-keybinds, U2) ──────────
//
// Parse routes every entry through the registry validator
// (`mbv_keybinds::load`), so the tests here pin the surface through the
// existing config error path (`parse_config` -> `Err(String)`).

#[test]
fn keys_section_absent_yields_default_keybinds() {
    let cfg = parse_config("[server]\nurl = \"http://localhost:8096/\"\n").unwrap();
    assert_eq!(cfg.keybinds, mbv_keybinds::Keybinds::default());
    assert!(cfg.keybinds.prefix.is_none());
    assert!(cfg.keybinds.router_override("help_open").is_none());
    // Absent section falls back to the declared default chords.
    let help = mbv_keybinds::action_by_id("help_open").unwrap();
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
        Some(mbv_keybinds::Chord::parse("F9").unwrap())
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
        Some(mbv_keybinds::Chord::parse("Ctrl+b").unwrap())
    );
    assert_eq!(
        cfg.keybinds.router_override("next_library_tab"),
        Some(mbv_keybinds::Chord::parse("T").unwrap())
    );
    assert_eq!(
        cfg.keybinds.router_override("help_open"),
        Some(mbv_keybinds::Chord::parse("F9").unwrap())
    );
    assert_eq!(
        cfg.keybinds.prefix_assignment("search_open"),
        Some(mbv_keybinds::Chord::parse("s").unwrap())
    );
    // A rebound action no longer fires its declared default chord.
    let next_tab = mbv_keybinds::action_by_id("next_library_tab").unwrap();
    assert!(
        !cfg.keybinds
            .router_chords(next_tab)
            .contains(&mbv_keybinds::Chord::parse("Tab").unwrap())
    );
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
        Some(mbv_keybinds::Chord::parse("F9").unwrap())
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
    let volume_up = mbv_keybinds::action_by_id("volume_up").unwrap();
    assert_eq!(
        cfg.keybinds.router_chords(volume_up),
        vec![mbv_keybinds::Chord::parse("w").unwrap()]
    );
}

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
        assert!(err.is_parse());
        assert!(
            err.to_string().contains(expected),
            "expected {expected:?} in error: {err}"
        );
    }
}
