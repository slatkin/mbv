use super::super::super::palette;
use super::super::super::HELP_PANEL_W;
use super::chrome;
use crate::app::{PanelFocus, TabSelection};
use mbv_core::keybinds::{action_by_id, KeyGate, KeySection, Keybinds, KEYBIND_ACTIONS};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The named help sections. `Audiobookshelf` is its own destination section so
/// its key set can never be presented as Emby or Feeds behavior (design §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpSection {
    Global,
    Playback,
    Queue,
    Home,
    EmbyLibrary,
    Audiobookshelf,
    Feeds,
}

/// The active help context: the focused panel plus the selected destination.
/// Queue panel focus classifies as Queue first while the selected browse
/// destination is retained below it (spec "User opens help while the queue has
/// focus"). With library focus the selected destination is matched
/// exhaustively — there is no default-to-Emby branch.
//
// `pub(in crate::app)` so the Interactive Component (`src/app/components/help.rs`)
// can receive a destination from the shell and pass it into `render_help_panel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum HelpDestination {
    Queue,
    Home,
    EmbyLibrary,
    Audiobookshelf,
    Feeds,
}

/// Compute the help destination from panel focus and selected tab.
///
/// Extracted from `impl App::help_destination` so the shell can call it
/// without `App` access in the Interactive Component path (design D5: the
/// shell computes the presentation model and writes it into the component
/// via `get_component_mut`+downcast).
pub(in crate::app) fn help_destination(
    panel_focus: PanelFocus,
    tab: TabSelection,
) -> HelpDestination {
    if matches!(panel_focus, PanelFocus::Queue) {
        return HelpDestination::Queue;
    }
    match tab {
        TabSelection::Home => HelpDestination::Home,
        TabSelection::EmbyLibrary(_) => HelpDestination::EmbyLibrary,
        TabSelection::AudiobookshelfLibrary(_) => HelpDestination::Audiobookshelf,
        TabSelection::Feeds => HelpDestination::Feeds,
    }
}

/// The ordered help sections for a destination: the matched section first,
/// then Global/Playback/Queue and the other destination sections retained
/// below it. Retained sections must not be presented as the matched
/// destination's own behavior.
fn help_section_order(dest: HelpDestination) -> Vec<HelpSection> {
    let matched = match dest {
        HelpDestination::Queue => HelpSection::Queue,
        HelpDestination::Home => HelpSection::Home,
        HelpDestination::EmbyLibrary => HelpSection::EmbyLibrary,
        HelpDestination::Audiobookshelf => HelpSection::Audiobookshelf,
        HelpDestination::Feeds => HelpSection::Feeds,
    };
    let canonical = [
        HelpSection::Global,
        HelpSection::Playback,
        HelpSection::Queue,
        HelpSection::Home,
        HelpSection::EmbyLibrary,
        HelpSection::Audiobookshelf,
        HelpSection::Feeds,
    ];
    std::iter::once(matched)
        .chain(
            canonical
                .into_iter()
                .filter(move |section| *section != matched),
        )
        .collect()
}

impl HelpSection {
    fn index(self) -> usize {
        match self {
            Self::Global => 0,
            Self::Playback => 1,
            Self::Queue => 2,
            Self::Home => 3,
            Self::EmbyLibrary => 4,
            Self::Audiobookshelf => 5,
            Self::Feeds => 6,
        }
    }
}

/// One-line help label per declared Playback action. Only the labels live
/// here; the row set and the key chords come from the declared registry and
/// the loaded `Keybinds` (design D7), so help cannot drift from routing.
fn playback_label(action_id: &str) -> &'static str {
    match action_id {
        "toggle_play_pause" => "Pause/Resume",
        "stop" => "Stop",
        "seek_back" => "Seek back 5 seconds",
        "seek_forward" => "Seek forward 5 seconds",
        "next_track" => "Next track",
        "previous_track" => "Previous track",
        "volume_down" => "Volume down",
        "volume_up" => "Volume up",
        "toggle_mute" => "Mute",
        "toggle_mute_or_cycle_audio" => "Cycle audio track",
        "cycle_subtitle" => "Cycle subtitles",
        "open_idle_feed_link" => "Open idle feed link",
        other => panic!("declared Playback action `{other}` has no help label"),
    }
}

fn help_line(key_w: usize, key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw(""),
        Span::styled(
            format!("{:<kw$}", key, kw = key_w),
            Style::default()
                .fg(palette::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            desc.to_owned(),
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
    ])
}

fn help_section_line(label: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::raw(""),
        Span::styled(
            label.to_owned(),
            Style::default()
                .fg(palette::TEXT_METADATA)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn help_blank() -> Line<'static> {
    Line::from("")
}

/// The key-column text for a declared action under the loaded
/// configuration: its configured/declared chords joined with ` / `, or the
/// positional set's range (`1 – 9`) when the declaration carries more than
/// two chords (the tab-jump digits). The registry and the loaded `Keybinds`
/// are the only sources — the hand-written Global chord strings are gone
/// (design D7), so a rebind is reflected here without a second table.
fn action_keys(keybinds: &Keybinds, id: &str) -> String {
    let action = action_by_id(id).expect("help references a declared action");
    let chords: Vec<String> = keybinds
        .router_chords(action)
        .iter()
        .map(|chord| chord.to_string())
        .collect();
    if chords.len() > 2 {
        format!("{} – {}", chords[0], chords[chords.len() - 1])
    } else {
        chords.join(" / ")
    }
}

/// The Global section's configurable rows as (action id, label), in the
/// order they are presented. Only labels live here; the chords come from
/// the registry and the loaded `Keybinds`.
const GLOBAL_CONFIGURABLE_ROWS: &[(&str, &str)] = &[
    ("help_open", "Help"),
    ("settings_open", "Settings"),
    ("sessions_open", "Remote sessions"),
    ("playlists_open", "Playlists"),
    ("f5_refresh", "Refresh view"),
    ("visualizer", "Switch queue artwork / visualizer"),
    ("next_library_tab", "Cycle menu"),
    ("library_tab_jump", "Jump to tab"),
    ("clear_queue_prompt_c", "Clear Queue"),
    ("quit", "Quit"),
];

/// Builds every named help section. Kept as a pure function so classification
/// tests can inspect section content and ordering without driving a terminal.
fn build_help_sections(
    key_w: usize,
    keybinds: &Keybinds,
) -> Vec<(HelpSection, Vec<Line<'static>>)> {
    let mut sec_global = vec![help_section_line("Global")];
    for (id, label) in GLOBAL_CONFIGURABLE_ROWS {
        sec_global.push(help_line(key_w, &action_keys(keybinds, id), label));
    }
    // The panel-focus pair is two declared actions presented as one row.
    sec_global.push(help_line(
        key_w,
        &format!(
            "{} / {}",
            action_keys(keybinds, "panel_left"),
            action_keys(keybinds, "panel_right")
        ),
        "Switch panels",
    ));
    // Leaf-local, non-configurable rows stay hand-written: the registry
    // declares only router-owned chords (design D7).
    sec_global.push(help_line(key_w, "↑ / ↓", "Move cursor"));
    sec_global.push(help_line(key_w, "PgUp / PgDn", "Page scroll"));
    sec_global.push(help_line(key_w, "Home / End", "First/last item"));
    sec_global.push(help_line(key_w, "Enter", "Select/Play/Open"));
    sec_global.push(help_line(key_w, ".", "Context menu"));
    // The prefix namespace is presented when configured (spec: help lists
    // the prefix and its assigned actions); with no prefix, help matches
    // the pre-prefix presentation.
    if let Some(prefix) = keybinds.prefix {
        sec_global.push(help_section_line("Prefix"));
        sec_global.push(help_line(key_w, &prefix.to_string(), "Arm prefix mode"));
        for (action, chord) in keybinds.prefix_assignments() {
            sec_global.push(help_line(key_w, &chord.to_string(), action.id));
        }
    }
    sec_global.push(help_blank());
    // Rendered from the declared registry (design D7): one row per Playback
    // action, its keys the chords it actually fires on for the loaded
    // configuration, so the section cannot silently drift from routing.
    let mut sec_playback = vec![help_section_line("Playback")];
    // The transport bucket: the Playback-section actions gated `Playback`
    // (design D2). `visualizer` shares the section but is presented in the
    // Global rows above, where it lived before the split.
    for action in KEYBIND_ACTIONS
        .iter()
        .filter(|action| action.section == KeySection::Playback && action.gate == KeyGate::Playback)
    {
        let keys = keybinds
            .router_chords(action)
            .iter()
            .map(|chord| chord.to_string())
            .collect::<Vec<_>>()
            .join(" / ");
        sec_playback.push(help_line(key_w, &keys, playback_label(action.id)));
    }
    sec_playback.push(help_blank());
    let sec_queue = vec![
        help_section_line("Queue"),
        help_line(key_w, "p", "Jump to playing item"),
        help_line(key_w, "i", "Go to item in library"),
        help_line(key_w, "Del", "Remove from Queue"),
        help_line(key_w, "Ctrl+Z", "Undo removal"),
        help_line(
            key_w,
            "x",
            "Cycle panel layout (both / queue / library; queue / library under 80 cols)",
        ),
        help_line(key_w, "Shift+← / →", "Resize queue column"),
        help_line(key_w, "Ctrl+S", "Save playlist"),
        help_blank(),
    ];
    let sec_home = vec![
        help_section_line("Home"),
        help_line(key_w, "[ / ]", "Switch sections"),
        help_line(key_w, "Ctrl+W", "Toggle watched"),
        help_line(key_w, "Ctrl+A", "Add to Queue"),
        help_blank(),
    ];
    let sec_library = vec![
        help_section_line("Library"),
        help_line(key_w, "Esc / Backspace", "Go back"),
        help_line(key_w, "/", "Search library"),
        help_line(key_w, "Ctrl+W", "Toggle watched"),
        help_line(key_w, "Ctrl+S", "Shuffle"),
        help_line(key_w, "Ctrl+P", "Play all"),
        help_line(key_w, "Ctrl+A", "Add to Queue"),
        help_line(key_w, "r", "Refresh library"),
        help_line(key_w, "Ctrl+R", "Rescan library"),
        help_blank(),
    ];
    // The Audiobookshelf section advertises only Audiobookshelf keys: show
    // navigation, episode selection, and the book tab's book/chapter
    // navigation (spec "Help and context actions reflect the selected
    // destination"). Episode-mode Enter/Space is explicitly inert until #518
    // applies playback support, so no Emby play/enqueue/search/watched/
    // shuffle/rescan/route/context action is listed here.
    let sec_audiobookshelf = vec![
        help_section_line("Audiobookshelf"),
        help_line(key_w, "Up / Down or k / j", "Move show rows"),
        help_line(key_w, "Left / Right or h / l", "Adjacent shows"),
        help_line(key_w, "PageUp / PageDown", "Page through shows"),
        help_line(key_w, "Home / End", "First/last show"),
        help_line(key_w, "Enter / Space", "Enter episode selection"),
        help_line(key_w, "Esc / Backspace", "Return to show selection"),
        help_line(key_w, "[ / ]", "Cycle played-state filter"),
        help_line(
            key_w,
            "Enter / Space (in episode)",
            "Play episode (inert until #518)",
        ),
        help_line(key_w, "Left / Right", "Focus chapters or books"),
        help_line(key_w, "Up / Down or k / j", "Move focused rows"),
        help_line(key_w, "[ / ]", "Switch author bucket"),
        help_line(key_w, "Space", "Play selected book"),
        help_line(key_w, "Ctrl+A", "Add selected book to queue"),
        help_line(key_w, "Enter (in chapters)", "Seek to chapter start"),
        help_blank(),
    ];
    let sec_feeds = vec![
        help_section_line("Feeds"),
        help_line(key_w, "w", "Cycle watched filter"),
        help_line(key_w, "r", "Refresh feeds"),
        help_line(key_w, "[ / ]", "Switch subscription"),
        help_line(key_w, "Enter", "Play entry"),
        help_line(key_w, "e", "Enqueue entry"),
        help_blank(),
    ];
    vec![
        (HelpSection::Global, sec_global),
        (HelpSection::Playback, sec_playback),
        (HelpSection::Queue, sec_queue),
        (HelpSection::Home, sec_home),
        (HelpSection::EmbyLibrary, sec_library),
        (HelpSection::Audiobookshelf, sec_audiobookshelf),
        (HelpSection::Feeds, sec_feeds),
    ]
}

/// Render the help sidebar panel.
///
/// Extracted from `impl App::render_help_panel` so the Interactive Component
/// (`src/app/components/help.rs`) can call it in `view()` without `App` access
/// (design D9: a component's `view()` calls the existing render substrate).
/// The shell-owned scroll offset and destination are passed in; the function
/// clamps scroll to the visible content and mutates the caller's `scroll`.
//
// `pub(in crate::app)` so the Interactive Component can call it.
pub(in crate::app) struct HelpRenderGeometry {
    pub max_scroll: u16,
}

/// The rendered Playback section's row texts (padded key column + label),
/// one per declared Playback action, in registry order. Test seam for the
/// render-layer tests that pin help to the registry (design D7).
#[cfg(test)]
pub(in crate::app::render) fn playback_help_rows(key_w: usize, keybinds: &Keybinds) -> Vec<String> {
    section_help_rows(key_w, keybinds, HelpSection::Playback)
}

/// The rendered Global section's row texts (padded key column + label),
/// including the prefix block when configured. Test seam for the
/// render-layer tests that pin the Global/prefix presentation to the
/// registry (task 7.4, design D7).
#[cfg(test)]
pub(in crate::app::render) fn global_help_rows(key_w: usize, keybinds: &Keybinds) -> Vec<String> {
    section_help_rows(key_w, keybinds, HelpSection::Global)
}

#[cfg(test)]
fn section_help_rows(key_w: usize, keybinds: &Keybinds, section: HelpSection) -> Vec<String> {
    let (_, lines) = build_help_sections(key_w, keybinds)
        .into_iter()
        .find(|(name, _)| *name == section)
        .expect("help section exists");
    // Skip the section header; the rows follow it.
    lines[1..]
        .iter()
        .filter(|line| !line.spans.is_empty())
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref() as &str)
                .collect::<String>()
        })
        .filter(|text| !text.trim().is_empty())
        .collect()
}

pub(in crate::app) fn render_help_panel(
    f: &mut Frame,
    area: Option<ratatui::layout::Rect>,
    scroll: &mut u16,
    dest: HelpDestination,
    keybinds: &Keybinds,
) -> HelpRenderGeometry {
    let content = match area {
        Some(area) => chrome::render_panel_shell_at(
            f,
            area,
            "KEYBOARD SHORTCUTS",
            "[↑↓]scroll [Esc]close",
            true,
        ),
        None => chrome::render_panel_shell(
            f,
            f.area(),
            HELP_PANEL_W,
            "KEYBOARD SHORTCUTS",
            "[↑↓]scroll [Esc]close",
        ),
    };
    let key_w = 16usize;

    let mut sections: [Option<Vec<Line<'static>>>; 7] = std::array::from_fn(|_| None);
    for (name, lines) in build_help_sections(key_w, keybinds) {
        sections[name.index()] = Some(lines);
    }
    let order = help_section_order(dest);
    let mut lines: Vec<Line> = Vec::new();
    for section in order {
        if let Some(section_lines) = sections[section.index()].take() {
            lines.extend(section_lines);
        }
    }
    lines.push(help_blank());

    let total = lines.len();
    let visible = content.height as usize;
    let max_scroll = total.saturating_sub(visible) as u16;
    *scroll = (*scroll).min(max_scroll);
    f.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), content);
    chrome::render_sidebar_scrollbar(f, content, total, *scroll as usize);
    HelpRenderGeometry { max_scroll }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flatten a section's styled lines to plain text so tests can assert
    /// ordering and exact key strings without comparing full terminal buffers.
    fn lines_to_text(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn audiobookshelf_destination_classifies_as_its_own_section() {
        assert_eq!(
            help_destination(PanelFocus::Library, TabSelection::AudiobookshelfLibrary(0)),
            HelpDestination::Audiobookshelf
        );
        // Not classified as an Emby library.
        assert_ne!(
            help_destination(PanelFocus::Library, TabSelection::AudiobookshelfLibrary(0)),
            HelpDestination::EmbyLibrary
        );
    }

    #[test]
    fn audiobookshelf_help_lists_spec_key_sets_first() {
        let sections = build_help_sections(16, &Keybinds::default());
        let order = help_section_order(HelpDestination::Audiobookshelf);
        assert_eq!(order[0], HelpSection::Audiobookshelf);

        let (_, abs) = sections
            .iter()
            .find(|(name, _)| *name == HelpSection::Audiobookshelf)
            .unwrap();
        let text = lines_to_text(abs);
        for needle in [
            "[ / ]",
            "Up / Down or k / j",
            "Left / Right or h / l",
            "PageUp / PageDown",
            "Home / End",
            "Enter episode selection",
            "Esc / Backspace",
            "inert until #518",
        ] {
            assert!(
                text.iter().any(|line| line.contains(needle)),
                "Audiobookshelf section must list {needle:?}, got {text:?}"
            );
        }
        // No Emby-only action may be advertised as Audiobookshelf behavior.
        for emby_only in ["Shuffle", "Rescan", "Search library"] {
            assert!(
                !text.iter().any(|line| line.contains(emby_only)),
                "Audiobookshelf section must not advertise Emby action {emby_only:?}: {text:?}"
            );
        }
    }

    #[test]
    fn home_help_lists_section_switch_watched_and_enqueue() {
        let sections = build_help_sections(16, &Keybinds::default());
        let order = help_section_order(HelpDestination::Home);
        assert_eq!(order[0], HelpSection::Home);

        let (_, home) = sections
            .iter()
            .find(|(name, _)| *name == HelpSection::Home)
            .unwrap();
        let text = lines_to_text(home);
        assert!(text.iter().any(|line| line.contains("[ / ]")));
        assert!(text.iter().any(|line| line.contains("Ctrl+W")));
        assert!(text.iter().any(|line| line.contains("Ctrl+A")));
        // Section switching is `[` / `]`, not the removed Alt+↑/↓ binding.
        assert!(!text.iter().any(|line| line.contains("Alt")));
    }

    #[test]
    fn queue_focus_puts_queue_first_and_retains_browse_destination() {
        // Queue focus over a retained Audiobookshelf destination.
        let dest = help_destination(PanelFocus::Queue, TabSelection::AudiobookshelfLibrary(0));
        assert_eq!(dest, HelpDestination::Queue);
        let order = help_section_order(HelpDestination::Queue);
        assert_eq!(order[0], HelpSection::Queue);
        assert!(
            order.contains(&HelpSection::Audiobookshelf),
            "queue help must retain the selected Audiobookshelf destination below Queue: {order:?}"
        );
    }
}
