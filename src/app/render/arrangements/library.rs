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
        pills_area,
        spacer_area,
        content_area,
    } = if has_selector {
        wide_hero::pill_bar_areas(area)
    } else {
        wide_hero::spacer_only_areas(area)
    };
    let (hero_panel, browser_panel) = if show_hero {
        let wide_hero::WideHeroPanes { hero, browser } =
            wide_hero::wide_hero_presentation(content_area, override_width);
        (hero, browser)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render::arrangements::wide_hero::{
        WIDE_HERO_MIN_AREA_HEIGHT, WIDE_HERO_PILL_BAND_HEIGHT,
    };

    #[test]
    fn overlay_exactly_matches_the_library_pane() {
        for area in [
            Rect::new(3, 4, 38, 12),
            Rect::new(1, 1, 20, 8),
            Rect::new(2, 3, 60, 5),
            Rect::new(7, 5, 80, 24),
        ] {
            let overlay = library_hero_overlay(area).expect("non-empty pane");
            assert_eq!(overlay, area);
        }

        let library = Rect::new(2, 3, 72, 24);
        let queue = Rect::new(library.right(), library.y, 38, library.height);
        let overlay = library_hero_overlay(library).unwrap();
        assert_eq!(overlay.intersection(queue).width, 0);
    }

    #[test]
    fn wide_library_preserves_breakpoint_and_padding() {
        let area = Rect {
            x: 2,
            y: 3,
            width: crate::app::TWO_COLUMN_THRESHOLD,
            height: WIDE_HERO_MIN_AREA_HEIGHT + 1,
        };
        let panes = wide_library_panes(area, 2, 1, None, true).expect("wide area");
        assert_eq!(panes.hero_area.x, panes.hero_panel.x + 2);
        assert_eq!(panes.hero_area.y, panes.hero_panel.y + 1);
        assert!(wide_library_panes(
            Rect {
                height: WIDE_HERO_MIN_AREA_HEIGHT,
                ..area
            },
            2,
            1,
            None,
            true,
        )
        .is_none());
    }

    #[test]
    fn suppressed_hero_gives_the_browser_the_full_content_area() {
        let area = Rect::new(2, 3, crate::app::TWO_COLUMN_THRESHOLD + 20, 30);
        let panes = wide_library_panes(area, 2, 1, Some(70), false).expect("wide area");
        assert_eq!(panes.browser_panel, panes.content_area);
        assert_eq!(panes.hero_panel.width, 0);
        assert_eq!(panes.hero_panel.height, panes.content_area.height);
    }

    /// Fit-first-then-carve (D1): the band carve must not move the Wide
    /// breakpoint (no two-row shift), and when Wide is chosen the band spans
    /// the full panel width while both panes start at its bottom.
    #[test]
    fn band_carve_keeps_the_breakpoint_and_pushes_both_panes_below_it() {
        // Raw heights around the breakpoint: 7 is the shortest Wide area, 6 is
        // one row too short (the pre-change decision, unchanged).
        for height in [7u16, 8, 9] {
            let area = Rect {
                x: 2,
                y: 3,
                width: crate::app::TWO_COLUMN_THRESHOLD,
                height,
            };
            let panes = wide_library_panes(area, 2, 1, None, true)
                .unwrap_or_else(|| panic!("height {height} keeps the Wide presentation"));
            assert_eq!(panes.pills_area.y, area.y);
            assert_eq!(panes.pills_area.width, area.width);
            assert_eq!(panes.spacer_area.y, panes.pills_area.bottom());
            assert_eq!(panes.spacer_area.width, area.width);
            assert_eq!(
                panes.content_area.height,
                height - WIDE_HERO_PILL_BAND_HEIGHT
            );
            // Both panes start at the band's bottom and tile the content area.
            assert_eq!(panes.hero_panel.y, panes.spacer_area.bottom());
            assert_eq!(panes.browser_panel.y, panes.spacer_area.bottom());
            assert_eq!(panes.hero_panel.bottom(), area.bottom());
            assert_eq!(panes.browser_panel.bottom(), area.bottom());
        }
        assert!(
            wide_library_panes(
                Rect {
                    x: 2,
                    y: 3,
                    width: crate::app::TWO_COLUMN_THRESHOLD,
                    height: WIDE_HERO_MIN_AREA_HEIGHT,
                },
                2,
                1,
                None,
                true,
            )
            .is_none(),
            "one row below the shortest Wide area stays Narrow"
        );
    }
}
