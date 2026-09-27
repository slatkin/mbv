use crate::app::components::media_list::{
    queue_row_zebra_stripe, WideMediaList, WideMediaListPaintPolicy,
};
use crate::app::{palette, App, RemoteSlotState};
use mbv_queue::QueueSlotId;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use tuirealm::component::Component;
use unicode_width::UnicodeWidthStr;

/// The media-list Queue hands to the render layer this frame (design.md
/// D1/D2). Queue keeps the Wide presentation in every panel mode; the closed
/// handoff keeps its body paint behind the carrier's surface instead of
/// reaching into one adapter.
pub(in crate::app) enum QueuePresentation<'a> {
    Wide(&'a mut WideMediaList<QueueSlotId>),
}

/// Paint the Queue body through the handed-over presentation once: configure
/// the closed paint policy, paint the rows, and retain the current frame's
/// point-resolution facts. This is the sole Queue body painter.
pub(in crate::app) fn render_queue_body(
    frame: &mut Frame,
    area: Rect,
    presentation: QueuePresentation<'_>,
    focused: bool,
) {
    match presentation {
        QueuePresentation::Wide(list) => {
            list.set_paint_policy(
                WideMediaListPaintPolicy::for_queue(focused).with_zebra(queue_row_zebra_stripe()),
            );
            Component::view(list, frame, area);
        }
    }
}

/// The remote-attachment indicator the queue footer paints: `Some` names
/// the playback target while a remote owner (daemon or session) owns
/// playback, `None` for a bare local player.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct QueueTitleModel {
    pub remote_pill: Option<String>,
}

impl App {
    pub(in crate::app) fn queue_title_model(&self) -> QueueTitleModel {
        let remote_state = self.remote_slot_state();
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
        let (icon, label) = self.remote_icon_and_label(remote_state, &daemon_endpoint);
        let remote_pill =
            (remote_state != RemoteSlotState::Off).then(|| format!(" {} {} ", icon, label.trim()));
        QueueTitleModel { remote_pill }
    }
}

pub(in crate::app) fn render_queue_status(
    frame: &mut Frame,
    area: Rect,
    playlist: Vec<Span<'static>>,
    autosave: Option<Vec<Span<'static>>>,
    remote_pill: Option<&str>,
) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(palette::surface_colors(palette::Surface::QueuePanelBand, false).fill),
        ),
        area,
    );
    frame.render_widget(Paragraph::new(Line::from(playlist)), area);
    // The remote-attachment indicator pill: far right, winning over
    // autosave and the playlist on narrow footers.
    let pill = remote_pill.map(|content| {
        Span::styled(
            content.to_string(),
            Style::default()
                .fg(palette::TEXT_FOCUS_ACCENT)
                .bg(palette::surface_colors(palette::Surface::QueueScopePillSelected, false).fill),
        )
    });
    let pill_w = pill
        .as_ref()
        .map_or(0, |s| u16::try_from(s.content.width()).unwrap_or(u16::MAX));
    if let Some(pill) = pill.filter(|_| pill_w > 0 && pill_w < area.width) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![pill])),
            Rect {
                x: area.x + area.width - pill_w,
                y: area.y,
                width: pill_w,
                height: 1,
            },
        );
    }
    if let Some(spans) = autosave {
        let width = spans
            .iter()
            .map(|span| u16::try_from(span.content.width()).unwrap_or(u16::MAX))
            .sum::<u16>();
        // Autosave yields the far right to the indicator pill when shown.
        let x = area.x + area.width.saturating_sub(pill_w).saturating_sub(width);
        if x > area.x {
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect {
                    x,
                    y: area.y,
                    width,
                    height: 1,
                },
            );
        }
    }
}
