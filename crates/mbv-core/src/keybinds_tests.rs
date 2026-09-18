use super::*;

// ── Task 1.1: declaration ───────────────────────────────────────────────

/// Mirrors `SETTING_SECTIONS`' names and order (design D4) plus `Global`.
const EXPECTED_SECTIONS: &[&str] = &[
    "Services",
    "Playback",
    "Display",
    "Session",
    "Library",
    "Queue",
    "Mpv",
    "Feeds",
    "Actions",
    "Global",
];

/// The router-owned global policy entries (today's `KEY_POLICY` names with
/// `global: true`, minus the blocking `alt_swallow`).
const GLOBAL_ACTION_IDS: &[&str] = &[
    "settings_open",
    "sessions_open",
    "playlists_open",
    "search_open",
    "help_open",
    "quit",
    "next_library_tab",
    "previous_library_tab",
    "panel_mode_cycle_x",
    "clear_queue_prompt_c",
    "visualizer",
    "ctrl_l_force_clear",
    "f5_refresh",
    "panel_right",
    "panel_left",
    "alt_previous_library_tab",
    "alt_next_library_tab",
    "library_tab_jump",
];

/// The split transport actions (the `playback` bucket, task 5.1).
const TRANSPORT_ACTION_IDS: &[&str] = &[
    "toggle_play_pause",
    "stop",
    "seek_back",
    "seek_forward",
    "next_track",
    "previous_track",
    "volume_down",
    "volume_up",
    "toggle_mute",
    "toggle_mute_or_cycle_audio",
    "cycle_subtitle",
    "open_idle_feed_link",
];

/// Policy entries that are not configurable actions: the blocking swallow
/// guard and the non-global, non-transport entries, which keep their literal
/// matches.
const EXCLUDED_POLICY_NAMES: &[&str] =
    &["alt_swallow", "queue_column_width", "sessions_sidebar_escape"];

#[test]
fn action_ids_are_unique() {
    let mut ids: Vec<&str> = KEYBIND_ACTIONS.iter().map(|action| action.id).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique);
}

#[test]
fn key_sections_mirror_setting_sections_order_plus_global() {
    let names: Vec<&str> = KEY_SECTIONS.iter().map(|section| section.name()).collect();
    assert_eq!(names, EXPECTED_SECTIONS);
    for name in EXPECTED_SECTIONS {
        assert_eq!(
            KeySection::from_name(name),
            Some(
                KEY_SECTIONS[EXPECTED_SECTIONS
                    .iter()
                    .position(|n| n == name)
                    .expect("listed")]
            ),
            "from_name round-trips `{name}`"
        );
    }
    assert_eq!(KeySection::from_name("bogus"), None);
}

#[test]
fn every_declared_section_is_a_key_section() {
    for action in KEYBIND_ACTIONS {
        assert!(
            KEY_SECTIONS.contains(&action.section),
            "action `{}` has section `{}` outside the vocabulary",
            action.id,
            action.section.name()
        );
        assert_eq!(KeySection::from_name(action.section.name()), Some(action.section));
    }
}

#[test]
fn rebindable_includes_router_globals_and_transport_actions() {
    for id in GLOBAL_ACTION_IDS.iter().chain(TRANSPORT_ACTION_IDS) {
        let action = action_by_id(id)
            .unwrap_or_else(|| panic!("action `{id}` missing from the declaration"));
        assert!(action.rebindable, "action `{id}` must be rebindable");
    }
}

#[test]
fn transport_actions_carry_the_playback_gate_and_policy() {
    for id in TRANSPORT_ACTION_IDS {
        let action = action_by_id(id).expect("listed transport action");
        assert_eq!(action.gate, KeyGate::Playback);
        assert_eq!(action.policy, "playback");
    }
}

#[test]
fn every_declared_action_is_rebindable_and_exclusions_stay_out() {
    for action in KEYBIND_ACTIONS {
        assert!(action.rebindable, "action `{}` must be rebindable", action.id);
        assert!(!action.default_chords.is_empty());
        assert!(!action.id.is_empty());
        assert!(!action.policy.is_empty());
    }
    for name in EXCLUDED_POLICY_NAMES {
        assert!(
            action_by_id(name).is_none(),
            "`{name}` must stay outside the declaration (literal match)"
        );
    }
}

#[test]
fn no_two_actions_share_a_default_chord() {
    // Pure default-vs-default pairs are exempt from load-time router
    // collision checks, so the declaration itself must not contain one.
    for (index, action) in KEYBIND_ACTIONS.iter().enumerate() {
        for other in KEYBIND_ACTIONS.iter().skip(index + 1) {
            for chord in action.parsed_default_chords() {
                assert!(
                    !other.parsed_default_chords().contains(&chord),
                    "actions `{}` and `{}` share default chord `{chord}`",
                    action.id,
                    other.id
                );
            }
        }
    }
}

#[test]
fn only_escape_defaulted_actions_are_not_prefix_addressable() {
    for action in KEYBIND_ACTIONS {
        let escape_default = action
            .parsed_default_chords()
            .iter()
            .any(|chord| chord.key == Key::Esc && chord.mods.is_empty());
        assert_eq!(
            action.prefix_addressable,
            !escape_default,
            "action `{}` prefix-addressability must follow its Esc default",
            action.id
        );
    }
}

// ── Task 1.2: parser and defaults ───────────────────────────────────────

#[test]
fn parser_accepts_canonical_and_reordered_forms() {
    let cases: &[(&str, Chord)] = &[
        ("Ctrl+b", Chord { mods: KeyMods::CTRL, key: Key::Char('b') }),
        ("F8", Chord { mods: KeyMods::NONE, key: Key::F(8) }),
        ("Shift+Left", Chord { mods: KeyMods::SHIFT, key: Key::Left }),
        ("Shift+Ctrl+b", Chord { mods: KeyMods::CTRL.union(KeyMods::SHIFT), key: Key::Char('b') }),
        ("Alt+Ctrl+Shift+x", Chord { mods: KeyMods::CTRL.union(KeyMods::SHIFT).union(KeyMods::ALT), key: Key::Char('x') }),
        ("control+alt+del", Chord { mods: KeyMods::CTRL.union(KeyMods::ALT), key: Key::Delete }),
        ("Escape", Chord { mods: KeyMods::NONE, key: Key::Esc }),
        ("BackTab", Chord { mods: KeyMods::NONE, key: Key::BackTab }),
        ("Space", Chord { mods: KeyMods::NONE, key: Key::Char(' ') }),
        ("+", Chord { mods: KeyMods::NONE, key: Key::Char('+') }),
        ("-", Chord { mods: KeyMods::NONE, key: Key::Char('-') }),
        ("<", Chord { mods: KeyMods::NONE, key: Key::Char('<') }),
        ("Ctrl+=", Chord { mods: KeyMods::CTRL, key: Key::Char('=') }),
        ("q", Chord { mods: KeyMods::NONE, key: Key::Char('q') }),
        ("N", Chord { mods: KeyMods::NONE, key: Key::Char('N') }),
    ];
    for (input, expected) in cases {
        assert_eq!(&Chord::parse(input), &Ok(*expected), "parsing `{input}`");
    }
    // Modifier order is not significant.
    assert_eq!(Chord::parse("Shift+Ctrl+b"), Chord::parse("Ctrl+Shift+b"));
    assert_eq!(Chord::parse("Shift+Alt+Ctrl+x"), Chord::parse("Ctrl+Shift+Alt+x"));
}

#[test]
fn parser_rejects_every_malformed_form() {
    let rejected: &[&str] = &[
        "",
        "   ",
        "Ctrl+",
        "Ctrl++",
        "Ctrl+Shift+",
        "Hyper+x",
        "Meta+b",
        "F0",
        "F13",
        "Ctrl+Foo",
        "Foo",
        "Ctrl+Space+Ctrl",
    ];
    for input in rejected {
        assert!(Chord::parse(input).is_err(), "`{input}` must not parse");
    }
}

#[test]
fn display_round_trips_every_declared_chord() {
    for action in KEYBIND_ACTIONS {
        for text in action.default_chords {
            let chord = Chord::parse(text).expect("declared default must parse");
            assert_eq!(
                Chord::parse(&chord.to_string()),
                Ok(chord),
                "canonical rendering of `{text}` must round-trip"
            );
        }
    }
}

/// Today's hard-coded chords per declared action: the `KeyPolicyBinding`
/// literals, the `1-9` tab jump, and the `playback_command_for_key` table.
const EXPECTED_DEFAULTS: &[(&str, &[&str])] = &[
    ("help_open", &["F1"]),
    ("settings_open", &["F2"]),
    ("quit", &["q"]),
    ("sessions_open", &["F3"]),
    ("search_open", &["Ctrl+/", "Ctrl+_"]),
    ("next_library_tab", &["Tab"]),
    ("previous_library_tab", &["BackTab"]),
    ("library_tab_jump", &["1", "2", "3", "4", "5", "6", "7", "8", "9"]),
    ("f5_refresh", &["F5"]),
    ("alt_next_library_tab", &["Alt+Down"]),
    ("alt_previous_library_tab", &["Alt+Up"]),
    ("playlists_open", &["F4"]),
    ("clear_queue_prompt_c", &["c"]),
    ("visualizer", &["v"]),
    ("toggle_play_pause", &["Space"]),
    ("stop", &["Esc"]),
    ("seek_back", &["<"]),
    ("seek_forward", &[">"]),
    ("next_track", &["Shift+N"]),
    ("previous_track", &["Shift+P"]),
    ("volume_down", &["-"]),
    ("volume_up", &["+", "="]),
    ("toggle_mute", &["m"]),
    ("toggle_mute_or_cycle_audio", &["a"]),
    ("cycle_subtitle", &["z"]),
    ("open_idle_feed_link", &["o"]),
    ("panel_mode_cycle_x", &["x"]),
    ("panel_right", &["Right"]),
    ("panel_left", &["Left"]),
    ("ctrl_l_force_clear", &["Ctrl+l"]),
];

#[test]
fn defaults_reproduce_todays_hard_coded_chords() {
    let defaults = Keybinds::defaults();
    assert!(defaults.prefix.is_none());
    for (id, expected) in EXPECTED_DEFAULTS {
        let action = action_by_id(id).expect("listed action");
        let resolved: Vec<Chord> = defaults.router_chords(action);
        let expected: Vec<Chord> = expected
            .iter()
            .map(|text| Chord::parse(text).expect("expected chord must parse"))
            .collect();
        assert_eq!(resolved, expected, "default chords for `{id}`");
    }
    // The declared table carries exactly the expected set, no more.
    for action in KEYBIND_ACTIONS {
        assert!(
            EXPECTED_DEFAULTS.iter().any(|(id, _)| *id == action.id),
            "action `{}` missing from the expected-defaults table",
            action.id
        );
    }
}

#[test]
fn partial_override_patches_only_the_configured_action() {
    let mut keybinds = Keybinds::defaults();
    keybinds.sections.push((
        KeySection::Global,
        SectionBindings {
            router: vec![("help_open", Chord::parse("F9").unwrap())],
            prefix: Vec::new(),
        },
    ));
    let rebound = action_by_id("help_open").unwrap();
    assert_eq!(keybinds.router_chords(rebound), vec![Chord::parse("F9").unwrap()]);
    let untouched = action_by_id("settings_open").unwrap();
    assert_eq!(keybinds.router_chord(untouched), Chord::parse("F2").unwrap());
}

// ── Task 1.3: load-time validation ──────────────────────────────────────

fn raw_router(
    section: &str,
    entries: &[(&str, &str)],
) -> (String, RawSection) {
    (
        section.to_string(),
        RawSection {
            router: entries
                .iter()
                .map(|(id, chord)| (id.to_string(), chord.to_string()))
                .collect(),
            prefix: Vec::new(),
        },
    )
}

fn raw_prefix(
    section: &str,
    entries: &[(&str, &str)],
) -> (String, RawSection) {
    (
        section.to_string(),
        RawSection {
            router: Vec::new(),
            prefix: entries
                .iter()
                .map(|(id, chord)| (id.to_string(), chord.to_string()))
                .collect(),
        },
    )
}

#[test]
fn load_accepts_a_valid_partially_overridden_config() {
    let raw = RawKeybinds {
        prefix: Some("Ctrl+b".to_string()),
        sections: vec![
            raw_router("global", &[("help_open", "F9")]),
            (
                "library".to_string(),
                RawSection {
                    router: vec![("previous_library_tab".to_string(), "Shift+Tab".to_string())],
                    prefix: vec![("next_library_tab".to_string(), "n".to_string())],
                },
            ),
            raw_prefix("playback", &[("toggle_play_pause", "p")]),
        ],
    };
    let keybinds = load(&raw).expect("valid config must load");
    assert_eq!(keybinds.prefix, Some(Chord::parse("Ctrl+b").unwrap()));
    assert_eq!(
        keybinds.router_override("help_open"),
        Some(Chord::parse("F9").unwrap())
    );
    assert_eq!(
        keybinds.router_override("previous_library_tab"),
        Some(Chord::parse("Shift+Tab").unwrap())
    );
    assert_eq!(
        keybinds.prefix_assignment("next_library_tab"),
        Some(Chord::parse("n").unwrap())
    );
    assert_eq!(
        keybinds.prefix_assignment("toggle_play_pause"),
        Some(Chord::parse("p").unwrap())
    );
    // Unconfigured actions fall back to their defaults.
    assert_eq!(keybinds.router_override("settings_open"), None);
    assert_eq!(
        keybinds.router_chord(action_by_id("settings_open").unwrap()),
        Chord::parse("F2").unwrap()
    );
}

#[test]
fn load_accepts_an_absent_keys_section_and_a_redundant_default_override() {
    assert_eq!(load(&RawKeybinds::default()), Ok(Keybinds::defaults()));

    // An override equal to the declared default is not a collision.
    let raw = RawKeybinds {
        prefix: None,
        sections: vec![raw_router("global", &[("help_open", "F1")])],
    };
    assert!(load(&raw).is_ok());
}

/// Each rejection class, with the expected error naming the offending entry
/// (both entries for the collision classes).
#[test]
fn load_rejects_every_validation_class() {
    let cases: Vec<(&str, RawKeybinds, KeybindsError, &[&str])> = vec![
        (
            "reserved chord as prefix",
            RawKeybinds {
                prefix: Some("Ctrl+q".to_string()),
                sections: Vec::new(),
            },
            KeybindsError::ReservedChord {
                chord: "Ctrl+q".to_string(),
                entry: "keys.prefix".to_string(),
            },
            &["Ctrl+q", "reserved", "keys.prefix"],
        ),
        (
            "reserved chord as router override",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_router("global", &[("help_open", "Ctrl+q")])],
            },
            KeybindsError::ReservedChord {
                chord: "Ctrl+q".to_string(),
                entry: "keys.global.help_open".to_string(),
            },
            &["Ctrl+q", "reserved", "keys.global.help_open"],
        ),
        (
            "reserved chord as prefix-namespace assignment",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_prefix("library", &[("next_library_tab", "Ctrl+q")])],
            },
            KeybindsError::ReservedChord {
                chord: "Ctrl+q".to_string(),
                entry: "keys.library.prefix.next_library_tab".to_string(),
            },
            &["Ctrl+q", "reserved", "keys.library.prefix.next_library_tab"],
        ),
        (
            "unparseable chord",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_router("global", &[("help_open", "F13")])],
            },
            KeybindsError::UnparseableChord {
                chord: "F13".to_string(),
                entry: "keys.global.help_open".to_string(),
                reason: Chord::parse("F13").unwrap_err().to_string(),
            },
            &["F13", "keys.global.help_open", "unparseable"],
        ),
        (
            "unknown action id",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_router("global", &[("help_opn", "F9")])],
            },
            KeybindsError::UnknownAction {
                action: "help_opn".to_string(),
                section: "global".to_string(),
            },
            &["help_opn"],
        ),
        (
            "unknown section name",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_router("bogus", &[("help_open", "F9")])],
            },
            KeybindsError::UnknownSection {
                section: "bogus".to_string(),
            },
            &["bogus"],
        ),
        (
            "case-variant duplicate section names",
            RawKeybinds {
                prefix: None,
                sections: vec![
                    raw_router("Library", &[("previous_library_tab", "Shift+Tab")]),
                    raw_router("library", &[("next_library_tab", "n")]),
                ],
            },
            KeybindsError::DuplicateSection {
                first: "Library".to_string(),
                second: "library".to_string(),
            },
            &["Library", "library"],
        ),
        (
            "section mismatch",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_router("library", &[("help_open", "F9")])],
            },
            KeybindsError::SectionMismatch {
                action: "help_open".to_string(),
                section: "library".to_string(),
                declared: KeySection::Global,
            },
            &["help_open", "library", "Global"],
        ),
        (
            "non-prefix-addressable action in a prefix table",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_prefix("playback", &[("stop", "s")])],
            },
            KeybindsError::NotPrefixAddressable {
                action: "stop".to_string(),
                section: "playback".to_string(),
            },
            &["stop", "prefix-addressable"],
        ),
        (
            "prefix collides with a declared default",
            RawKeybinds {
                prefix: Some("F1".to_string()),
                sections: Vec::new(),
            },
            KeybindsError::PrefixCollision {
                chord: "F1".to_string(),
                entry: "keys.Global.help_open".to_string(),
            },
            &["F1", "keys.Global.help_open"],
        ),
        (
            "prefix collides with a configured override",
            RawKeybinds {
                prefix: Some("F9".to_string()),
                sections: vec![raw_router("global", &[("help_open", "F9")])],
            },
            KeybindsError::PrefixCollision {
                chord: "F9".to_string(),
                entry: "keys.Global.help_open".to_string(),
            },
            &["F9", "keys.Global.help_open"],
        ),
        (
            "prefix collides with a prefix-namespace assignment",
            RawKeybinds {
                prefix: Some("n".to_string()),
                sections: vec![raw_prefix("library", &[("next_library_tab", "n")])],
            },
            KeybindsError::PrefixCollision {
                chord: "n".to_string(),
                entry: "keys.Library.prefix.next_library_tab".to_string(),
            },
            &["n", "keys.Library.prefix.next_library_tab"],
        ),
        (
            "two configured router-scope actions collide",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_router(
                    "Global",
                    &[("help_open", "F2"), ("settings_open", "F2")],
                )],
            },
            KeybindsError::RouterCollision {
                chord: "F2".to_string(),
                first: "help_open".to_string(),
                second: "settings_open".to_string(),
            },
            &["F2", "help_open", "settings_open"],
        ),
        (
            "configured router chord collides with another action's default",
            RawKeybinds {
                prefix: None,
                sections: vec![raw_router("global", &[("help_open", "q")])],
            },
            KeybindsError::RouterCollision {
                chord: "q".to_string(),
                first: "help_open".to_string(),
                second: "quit".to_string(),
            },
            &["q", "help_open", "quit"],
        ),
        (
            "two prefix-namespace actions collide",
            RawKeybinds {
                prefix: Some("Ctrl+b".to_string()),
                sections: vec![
                    raw_prefix("library", &[("next_library_tab", "n")]),
                    raw_prefix("playback", &[("toggle_play_pause", "n")]),
                ],
            },
            KeybindsError::PrefixNamespaceCollision {
                chord: "n".to_string(),
                first: "next_library_tab".to_string(),
                second: "toggle_play_pause".to_string(),
            },
            &["n", "next_library_tab", "toggle_play_pause"],
        ),
    ];

    for (name, raw, expected, message_parts) in cases {
        let result = load(&raw);
        assert_eq!(result.as_ref(), Err(&expected), "case `{name}`");
        let message = result.unwrap_err().to_string();
        for part in message_parts {
            assert!(
                message.contains(part),
                "case `{name}`: message `{message}` must name `{part}`"
            );
        }
    }
}

#[test]
fn reserved_chord_list_is_the_documented_set() {
    assert_eq!(RESERVED_CHORDS, &["Ctrl+q"]);
    // Every reserved entry parses and is rejected wherever a chord is
    // configured (covered per-position above); defaults never use one.
    for text in RESERVED_CHORDS {
        let chord = Chord::parse(text).expect("reserved chord must parse");
        for action in KEYBIND_ACTIONS {
            assert!(
                !action.parsed_default_chords().contains(&chord),
                "action `{}` must not default to reserved chord `{text}`",
                action.id
            );
        }
    }
}
