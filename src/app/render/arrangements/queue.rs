use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct QueuePanelGeometry {
    pub panel_area: Rect,
    pub content_area: Rect,
    pub title_area: Option<Rect>,
    pub pill_row: Option<Rect>,
    pub title_reserved: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct QueuePanelInputs {
    pub left_content: Rect,
    pub card_height: u16,
    pub narrow_player_height: u16,
}

/// Places the complete queue panel and its framed sub-areas.
pub(in crate::app) fn queue_panel_geometry(input: QueuePanelInputs) -> QueuePanelGeometry {
    let panel_area = Rect {
        y: input.left_content.y + input.card_height + input.narrow_player_height + 1,
        height: input
            .left_content
            .height
            .saturating_sub(input.card_height)
            .saturating_sub(input.narrow_player_height)
            .saturating_sub(1),
        ..input.left_content
    };
    let title_reserved = panel_area.height >= 4;
    let title_overhead = u16::from(title_reserved) * 3;
    let status_overhead = u16::from(panel_area.height >= title_overhead + 4) * 3;
    let content_area = Rect {
        y: panel_area.y + title_overhead,
        height: panel_area
            .height
            .saturating_sub(title_overhead + status_overhead),
        ..panel_area
    };
    let title_area = title_reserved.then_some(Rect {
        x: panel_area.x + 2,
        y: panel_area.y + 1,
        width: panel_area.width.saturating_sub(4),
        height: 1,
    });
    let pill_row = (status_overhead > 0).then(|| Rect {
        x: panel_area.x + 2,
        y: panel_area.y + panel_area.height.saturating_sub(2),
        width: panel_area.width.saturating_sub(4),
        height: 1,
    });
    QueuePanelGeometry {
        panel_area,
        content_area,
        title_area,
        pill_row,
        title_reserved,
    }
}
