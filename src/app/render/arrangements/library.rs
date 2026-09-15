use super::{padded_rect, wide_hero};
use ratatui::layout::Rect;

/// The shared padded panes used by wide library presentations.
pub(in crate::app) struct WideLibraryPanes {
    pub hero_panel: Rect,
    pub browser_panel: Rect,
    pub hero_area: Rect,
    pub browser_area: Rect,
}

pub(in crate::app) fn wide_library_panes(
    area: Rect,
    pad_x: u16,
    pad_y: u16,
    override_width: Option<u16>,
) -> Option<WideLibraryPanes> {
    let wide_hero::WideHeroPanes {
        hero: hero_panel,
        browser: browser_panel,
    } = wide_hero::wide_hero_presentation(area, override_width)?;
    let hero_area = padded_rect(hero_panel, pad_x, pad_y);
    let browser_area = Rect {
        x: browser_panel.x,
        y: browser_panel.y.saturating_add(pad_y),
        width: browser_panel.width,
        height: browser_panel.height.saturating_sub(pad_y * 2),
    };
    Some(WideLibraryPanes {
        hero_panel,
        browser_panel,
        hero_area,
        browser_area,
    })
}

/// Place the Library-local Hero overlay inside the supplied Library pane.
///
/// The percentage is deliberately computed from the pane, not the terminal:
/// this keeps the Queue column outside both the frame and its dimmed backdrop.
pub(in crate::app) fn library_hero_overlay(area: Rect) -> Option<Rect> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let width = ((area.width as u32 * 85) / 100).max(1) as u16;
    let height = ((area.height as u32 * 85) / 100).max(1) as u16;
    Some(Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render::arrangements::wide_hero::WIDE_HERO_MIN_AREA_HEIGHT;

    fn contains(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.right() <= outer.right()
            && inner.bottom() <= outer.bottom()
    }

    #[test]
    fn overlay_is_centered_and_contained_by_library_pane() {
        for area in [
            Rect::new(3, 4, 38, 12),
            Rect::new(1, 1, 20, 8),
            Rect::new(2, 3, 60, 5),
            Rect::new(7, 5, 80, 24),
        ] {
            let overlay = library_hero_overlay(area).expect("non-empty pane");
            assert!(contains(area, overlay));
            assert_eq!(overlay.x - area.x, (area.width - overlay.width) / 2);
            assert_eq!(overlay.y - area.y, (area.height - overlay.height) / 2);
        }

        let library = Rect::new(2, 3, 72, 24);
        let queue = Rect::new(library.right(), library.y, 38, library.height);
        let overlay = library_hero_overlay(library).unwrap();
        assert!(contains(library, overlay));
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
        let panes = wide_library_panes(area, 2, 1, None).expect("wide area");
        assert_eq!(panes.hero_area.x, panes.hero_panel.x + 2);
        assert_eq!(panes.browser_area.y, panes.browser_panel.y + 1);
        assert!(wide_library_panes(
            Rect {
                height: WIDE_HERO_MIN_AREA_HEIGHT,
                ..area
            },
            2,
            1,
            None,
        )
        .is_none());
    }
}
