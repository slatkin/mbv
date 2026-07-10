use super::super::super::action::PLAYBACK_HELP_BINDINGS;
use super::super::super::palette;
use super::super::super::App;
use super::super::super::HELP_PANEL_W;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_help_panel(&mut self, f: &mut Frame) {
        let content = Self::render_panel_shell(
            f,
            f.area(),
            HELP_PANEL_W,
            "KEYBOARD SHORTCUTS",
            "[↑↓]scroll [Esc]close",
        );
        let key_w = 16usize;

        let mk = |key: &str, desc: &str| -> Line<'static> {
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
        };
        let section = |label: &str| -> Line<'static> {
            Line::from(vec![
                Span::raw(""),
                Span::styled(
                    label.to_owned(),
                    Style::default()
                        .fg(palette::IRIS)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        };
        let blank = || Line::from("");

        let sec_global = vec![
            section("[global]"),
            mk("F1", "Help"),
            mk("F2", "Settings"),
            mk("F3", "Remote sessions"),
            mk("F4", "Playlists"),
            mk("F5", "Refresh view"),
            mk("Tab", "Cycle menu"),
            mk("1 – 9", "Jump to tab"),
            mk("↑ / ↓", "Move cursor"),
            mk("PgUp / PgDn", "Page scroll"),
            mk("Home / End", "First/last item"),
            mk("Enter", "Select/Play/Open"),
            mk(".", "Context menu"),
            mk("c", "Clear Queue"),
            mk("q", "Quit"),
            blank(),
        ];
        // Rendered from `PLAYBACK_HELP_BINDINGS` (issue #133, phase 4) so this
        // section can no longer silently drift from `playback_command_for_key`.
        // `h` (hide/show player) isn't part of that Command seam yet — it
        // lives in `handle_key_panel_toggle` — so it stays hand-written here
        // until that handler migrates too.
        let mut sec_playback = vec![section("[playback]")];
        sec_playback.extend(PLAYBACK_HELP_BINDINGS.iter().map(|b| mk(b.keys, b.label)));
        sec_playback.push(mk("h", "Hide/show player"));
        sec_playback.push(blank());
        let sec_queue = vec![
            section("[queue]"),
            mk("p", "Jump to playing item"),
            mk("i", "Go to item in library"),
            mk("Del", "Remove from Queue"),
            mk("Ctrl+Z", "Undo removal"),
            mk("v", "Toggle view"),
            mk("g", "Toggle grouping"),
            mk("Shift+← / →", "Resize Power View left column"),
            mk("Ctrl+S", "Save playlist"),
            blank(),
        ];
        let sec_home = vec![
            section("[home]"),
            mk("Alt+↑ / ↓", "Switch sections"),
            mk("Ctrl+W", "Toggle watched"),
            mk("Alt+Q", "Add to Queue"),
            blank(),
        ];
        let sec_library = vec![
            section("[library]"),
            mk("Esc / Backspace", "Go back"),
            mk("/", "Search library"),
            mk("Ctrl+W", "Toggle watched"),
            mk("Ctrl+S", "Shuffle"),
            mk("Ctrl+P", "Play all"),
            mk("Alt+Q", "Add to Queue"),
            mk("r", "Refresh library"),
            mk("Ctrl+R", "Rescan"),
            blank(),
        ];
        let is_lib = self.tab_idx >= self.lib_tab_offset();
        let is_queue = self.tab_idx == 1;
        let is_home = self.tab_idx == 0;

        let mut ordered: Vec<Vec<Line>> = Vec::new();
        if is_home {
            ordered.push(sec_home);
            ordered.push(sec_global);
            ordered.push(sec_playback);
            ordered.push(sec_queue);
            ordered.push(sec_library);
        } else if is_queue {
            ordered.push(sec_queue);
            ordered.push(sec_global);
            ordered.push(sec_playback);
            ordered.push(sec_home);
            ordered.push(sec_library);
        } else {
            debug_assert!(is_lib);
            ordered.push(sec_library);
            ordered.push(sec_global);
            ordered.push(sec_playback);
            ordered.push(sec_queue);
            ordered.push(sec_home);
        }

        let mut lines: Vec<Line> = ordered.into_iter().flatten().collect();
        lines.push(blank());

        let total = lines.len();
        let visible = content.height as usize;
        self.help_scroll = self.help_scroll.min(total.saturating_sub(visible) as u16);
        f.render_widget(Paragraph::new(lines).scroll((self.help_scroll, 0)), content);
        Self::render_sidebar_scrollbar(f, content, total, self.help_scroll as usize);
    }
}
