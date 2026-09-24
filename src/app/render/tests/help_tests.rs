use super::test_helpers::buffer_to_string;
use crate::app::palette;
use crate::app::render::components::help::{
    global_help_rows, help_destination, playback_help_rows, render_help_panel, HelpDestination,
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
fn help_none_fallback_paints_the_fullscreen_shell() {
    let width = 40;
    let height = 12;
    let mut scroll = 0;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = Some(render_help_panel(
                f,
                None,
                &mut scroll,
                HelpDestination::EmbyLibrary,
                &Keybinds::default(),
            ));
        })
        .unwrap();
    let geometry = geometry.expect("help renderer returns geometry");
    let buffer = terminal.backend().buffer();
    assert_eq!(geometry.panel_area, Rect::new(0, 0, width, height));
    assert_eq!(
        buffer[(0, 0)].bg,
        palette::surface_colors(palette::Surface::SidebarBody, false).fill
    );
    assert_eq!(
        buffer[(2, 1)].bg,
        palette::surface_colors(palette::Surface::SidebarBand, false).fill
    );
    assert_eq!(buffer[(3, 1)].symbol(), "K");
    assert_eq!(buffer[(width - 1, 2)].symbol(), " ");
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
    let keybinds = Keybinds::default();
    let key_w = key_width(&keybinds);
    let rows = playback_help_rows(&keybinds);
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
        let key_column = key_column(row, key_w);
        assert_eq!(key_column, expected_keys, "row for `{}`", action.id);
        assert!(
            !label_column(row, key_w).trim().is_empty(),
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
    let key_w = key_width(&keybinds);
    let rows = playback_help_rows(&keybinds);
    let toggle_row = rows
        .iter()
        .find(|row| key_column(row, key_w) == "k")
        .expect("rebound toggle_play_pause renders its configured chord");
    assert!(
        label_column(toggle_row, key_w).contains("Pause/Resume"),
        "the rebound row keeps the action's label: {toggle_row:?}"
    );
    assert!(
        !rows.iter().any(|row| key_column(row, key_w) == "Space"),
        "the declared default Space must disappear from the rendered section"
    );
}

// ── Task 7.4: the Global section and the prefix list render from the registry ──

/// The chord column's width for these bindings: the painter's own rule, so a
/// rebind cannot desynchronise the test from the rendered padding.
fn key_width(keybinds: &Keybinds) -> usize {
    super::components::help::help_key_width(keybinds)
}

/// The row's key column. Sub-headers inside a section (e.g. the prefix
/// block's `Prefix` line) are shorter than the padded key width, so the
/// slice is guarded.
fn key_column(row: &str, key_w: usize) -> &str {
    let end = row.len().min(key_w);
    row[..end].trim()
}

fn label_column(row: &str, key_w: usize) -> &str {
    if row.len() > key_w {
        &row[key_w..]
    } else {
        ""
    }
}

/// With defaults, the Global section's configurable rows carry the declared
/// default chords: the registry is the only chord source.
#[test]
fn global_rows_render_the_registry_defaults() {
    let keybinds = Keybinds::default();
    let key_w = key_width(&keybinds);
    let rows = global_help_rows(&keybinds);
    for (chord, label) in [
        ("F1", "Help"),
        ("F2", "Settings"),
        ("F3", "Remote sessions"),
        ("F4", "Playlists"),
        ("F5", "Refresh view"),
        ("v", "Switch queue artwork / visualizer"),
        ("Tab", "Cycle menu"),
        ("1 – 9", "Jump to tab"),
        ("c", "Clear Queue"),
        ("q", "Quit"),
        ("Ctrl+Left / Ctrl+Right", "Switch panels"),
    ] {
        let row = rows
            .iter()
            .find(|row| key_column(row, key_w) == chord)
            .unwrap_or_else(|| panic!("no Global row for chord {chord:?} in {rows:?}"));
        assert!(
            label_column(row, key_w).contains(label),
            "row for {chord:?} must carry the {label:?} label: {row:?}"
        );
    }
    // No prefix block without a configured prefix.
    assert!(!rows
        .iter()
        .any(|row| label_column(row, key_w).contains("Arm prefix mode")));
}

/// After a rebind, the Global section shows the configured chord and the
/// declared default disappears.
#[test]
fn global_rows_follow_an_override() {
    let keybinds = Keybinds {
        prefix: None,
        sections: vec![(
            KeySection::Library,
            SectionBindings {
                router: vec![("next_library_tab", Chord::parse("k").unwrap())],
                prefix: vec![],
            },
        )],
    };
    let key_w = key_width(&keybinds);
    let rows = global_help_rows(&keybinds);
    let rebound = rows
        .iter()
        .find(|row| key_column(row, key_w) == "k")
        .expect("the rebound next_library_tab renders its configured chord");
    assert!(
        label_column(rebound, key_w).contains("Cycle menu"),
        "the rebound row keeps the action's label: {rebound:?}"
    );
    assert!(
        !rows.iter().any(|row| key_column(row, key_w) == "Tab"),
        "the declared default Tab must disappear from the rendered section"
    );
}

/// With a prefix configured, help lists the prefix chord and its assigned
/// actions (spec: help reflects the prefix namespace).
#[test]
fn help_lists_the_prefix_and_its_assignments() {
    let keybinds = Keybinds {
        prefix: Some(Chord::parse("Ctrl+k").unwrap()),
        sections: vec![(
            KeySection::Playback,
            SectionBindings {
                router: vec![],
                prefix: vec![("next_track", Chord::parse("n").unwrap())],
            },
        )],
    };
    let key_w = key_width(&keybinds);
    let rows = global_help_rows(&keybinds);
    let arm = rows
        .iter()
        .find(|row| key_column(row, key_w) == "Ctrl+k")
        .expect("the configured prefix chord is listed");
    assert!(
        label_column(arm, key_w).contains("Arm prefix mode"),
        "{arm:?}"
    );
    let assigned = rows
        .iter()
        .find(|row| {
            key_column(row, key_w) == "n" && label_column(row, key_w).contains("Next track")
        })
        .expect("the prefix-namespace assignment is listed");
    assert!(
        label_column(assigned, key_w).contains("Next track"),
        "{assigned:?}"
    );
}
