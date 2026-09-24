//! Prefix-mode arming and armed dispatch through the single routing site.
/// Task 6.2 (design D6): the prefix-mode state machine — every arm of
/// the two policy layers plus the App-owned armed bit, resolved through
/// `resolve_router_outcome_with_focused` (the single routing site).
mod prefix_mode {
    use super::super::test_support::*;
    use super::super::*;
    use crate::app::input::router::RouterOutcome;
    use crossterm::event::KeyEvent;

    fn prefix_keybinds(assignments: &[(&str, &str)]) -> Keybinds {
        let mut sections: Vec<(String, mbv_core::keybinds::RawSection)> = Vec::new();
        for (id, chord) in assignments {
            let section = action_by_id(id)
                .expect("declared action")
                .section
                .name()
                .to_ascii_lowercase();
            let index = sections
                .iter()
                .position(|(name, _)| *name == section)
                .unwrap_or_else(|| {
                    sections.push((section.clone(), mbv_core::keybinds::RawSection::default()));
                    sections.len() - 1
                });
            sections[index]
                .1
                .prefix
                .push(((*id).into(), (*chord).into()));
        }
        mbv_core::keybinds::load(&mbv_core::keybinds::RawKeybinds {
            prefix: Some("Ctrl+b".into()),
            sections,
        })
        .expect("valid prefix configuration")
    }

    fn armed_snapshot() -> RouterSnapshot {
        RouterSnapshot {
            panel_mode: PanelMode::Both,
            prefix_armed: true,
            ..RouterSnapshot::default()
        }
    }

    fn resolve(key: KeyEvent, snapshot: &RouterSnapshot, keybinds: &Keybinds) -> RouterOutcome {
        crate::app::input::router::resolve_router_outcome_with_focused(
            key, snapshot, None, keybinds,
        )
    }

    #[test]
    fn prefix_arm_is_the_top_layer_and_arms_through_the_router() {
        let keybinds = prefix_keybinds(&[]);
        let key = chord(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert_eq!(
            resolve_policy(key, &snapshot(), &keybinds).unwrap().name,
            "prefix_arm"
        );
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
                &snapshot(),
                &keybinds
            ),
            RouterOutcome::PrefixArm
        );
    }

    #[test]
    fn without_a_configured_prefix_nothing_arms() {
        let keybinds = Keybinds::default();
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char('b'), KeyModifiers::CONTROL),
                &snapshot(),
                &keybinds
            )
            .map(|entry| entry.name),
            None,
            "Ctrl+b matches no layer without a configured prefix"
        );
    }

    #[test]
    fn arming_is_suppressed_while_a_text_entry_owns_focus() {
        let keybinds = prefix_keybinds(&[]);
        let mut typing = snapshot();
        typing.text_entry_focused = true;
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
                &typing,
                &keybinds
            ),
            RouterOutcome::FallThrough,
            "the prefix chord reaches the text entry as an ordinary chord"
        );
    }

    #[test]
    fn arming_is_suppressed_under_a_blocking_overlay() {
        let keybinds = prefix_keybinds(&[]);
        let mut blocked = snapshot();
        blocked.blocking_overlay_open = true;
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
                &blocked,
                &keybinds
            ),
            RouterOutcome::Swallow,
            "the chord follows the overlay routing rules"
        );
        // When the focused leaf is the blocking overlay itself, its own
        // request stands.
        let focused =
            crate::app::components::ComponentId::Modal(crate::app::components::ModalId::Confirm);
        assert_eq!(
            crate::app::input::router::resolve_router_outcome_with_focused(
                crossterm::event::KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
                &blocked,
                Some(&focused),
                &keybinds
            ),
            RouterOutcome::FallThrough
        );
    }

    #[test]
    fn armed_mapped_gate_open_dispatches() {
        // next_library_tab assigned `n` in the prefix namespace: ungated,
        // so it fires while armed and disarms.
        let keybinds = prefix_keybinds(&[("next_library_tab", "n")]);
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &armed_snapshot(),
                &keybinds
            ),
            RouterOutcome::PrefixDispatch(Command::NextLibraryTab)
        );
    }

    #[test]
    fn armed_mapped_gate_closed_is_swallowed_as_unmapped() {
        // toggle_play_pause assigned `p`: Playback-gated, so with no
        // active player and no remote session the mapped chord is
        // treated as unmapped — swallowed, disarmed, never fired.
        let keybinds = prefix_keybinds(&[("toggle_play_pause", "p")]);
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
                &armed_snapshot(),
                &keybinds
            ),
            RouterOutcome::PrefixSwallow
        );
        let mut active = armed_snapshot();
        active.player_active = true;
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
                &active,
                &keybinds
            ),
            RouterOutcome::PrefixDispatch(Command::TogglePlayPause),
            "the same chord fires once the action's gate opens"
        );
    }

    #[test]
    fn armed_idle_feed_link_gate_reads_the_router_chord() {
        // open_idle_feed_link assigned `f` in the prefix namespace: the
        // chord-sensitive IdleFeedLink gate tests the action's configured
        // router chord (`o`), not the pressed prefix chord, so the
        // mapping fires when the link conditions hold (R6 fix).
        let keybinds = prefix_keybinds(&[("open_idle_feed_link", "f")]);
        let mut link = armed_snapshot();
        link.idle_feed_link_available = true;
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &link,
                &keybinds
            ),
            RouterOutcome::PrefixDispatch(Command::OpenIdleFeedLink)
        );
        // The gate's non-chord conditions still close it.
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &armed_snapshot(),
                &keybinds
            ),
            RouterOutcome::PrefixSwallow,
            "an unavailable link swallows and disarms"
        );
    }

    #[test]
    fn armed_unmapped_chord_swallows_and_disarms() {
        let keybinds = prefix_keybinds(&[]);
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                &armed_snapshot(),
                &keybinds
            ),
            RouterOutcome::PrefixSwallow
        );
    }

    #[test]
    fn armed_escape_does_not_reach_stop() {
        // Esc is the stop default and the player is active, but armed
        // dispatch never consults router-scope chords and `stop` is not
        // prefix-addressable (design D1): Esc swallows and disarms.
        let keybinds = prefix_keybinds(&[]);
        let mut armed = armed_snapshot();
        armed.player_active = true;
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &armed,
                &keybinds
            ),
            RouterOutcome::PrefixSwallow
        );
    }

    #[test]
    fn armed_f_keys_are_captured() {
        // F1 (help_open) is mapped in the router scope only; while armed
        // it resolves only against the prefix namespace — captured,
        // disarming, help never opens.
        let keybinds = prefix_keybinds(&[]);
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
                &armed_snapshot(),
                &keybinds
            ),
            RouterOutcome::PrefixSwallow
        );
    }

    #[test]
    fn double_prefix_re_arms() {
        let keybinds = prefix_keybinds(&[]);
        assert_eq!(
            resolve(
                crossterm::event::KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
                &armed_snapshot(),
                &keybinds
            ),
            RouterOutcome::PrefixArm,
            "the prefix chord while armed re-arms and stays consumed"
        );
    }

    #[test]
    fn no_chord_falls_through_while_armed() {
        let keybinds = prefix_keybinds(&[("next_library_tab", "n")]);
        for code in [
            KeyCode::Char('k'),
            KeyCode::Char('n'),
            KeyCode::Esc,
            KeyCode::F(1),
            KeyCode::Tab,
            KeyCode::Down,
        ] {
            let outcome = resolve(
                crossterm::event::KeyEvent::new(code, KeyModifiers::NONE),
                &armed_snapshot(),
                &keybinds,
            );
            assert!(
                matches!(
                    outcome,
                    RouterOutcome::PrefixArm
                        | RouterOutcome::PrefixDispatch(_)
                        | RouterOutcome::PrefixSwallow
                ),
                "{code:?} must resolve inside the armed machine, got {outcome:?}"
            );
        }
    }
}
