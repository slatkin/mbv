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
    /// Rows the Queue playback panel's header row always spends above the
    /// Queue panel in every queue-visible layout, idle included (design D10;
    /// task 3.2). While idle the header row is the only space above the
    /// Queue panel besides the panel's own recessed inset; the separator row
    /// is reserved only alongside the visual-slot and transport heights.
    pub header_height: u16,
    pub card_height: u16,
}

/// The Queue panel's framed sub-areas inside its placement: the framed content
/// box, the title band and the status pill row. Shared by the root placement
/// (`queue_panel_geometry`) and the mounted `QueuePanel`'s own view, which
/// derives the same sub-areas from the placement it is handed (task 3.1) --
/// one source for the panel's internal geometry.
pub(in crate::app) fn queue_panel_subareas(
    panel_area: Rect,
) -> (Rect, Option<Rect>, Option<Rect>, bool) {
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
    (content_area, title_area, pill_row, title_reserved)
}

/// Places the complete queue panel and its framed sub-areas.
pub(in crate::app) fn queue_panel_geometry(input: QueuePanelInputs) -> QueuePanelGeometry {
    // The header row (always present, idle included) plus the visual-slot and
    // transport rows sit above the panel, separated from it by one gap row.
    // The gap belongs to the slot/transport band: while idle nothing renders
    // between the header and the panel, and the panel's recessed top inset is
    // the single space row below the header.
    let playback_rows = input.header_height + input.card_height;
    let gap = u16::from(input.card_height > 0);
    let panel_area = Rect {
        y: input.left_content.y + playback_rows + gap,
        height: input
            .left_content
            .height
            .saturating_sub(playback_rows)
            .saturating_sub(gap),
        ..input.left_content
    };
    let (content_area, title_area, pill_row, title_reserved) = queue_panel_subareas(panel_area);
    QueuePanelGeometry {
        panel_area,
        content_area,
        title_area,
        pill_row,
        title_reserved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left_content(height: u16) -> Rect {
        Rect::new(0, 0, 40, height)
    }

    /// Task 3.2: the Queue panel always starts below the Queue playback
    /// panel's placement (header row + visual slot/transport rows), with one
    /// separator row whenever slot/transport rows exist. While idle only the
    /// header row is reserved above the panel (the panel's recessed inset is
    /// the single space row).
    #[test]
    fn queue_panel_starts_below_header_and_separator_in_every_playback_state() {
        let header = 1u16;
        // (card_rows, separator_rows, label)
        let cases: [(u16, u16, &str); 3] = [
            (0, 0, "idle: no visual slot, no transport, no separator"),
            (
                2,
                1,
                "paused: visual slot only (transport rows collapse into the slot height)",
            ),
            (6, 1, "playing: visual slot + transport rows"),
        ];
        for (card_rows, gap, label) in cases {
            let geometry = queue_panel_geometry(QueuePanelInputs {
                left_content: left_content(30),
                header_height: header,
                card_height: card_rows,
            });
            // Header row (always) + card/transport rows + the separator row.
            assert_eq!(
                geometry.panel_area.y,
                header + card_rows + gap,
                "{label}: the queue panel must start below the header row and its separator"
            );
            assert_eq!(
                geometry.panel_area.height,
                30 - header - card_rows - gap,
                "{label}: the queue panel takes the remaining rows"
            );
            assert_eq!(
                geometry.panel_area.bottom(),
                30,
                "{label}: the queue panel ends at the column's content bottom"
            );
            assert!(
                geometry.title_area.unwrap_or_default().y >= geometry.panel_area.y
                    && geometry.content_area.y >= geometry.panel_area.y,
                "{label}: sub-areas live inside the panel"
            );
        }
    }

    /// The framed sub-area split is shared with the mounted panel's view:
    /// title band, content box and status pill row all sit inside the panel.
    #[test]
    fn queue_panel_subareas_stay_inside_the_panel() {
        let panel = Rect::new(2, 5, 30, 20);
        let (content, title, pill, title_reserved) = queue_panel_subareas(panel);
        assert!(title_reserved);
        let title = title.expect("20-row panel reserves a title band");
        let pill = pill.expect("20-row panel reserves a status row");
        assert!(panel.contains((title.x, title.y).into()));
        assert!(panel.contains((content.x, content.y).into()));
        assert!(panel.contains((pill.x, pill.y).into()));
        assert_eq!(title.width, panel.width.saturating_sub(4));
        assert!(content.bottom() <= pill.y);

        // A degenerate panel reserves nothing.
        let (content, title, pill, reserved) = queue_panel_subareas(Rect::new(0, 0, 10, 3));
        assert!(title.is_none() && pill.is_none() && !reserved);
        assert_eq!(content.height, 3);
    }

    /// The idle queue-only pane-geometry chain (review of tasks 3.1-3.4,
    /// moved from the queue render test): with no visual slot or transport
    /// rows, the list content starts below the header row and the panel's
    /// reserved title band — the panel's recessed inset is the single space
    /// row below the header — all from the arrangement's own inputs, not
    /// pulled from a render-test buffer.
    #[test]
    fn idle_pane_starts_below_header_and_title_band() {
        let header = 1u16;
        let left = left_content(30);
        let geometry = queue_panel_geometry(QueuePanelInputs {
            left_content: left,
            header_height: header,
            card_height: 0,
        });
        // Header row (the panel's own inset is the one space row) + the
        // panel's 3-row title band.
        assert_eq!(geometry.content_area.y, left.y + header + 3);
        assert_eq!(geometry.panel_area.bottom(), 30);
    }
}
