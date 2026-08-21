use super::super::super::action::PLAYBACK_HELP_BINDINGS;
use super::super::super::palette;
use super::super::super::App;
use super::super::super::HELP_PANEL_W;
use crate::app::{PanelFocus, TabSelection};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpDestination {
    Queue,
    Home,
    EmbyLibrary,
    Audiobookshelf,
    Feeds,
}

impl App {
    fn help_destination(&self) -> HelpDestination {
        if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
            return HelpDestination::Queue;
        }
        match self.tab {
            TabSelection::Home => HelpDestination::Home,
            TabSelection::EmbyLibrary(_) => HelpDestination::EmbyLibrary,
            TabSelection::AudiobookshelfLibrary(_) => HelpDestination::Audiobookshelf,
            TabSelection::Feeds => HelpDestination::Feeds,
        }
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

fn help_line(key_w: usize, key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw(""),
        Span::styled(
            format!("{:<kw$}", key, kw = key_w),
            Style::default()
                .fg(palette::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_owned(), Style::default().fg(palette::SUBTLE)),
    ])
}

fn help_section_line(label: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::raw(""),
        Span::styled(
            label.to_owned(),
            Style::default()
                .fg(palette::FOAM)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn help_blank() -> Line<'static> {
    Line::from("")
}

/// Builds every named help section. Kept as a pure function so classification
/// tests can inspect section content and ordering without driving a terminal.
fn build_help_sections(key_w: usize) -> Vec<(HelpSection, Vec<Line<'static>>)> {
    let sec_global = vec![
        help_section_line("Global"),
        help_line(key_w, "F1", "Help"),
        help_line(key_w, "F2", "Settings"),
        help_line(key_w, "F3", "Remote sessions"),
        help_line(key_w, "F4", "Playlists"),
        help_line(key_w, "F5", "Refresh view"),
        help_line(key_w, "v", "Toggle system-audio visualizer"),
        help_line(key_w, "Tab", "Cycle menu"),
        help_line(key_w, "1 – 9", "Jump to tab"),
        help_line(key_w, "↑ / ↓", "Move cursor"),
        help_line(key_w, "Alt+← / →", "Switch panels"),
        help_line(key_w, "PgUp / PgDn", "Page scroll"),
        help_line(key_w, "Home / End", "First/last item"),
        help_line(key_w, "Enter", "Select/Play/Open"),
        help_line(key_w, ".", "Context menu"),
        help_line(key_w, "c", "Clear Queue"),
        help_line(key_w, "q", "Quit"),
        help_blank(),
    ];
    // Rendered from `PLAYBACK_HELP_BINDINGS` (issue #133, phase 4) so this
    // section can no longer silently drift from `playback_command_for_key`.
    let mut sec_playback = vec![help_section_line("Playback")];
    sec_playback.extend(
        PLAYBACK_HELP_BINDINGS
            .iter()
            .map(|b| help_line(key_w, b.keys, b.label)),
    );
    sec_playback.push(help_line(key_w, "o", "Open idle feed link"));
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
        help_line(key_w, "Ctrl+R", "Re-anchor tracking"),
        help_line(key_w, "Ctrl+T", "Stop remote tracking"),
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

impl App {
    pub(in crate::app::render) fn render_help_panel(
        &mut self,
        f: &mut Frame,
        area: Option<ratatui::layout::Rect>,
    ) {
        let content = match area {
            Some(area) => Self::render_panel_shell_at(
                f,
                area,
                "KEYBOARD SHORTCUTS",
                "[↑↓]scroll [Esc]close",
                true,
            ),
            None => Self::render_panel_shell(
                f,
                f.area(),
                HELP_PANEL_W,
                "KEYBOARD SHORTCUTS",
                "[↑↓]scroll [Esc]close",
            ),
        };
        let key_w = 16usize;

        let mut sections: [Option<Vec<Line<'static>>>; 7] = std::array::from_fn(|_| None);
        for (name, lines) in build_help_sections(key_w) {
            sections[name.index()] = Some(lines);
        }
        let dest = self.help_destination();
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
        self.help_scroll = self.help_scroll.min(total.saturating_sub(visible) as u16);
        f.render_widget(Paragraph::new(lines).scroll((self.help_scroll, 0)), content);
        Self::render_sidebar_scrollbar(f, content, total, self.help_scroll as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;

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
        let mut app = make_app_stub();
        app.panel_focus = PanelFocus::Library;
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        assert_eq!(app.help_destination(), HelpDestination::Audiobookshelf);
        // Not classified as an Emby library.
        assert_ne!(app.help_destination(), HelpDestination::EmbyLibrary);
    }

    #[test]
    fn audiobookshelf_help_lists_spec_key_sets_first() {
        let sections = build_help_sections(16);
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
        let sections = build_help_sections(16);
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
        let mut app = make_app_stub();
        // Queue focus over a retained Audiobookshelf destination.
        app.panel_focus = PanelFocus::Queue;
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        assert_eq!(app.help_destination(), HelpDestination::Queue);
        let order = help_section_order(HelpDestination::Queue);
        assert_eq!(order[0], HelpSection::Queue);
        assert!(
            order.contains(&HelpSection::Audiobookshelf),
            "queue help must retain the selected Audiobookshelf destination below Queue: {order:?}"
        );
        // The retained browse destination itself is unchanged.
        assert!(matches!(app.tab, TabSelection::AudiobookshelfLibrary(0)));
    }
}
