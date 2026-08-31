use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct QueuePanelGeometry {
    pub content_area: Rect,
    pub pill_row: Option<Rect>,
    pub title_reserved: bool,
}

/// Places the queue title, content, and status rows inside its framed area.
pub(in crate::app) fn queue_panel_geometry(qla: Rect) -> QueuePanelGeometry {
    let title_overhead = u16::from(qla.height >= 4) * 3;
    let title_reserved = title_overhead > 0;
    let status_overhead = u16::from(qla.height >= title_overhead + 4) * 3;
    let content_area = Rect {
        y: qla.y + title_overhead,
        height: qla.height.saturating_sub(title_overhead + status_overhead),
        ..qla
    };
    let pill_row = (status_overhead > 0).then(|| Rect {
        x: qla.x + 2,
        y: qla.y + qla.height.saturating_sub(2),
        width: qla.width.saturating_sub(4),
        height: 1,
    });
    QueuePanelGeometry {
        content_area,
        pill_row,
        title_reserved,
    }
}
