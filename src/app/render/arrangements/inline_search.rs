use ratatui::layout::Rect;

/// Areas for one embedded Inline Search session inside a library-list area
/// (design.md D3). The destination passes the exact list area it owns; this
/// admits a three-row input at the top when at least three rows are
/// available, otherwise the input is omitted and the whole area is used for
/// results.
pub(in crate::app) struct InlineSearchAreas {
    pub(in crate::app) input_area: Option<Rect>,
    pub(in crate::app) result_area: Rect,
}

const INPUT_HEIGHT: u16 = 3;

pub(in crate::app) fn search_areas(area: Rect) -> InlineSearchAreas {
    if area.height < INPUT_HEIGHT {
        return InlineSearchAreas {
            input_area: None,
            result_area: area,
        };
    }
    InlineSearchAreas {
        input_area: Some(Rect {
            height: INPUT_HEIGHT,
            ..area
        }),
        result_area: Rect {
            y: area.y + INPUT_HEIGHT,
            height: area.height - INPUT_HEIGHT,
            ..area
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_three_row_input_when_area_fits() {
        let area = Rect {
            x: 1,
            y: 2,
            width: 40,
            height: 10,
        };
        let areas = search_areas(area);
        assert_eq!(
            areas.input_area,
            Some(Rect {
                x: 1,
                y: 2,
                width: 40,
                height: 3
            })
        );
        assert_eq!(
            areas.result_area,
            Rect {
                x: 1,
                y: 5,
                width: 40,
                height: 7
            }
        );
    }

    #[test]
    fn omits_input_when_area_too_short() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 2,
        };
        let areas = search_areas(area);
        assert_eq!(areas.input_area, None);
        assert_eq!(areas.result_area, area);
    }
}
