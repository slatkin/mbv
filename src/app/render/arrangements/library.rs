use super::{padded_rect, wide_hero};
use ratatui::layout::Rect;

/// The shared padded panes used by wide library presentations.
pub(in crate::app) struct WideLibraryPanes {
    /// The full-width Selector band above both panes when one is present;
    /// otherwise both rects have zero height.
    pub pills_area: Rect,
    pub spacer_area: Rect,
    /// The area the Hero/Browser split was computed over; both panes tile it.
    /// It starts below the Selector band when one is present.
    pub content_area: Rect,
    pub hero_panel: Rect,
    pub browser_panel: Rect,
    pub hero_area: Rect,
}

pub(in crate::app) fn wide_library_panes(
    area: Rect,
    pad_x: u16,
    pad_y: u16,
    override_width: Option<u16>,
    show_hero: bool,
) -> Option<WideLibraryPanes> {
    wide_library_panes_with_selector(area, pad_x, pad_y, override_width, show_hero, true)
}

pub(in crate::app) fn wide_library_panes_with_selector(
    area: Rect,
    pad_x: u16,
    pad_y: u16,
    override_width: Option<u16>,
    show_hero: bool,
    has_selector: bool,
) -> Option<WideLibraryPanes> {
    // Fit-first-then-carve (D1): the breakpoint is decided on the uncarved
    // `area`, so carving the Selector band cannot shift the Wide/Narrow choice
    // (nor strand a stale frame at the boundary heights).
    if !wide_hero::wide_hero_fits(area) {
        return None;
    }
    let wide_hero::PillBarAreas {
        pills: pills_area,
        spacer: spacer_area,
        content: content_area,
    } = if has_selector {
        wide_hero::pill_bar_areas(area)
    } else {
        wide_hero::no_selector_areas(area)
    };
    let (hero_panel, browser_panel) = if show_hero {
        let wide_hero::WideHeroPanes { left, right } =
            wide_hero::wide_hero_presentation(content_area, override_width);
        (left, right)
    } else {
        (
            Rect::new(content_area.x, content_area.y, 0, content_area.height),
            content_area,
        )
    };
    let hero_area = padded_rect(hero_panel, pad_x, pad_y);
    Some(WideLibraryPanes {
        pills_area,
        spacer_area,
        content_area,
        hero_panel,
        browser_panel,
        hero_area,
    })
}

/// Place the Library-local Hero overlay exactly over the supplied area.
///
/// The caller supplies the area the overlay owns: the non-Wide browser's inset
/// list box, so the overlay and its dim backdrop never cover the pill bar or
/// the spacer band above it. The overlay is the same size as that list panel,
/// not a fraction of the terminal: this keeps the Queue column outside both
/// the frame and its dimmed backdrop.
pub(in crate::app) fn library_hero_overlay(area: Rect) -> Option<Rect> {
    (area.width > 0 && area.height > 0).then_some(area)
}
