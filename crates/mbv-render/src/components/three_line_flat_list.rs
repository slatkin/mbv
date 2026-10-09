use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Borrowed data needed to paint a three-line flat list.
#[derive(Debug)]
pub struct ThreeLineFlatListPaintInput<'a, Target> {
    pub items: &'a [ThreeLineItem<Target>],
    pub selected: Option<&'a Target>,
    pub focused: bool,
    pub offset: usize,
    pub visible: usize,
    pub gap: usize,
}

#[derive(Debug)]
pub struct ThreeLineFlatListPaintResult<Target> {
    pub hits: Vec<(Rect, Target)>,
    pub selected_rect: Option<Rect>,
}

/// `claim_rect` is the owning panel's full-width span (so the selected row's
/// bar reaches the panel edges like every other selected-row paint);
/// `content_rect` is the inset row-flow span that owns text and hit geometry.
/// Zebra and ordinary rows stay inside `content_rect`; only the selected row
/// claims the full width (the `tree_browser` precedent).
pub fn render_three_line_flat_list<Target: Clone + Eq>(
    frame: &mut Frame,
    claim_rect: Rect,
    content_rect: Rect,
    input: &ThreeLineFlatListPaintInput<'_, Target>,
) -> ThreeLineFlatListPaintResult<Target> {
    let surface = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
    frame.render_widget(
        Paragraph::new(" ").style(Style::default().bg(surface)),
        content_rect,
    );
    let left_inset = content_rect.x.saturating_sub(claim_rect.x);
    let ThreeLineFlatListPaintInput {
        items,
        selected: selected_target,
        focused,
        offset,
        visible,
        gap,
    } = input;
    let mut hits = Vec::with_capacity(*visible);
    let mut selected_rect = None;
    for (index, item) in items.iter().enumerate().skip(*offset).take(*visible) {
        let y = content_rect.y + u16::try_from((index - offset) * (3 + gap)).unwrap_or(u16::MAX);
        let selected = *focused && *selected_target == Some(&item.target);
        let (row_x, row_width) = if selected {
            (claim_rect.x, claim_rect.width)
        } else {
            (content_rect.x, content_rect.width)
        };
        let rect = Rect::new(row_x, y, row_width, 3);
        let bg = if selected {
            palette::surface_colors(palette::Surface::SelectedRow, false).fill
        } else if index % 2 == 1 {
            palette::surface_colors(palette::Surface::ListStripe, false).fill
        } else {
            surface
        };
        for line_index in 0..3 {
            let line_area = Rect::new(row_x, y + line_index, row_width, 1);
            let mut spans = Vec::new();
            if selected && left_inset > 0 {
                spans.push(Span::raw(" ".repeat(left_inset as usize)));
            }
            spans.extend(
                item.lines[line_index as usize]
                    .iter()
                    .map(|span| {
                        let fg = if selected
                            && !matches!(span.role, ThreeLineRole::Accent | ThreeLineRole::Badge(_))
                        {
                            palette::SELECTED_ROW_FG
                        } else {
                            match span.role {
                                ThreeLineRole::Name => palette::TEXT_PRIMARY,
                                ThreeLineRole::Kind => palette::TEXT_EMPHASIS,
                                ThreeLineRole::Detail => palette::TEXT_MUTED,
                                ThreeLineRole::Status => palette::STATUS_AVAILABLE,
                                ThreeLineRole::Accent => palette::ACCENT_ACTIVE,
                                ThreeLineRole::Badge(color) => color,
                            }
                        };
                        Span::styled(span.text.clone(), Style::default().fg(fg).bg(bg))
                    })
                    .collect::<Vec<_>>(),
            );
            frame.render_widget(
                Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
                line_area,
            );
        }
        hits.push((rect, item.target.clone()));
        if selected {
            selected_rect = Some(rect);
        }
    }
    ThreeLineFlatListPaintResult {
        hits,
        selected_rect,
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThreeLineRole {
    #[default]
    Name,
    Kind,
    Detail,
    Status,
    Accent,
    /// Explicit-color badge (e.g. a nerd-font service glyph): the component
    /// resolves the color shell-side, and the painter preserves it on the
    /// selected row like `Accent`.
    Badge(Color),
}

/// One styled text span in a three-line item's presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLineSpan {
    pub text: String,
    pub role: ThreeLineRole,
}

impl ThreeLineSpan {
    pub fn new(text: impl Into<String>, role: ThreeLineRole) -> Self {
        Self {
            text: text.into(),
            role,
        }
    }
}

/// Target identity and presentation are independent of Service objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLineItem<Target> {
    pub target: Target,
    pub lines: [Vec<ThreeLineSpan>; 3],
}

impl<Target> ThreeLineItem<Target> {
    pub fn new(target: Target, lines: [Vec<ThreeLineSpan>; 3]) -> Self {
        Self { target, lines }
    }
}
