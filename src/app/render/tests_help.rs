use super::test_helpers::buffer_to_string;
use crate::app::render::components::help::{
    help_destination, playback_help_rows, render_help_panel, HelpDestination,
};
use mbv_core::keybinds::{Chord, KeyGate, KeySection, Keybinds, SectionBindings, KEYBIND_ACTIONS};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_help(width: u16, height: u16, scroll: u16) -> String {
    let mut scroll = scroll;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_help_panel(
                f,
                Some(Rect::new(0, 0, width, height)),
                &mut scroll,
                HelpDestination::EmbyLibrary,
                &Keybinds::default(),
            );
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn help_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, scroll) in [(60, 20, 0), (60, 20, 2), (18, 8, 0), (32, 12, 1)] {
        let output = render_help(width, height, scroll);
        assert!(
            output.contains("KEYBOARD"),
            "help shell missing: {output:?}"
        );
    }
}

#[test]
fn help_destination_queue_focus_returns_queue() {
    use crate::app::{PanelFocus, TabSelection};
    assert_eq!(
        help_destination(PanelFocus::Queue, TabSelection::Home),
        HelpDestination::Queue
    );
}

// ── Task 5.3: the Playback section renders from the declared registry ──

/// The rendered Playback rows under the default configuration: one row per
/// declared Playback action, in registry order, each showing that action's
/// default chords and a label.
#[test]
fn playback_rows_render_the_registry_defaults() {
    let rows = playback_help_rows(16, &Keybinds::default());
    // One row per declared Playback action, no header, in registry order.
    let declared: Vec<&str> = KEYBIND_ACTIONS
        .iter()
        .filter(|action| action.section == KeySection::Playback && action.gate == KeyGate::Playback)
        .map(|action| action.id)
        .collect();
    assert_eq!(rows.len(), declared.len());

    for (row, action) in rows.iter().zip(KEYBIND_ACTIONS.iter().filter(|action| {
        action.section == KeySection::Playback && action.gate == KeyGate::Playback
    })) {
        let expected_keys = action
            .parsed_default_chords()
            .iter()
            .map(|chord| chord.to_string())
            .collect::<Vec<_>>()
            .join(" / ");
        let key_column = row[..16].trim();
        assert_eq!(key_column, expected_keys, "row for `{}`", action.id);
        assert!(
            !row[16..].trim().is_empty(),
            "row for `{}` must carry a label",
            action.id
        );
    }
}

/// With an override, the rendered keys follow the configured chord and the
/// declared defaults disappear from the section.
#[test]
fn playback_rows_follow_an_override() {
    let keybinds = Keybinds {
        prefix: None,
        sections: vec![(
            KeySection::Playback,
            SectionBindings {
                router: vec![(
                    "toggle_play_pause",
                    Chord::parse("k").expect("test chord must parse"),
                )],
                prefix: vec![],
            },
        )],
    };
    let rows = playback_help_rows(16, &keybinds);
    let toggle_row = rows
        .iter()
        .find(|row| row[..16].trim() == "k")
        .expect("rebound toggle_play_pause renders its configured chord");
    assert!(
        toggle_row[16..].contains("Pause/Resume"),
        "the rebound row keeps the action's label: {toggle_row:?}"
    );
    assert!(
        !rows.iter().any(|row| row[..16].trim() == "Space"),
        "the declared default Space must disappear from the rendered section"
    );
}
