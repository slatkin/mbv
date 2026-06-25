use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::api::EmbyClient;
use crate::config::Config;

const BASE:    Color = Color::Rgb(26,  26,  26);
const OVERLAY: Color = Color::Rgb(63,  63,  63);
const MUTED:   Color = Color::Rgb(108, 108, 108);
const TEXT:    Color = Color::Rgb(230, 230, 230);
const IRIS:    Color = Color::Rgb(82,  181, 75);
const LOVE:    Color = Color::Rgb(220, 60,  60);

struct LoginForm {
    fields: [String; 3], // [server_url, username, password]
    focus: usize,
    error: String,
    busy: bool,
}

impl LoginForm {
    fn new(url: &str, username: &str) -> Self {
        LoginForm {
            fields: [url.to_string(), username.to_string(), String::new()],
            focus: if url.is_empty() { 0 } else if username.is_empty() { 1 } else { 2 },
            error: String::new(),
            busy: false,
        }
    }
}

pub fn run(base_client: EmbyClient) -> Result<EmbyClient, Box<dyn std::error::Error>> {
    let base_config = base_client.config.clone();

    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mut form = LoginForm::new(&base_config.server_url, &base_config.username);
    let mut done: Option<Result<EmbyClient, Box<dyn std::error::Error>>> = None;

    while done.is_none() {
        terminal.draw(|f| render(f, &form))?;

        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press { continue; }

        match key.code {
            KeyCode::Esc => {
                done = Some(Err("cancelled".into()));
            }
            KeyCode::Tab | KeyCode::Down => {
                form.focus = (form.focus + 1) % 3;
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.focus = if form.focus == 0 { 2 } else { form.focus - 1 };
            }
            KeyCode::Enter => {
                if form.focus < 2 {
                    form.focus += 1;
                    continue;
                }
                let url = form.fields[0].trim().trim_end_matches('/').to_string();
                if url.is_empty() {
                    form.error = "Server URL is required".into();
                    form.focus = 0;
                    continue;
                }
                let username = form.fields[1].trim().to_string();
                if username.is_empty() {
                    form.error = "Username is required".into();
                    form.focus = 1;
                    continue;
                }

                form.busy = true;
                form.error = "Logging in\u{2026}".into();
                terminal.draw(|f| render(f, &form))?;
                form.busy = false;

                let config = Config {
                    server_url: url,
                    username,
                    password: form.fields[2].clone(),
                    api_key: base_config.api_key.clone(),
                    hidden_libraries: base_config.hidden_libraries.clone(),
                    hidden_latest: base_config.hidden_latest.clone(),
                    show_audio_window: base_config.show_audio_window,
                    use_mpv_config: base_config.use_mpv_config,
                    always_play_next: base_config.always_play_next,
                    consume_videos: base_config.consume_videos,
                    always_skip_intro: base_config.always_skip_intro,
                    image_protocol: base_config.image_protocol.clone(),
                    show_systray_icon: base_config.show_systray_icon,
                    show_log_tab: base_config.show_log_tab,
                    no_scripts: base_config.no_scripts,
                    start_on_queue: base_config.start_on_queue,
                    daemon_mode_on_exit: base_config.daemon_mode_on_exit,
                    autoload: base_config.autoload,
                    music_levels: base_config.music_levels.clone(),
                    system_notifications: base_config.system_notifications,
                    image_cache_size: base_config.image_cache_size,
                    save_playlist_on_consume: base_config.save_playlist_on_consume,
                    use_nerd_fonts: base_config.use_nerd_fonts,
                    subtitle_mode: base_config.subtitle_mode.clone(),
                    subtitle_lang: base_config.subtitle_lang.clone(),
                    audio_lang: base_config.audio_lang.clone(),
                    my_languages: base_config.my_languages.clone(),
                    feed_view_libraries: base_config.feed_view_libraries.clone(),
                    config_version: base_config.config_version,
                };
                let mut client = EmbyClient::new(config);
                match client.authenticate_credentials() {
                    Ok(()) => done = Some(Ok(client)),
                    Err(e) => {
                        form.error = e;
                        form.fields[2].clear();
                    }
                }
            }
            KeyCode::Backspace => {
                form.fields[form.focus].pop();
                form.error.clear();
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE
                    || key.modifiers == KeyModifiers::SHIFT =>
            {
                form.fields[form.focus].push(c);
                form.error.clear();
            }
            _ => {}
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    done.unwrap()
}

fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    }
}

fn render(f: &mut ratatui::Frame, form: &LoginForm) {
    f.render_widget(
        Block::default().style(Style::default().bg(BASE)),
        f.area(),
    );

    let box_w = 52u16.min(f.area().width.saturating_sub(2));
    let box_h = 15u16.min(f.area().height);
    let area = centered_rect(box_w, box_h, f.area());

    f.render_widget(Clear, area);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(IRIS))
            .title(" mbv ")
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(BASE)),
        area,
    );

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    // blank / label / input / blank / label / input / blank / label / input / blank / hints / spacer / status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0] blank
            Constraint::Length(1), // [1] Server URL label
            Constraint::Length(1), // [2] Server URL input
            Constraint::Length(1), // [3] blank
            Constraint::Length(1), // [4] Username label
            Constraint::Length(1), // [5] Username input
            Constraint::Length(1), // [6] blank
            Constraint::Length(1), // [7] Password label
            Constraint::Length(1), // [8] Password input
            Constraint::Length(1), // [9] blank
            Constraint::Length(1), // [10] hints
            Constraint::Min(0),    // [11] spacer
            Constraint::Length(1), // [12] status / error
        ])
        .split(inner);

    let labels = ["Emby Server URL", "Username", "Password"];
    let label_rows = [1usize, 4, 7];
    let input_rows = [2usize, 5, 8];

    for i in 0..3usize {
        let focused = form.focus == i;
        let label_style = if focused {
            Style::default().fg(IRIS).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED)
        };
        f.render_widget(Paragraph::new(labels[i]).style(label_style), chunks[label_rows[i]]);

        let text = if i == 2 {
            "\u{25cf}".repeat(form.fields[i].chars().count())
        } else {
            form.fields[i].clone()
        };
        let display = if focused { format!("{text}\u{258f}") } else { text };
        let field_style = if focused {
            Style::default().fg(TEXT).bg(OVERLAY)
        } else {
            Style::default().fg(MUTED).bg(OVERLAY)
        };
        f.render_widget(Paragraph::new(display).style(field_style), chunks[input_rows[i]]);
    }

    let hint = Line::from(vec![
        Span::styled("Tab", Style::default().fg(IRIS)),
        Span::styled(" next  ", Style::default().fg(MUTED)),
        Span::styled("Enter", Style::default().fg(IRIS)),
        Span::styled(" login  ", Style::default().fg(MUTED)),
        Span::styled("Esc", Style::default().fg(IRIS)),
        Span::styled(" quit", Style::default().fg(MUTED)),
    ]);
    f.render_widget(Paragraph::new(hint), chunks[10]);

    if !form.error.is_empty() {
        let style = if form.busy {
            Style::default().fg(MUTED)
        } else {
            Style::default().fg(LOVE)
        };
        f.render_widget(Paragraph::new(form.error.as_str()).style(style), chunks[12]);
    }
}
