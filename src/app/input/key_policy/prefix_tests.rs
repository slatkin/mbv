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
        let mut sections: Vec<(String, mbv_keybinds::RawSection)> = Vec::new();
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
                    sections.push((section.clone(), mbv_keybinds::RawSection::default()));
                    sections.len() - 1
                });
            sections[index]
                .1
                .prefix
                .push(((*id).into(), (*chord).into()));
        }
        mbv_keybinds::load(&mbv_keybinds::RawKeybinds {
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
    fn arming_is_suppressed_under_a_blocking_overlay() {
        let keybinds = prefix_keybinds(&[]);
        let mut blocked = snapshot();
        blocked.overlays.focus = OverlayFocus::Blocking;
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
    fn armed_idle_feed_link_gate_reads_the_router_chord() {
        // open_idle_feed_link assigned `f` in the prefix namespace: the
        // chord-sensitive IdleFeedLink gate tests the action's configured
        // router chord (`o`), not the pressed prefix chord, so the
        // mapping fires when the link conditions hold (R6 fix).
        let keybinds = prefix_keybinds(&[("open_idle_feed_link", "f")]);
        let mut link = armed_snapshot();
        link.playback.idle_feed_link_available = true;
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
}
