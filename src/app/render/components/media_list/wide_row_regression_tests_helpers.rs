use super::wide::MediaListPaint;
use crate::app::components::media_list::{
    MediaKind, MediaListRow, MediaSemanticState, WideMediaList,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;

pub(super) fn item(target: &str, primary: &str, duration: Option<String>) -> MediaListRow<String> {
    row_of(target, primary, duration, MediaKind::Media)
}

pub(super) fn heading(text: &str) -> MediaListRow<String> {
    MediaListRow::Heading { text: text.into() }
}

pub(super) fn row_of(
    target: &str,
    primary: &str,
    duration: Option<String>,
    kind: MediaKind,
) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: primary.into(),
        secondary: None,
        trailing: None,
        duration,
        kind,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

pub(super) fn paint(
    list: &mut WideMediaList<String>,
    rect: Rect,
    selected_bg: Color,
) -> MediaListPaint<String> {
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    let mut captured = None;
    terminal
        .draw(|f| {
            captured = Some(render_wide_media_list(
                f,
                rect,
                rect,
                list,
                true,
                selected_bg,
            ));
        })
        .unwrap();
    captured.unwrap()
}

use super::wide::render_wide_media_list;
