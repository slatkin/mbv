use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::Write;

type AppTerminal = Terminal<CrosstermBackend<std::io::Stdout>>;

pub(in crate::app) fn init_terminal(
    mouse_support: bool,
) -> Result<AppTerminal, Box<dyn std::error::Error>> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    set_mouse_capture(&mut stdout, mouse_support)?;
    set_shift_escape_mode(&mut stdout, true)?;
    crossterm::execute!(stdout, crossterm::event::EnableFocusChange)?;
    let _ = crossterm::execute!(
        stdout,
        crossterm::event::PushKeyboardEnhancementFlags(
            crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    );
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

pub(in crate::app) fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
    }
}

fn set_shift_escape_mode<W: Write>(writer: &mut W, enabled: bool) -> std::io::Result<()> {
    writer.write_all(if enabled { b"\x1b[>1s" } else { b"\x1b[>0s" })
}

pub(crate) fn set_mouse_capture<W: Write>(writer: &mut W, enabled: bool) -> std::io::Result<()> {
    if enabled {
        crossterm::execute!(writer, crossterm::event::EnableMouseCapture)
    } else {
        crossterm::execute!(writer, crossterm::event::DisableMouseCapture)
    }
}

pub(in crate::app) fn restore_terminal(
    mut terminal: AppTerminal,
) -> Result<(), Box<dyn std::error::Error>> {
    crossterm::terminal::disable_raw_mode()?;
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::event::PopKeyboardEnhancementFlags
    );
    set_shift_escape_mode(terminal.backend_mut(), false)?;
    set_mouse_capture(terminal.backend_mut(), false)?;
    crossterm::execute!(terminal.backend_mut(), crossterm::event::DisableFocusChange)?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod shift_escape_tests {
    use super::set_shift_escape_mode;

    #[test]
    fn emits_xtshift_escape_mode_sequences() {
        let mut output = Vec::new();
        set_shift_escape_mode(&mut output, true).unwrap();
        assert_eq!(output, b"\x1b[>1s");
        output.clear();
        set_shift_escape_mode(&mut output, false).unwrap();
        assert_eq!(output, b"\x1b[>0s");
    }
}
