//! The Keys editor's compiled-model builders (`add-gui-settings-screen`
//! task 1.3, design D6): the builders are pure, and `load` on the raw form
//! is the only rule set that accepts or refuses an edit.

use mbv_keybinds::{Chord, Keybinds, RawSection, SectionBindings, action_by_id, load};

/// A configuration with a prefix, a router override, and a prefix-namespace
/// assignment, in different sections.
fn sample_keybinds() -> Keybinds {
    Keybinds {
        prefix: Some(Chord::parse("Ctrl+b").unwrap()),
        sections: vec![
            (
                mbv_keybinds::KeySection::Global,
                SectionBindings {
                    router: vec![("help_open", Chord::parse("F9").unwrap())],
                    prefix: Vec::new(),
                },
            ),
            (
                mbv_keybinds::KeySection::Library,
                SectionBindings {
                    router: Vec::new(),
                    prefix: vec![("next_library_tab", Chord::parse("n").unwrap())],
                },
            ),
        ],
    }
}

#[test]
fn to_raw_loads_back_to_the_same_bindings() {
    let keybinds = sample_keybinds();
    assert_eq!(
        load(&keybinds.to_raw()).expect("the raw form of a loaded configuration must load"),
        keybinds
    );
}

#[test]
fn to_raw_of_an_edited_configuration_loads_back_equal() {
    let edited = sample_keybinds()
        .with_router(action_by_id("quit").unwrap(), Chord::parse("Q").unwrap())
        .with_prefix_assignment(
            action_by_id("toggle_play_pause").unwrap(),
            Some(Chord::parse("p").unwrap()),
        )
        .with_prefix(Some(Chord::parse("Ctrl+g").unwrap()));
    assert_eq!(
        load(&edited.to_raw()).expect("the edited raw form must load"),
        edited
    );
}

#[test]
fn a_with_router_collision_fails_load_naming_both_actions() {
    // `settings_open` defaults to F2; binding `help_open` to F2 collides
    // with that default. The builders check nothing, and `load` refuses.
    let candidate = Keybinds::defaults().with_router(
        action_by_id("help_open").unwrap(),
        Chord::parse("F2").unwrap(),
    );

    let error = load(&candidate.to_raw()).expect_err("a collision must fail the load");

    assert_eq!(error.kind_name(), "keybinds.router_collision");
    let message = error.to_string();
    assert!(
        message.contains("help_open") && message.contains("settings_open"),
        "the refusal must name both actions: {message}"
    );
}

#[test]
fn with_router_to_an_actions_only_default_removes_the_override() {
    // `help_open` has exactly one default chord, F1. An edit that sets that
    // chord is a reset, not an override that would survive as stale config.
    let overridden = Keybinds::defaults().with_router(
        action_by_id("help_open").unwrap(),
        Chord::parse("F9").unwrap(),
    );
    assert_eq!(
        overridden.router_override("help_open"),
        Some(Chord::parse("F9").unwrap())
    );

    let reset = overridden.with_router(
        action_by_id("help_open").unwrap(),
        Chord::parse("F1").unwrap(),
    );

    assert_eq!(reset.router_override("help_open"), None);
    assert_eq!(
        load(&reset.to_raw()).expect("the reset raw form must load"),
        reset
    );
}

#[test]
fn without_router_removes_the_override_and_keeps_the_prefix_assignment() {
    let keybinds = sample_keybinds().without_router(action_by_id("help_open").unwrap());
    assert_eq!(keybinds.router_override("help_open"), None);
    assert_eq!(
        keybinds.prefix_assignment("next_library_tab"),
        Some(Chord::parse("n").unwrap())
    );
    assert_eq!(
        load(&keybinds.to_raw()).expect("the raw form must load"),
        keybinds
    );
}

#[test]
fn with_prefix_assignment_none_removes_the_assignment() {
    let keybinds =
        sample_keybinds().with_prefix_assignment(action_by_id("next_library_tab").unwrap(), None);
    assert_eq!(keybinds.prefix_assignment("next_library_tab"), None);
    assert_eq!(
        load(&keybinds.to_raw()).expect("the raw form must load"),
        keybinds
    );
}

#[test]
fn with_prefix_sets_and_clears_the_prefix_chord() {
    let set = Keybinds::defaults().with_prefix(Some(Chord::parse("Ctrl+b").unwrap()));
    assert_eq!(set.prefix, Some(Chord::parse("Ctrl+b").unwrap()));
    assert_eq!(load(&set.to_raw()).expect("the raw form must load"), set);

    let cleared = set.with_prefix(None);
    assert_eq!(cleared.prefix, None);
    assert_eq!(
        load(&cleared.to_raw()).expect("the raw form must load"),
        cleared
    );
}

/// An all-defaults configuration round-trips to an empty raw form: the load
/// of that raw form yields the defaults again.
#[test]
fn to_raw_of_the_defaults_loads_back_to_the_defaults() {
    let defaults = Keybinds::defaults();
    let raw = defaults.to_raw();
    assert_eq!(raw.prefix, None);
    assert_eq!(raw.sections, Vec::<(String, RawSection)>::new());
    assert_eq!(load(&raw).expect("the raw form must load"), defaults);
}
