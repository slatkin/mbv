use std::time::{Duration, Instant};
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Click,
    DoubleClick,
    RightClick,
    Scroll(i16),
    DragStart,
    DragMove,
    DragEnd,
    HoverEnter,
    HoverLeave,
}

/// Per-mounted-parent mouse recognizer. Drag and hover variants are reserved.
#[derive(Debug, Default)]
pub struct MouseGestureState {
    last_click: Option<(Instant, u16, u16)>,
    last_scroll: Option<Instant>,
}

impl MouseGestureState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn recognize(&mut self, event: &MouseEvent, now: Instant) -> Option<Gesture> {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let double = self.last_click.is_some_and(|(at, c, r)| {
                    now.duration_since(at) <= DOUBLE_CLICK_WINDOW
                        && c == event.column
                        && r == event.row
                });
                self.last_click = Some((now, event.column, event.row));
                Some(if double {
                    Gesture::DoubleClick
                } else {
                    Gesture::Click
                })
            }
            MouseEventKind::Down(MouseButton::Right) => Some(Gesture::RightClick),
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                if self
                    .last_scroll
                    .is_some_and(|at| now.duration_since(at) < Duration::from_millis(50))
                {
                    return None;
                }
                self.last_scroll = Some(now);
                Some(Gesture::Scroll(
                    if matches!(event.kind, MouseEventKind::ScrollUp) {
                        1
                    } else {
                        -1
                    },
                ))
            }
            MouseEventKind::Drag(_) | MouseEventKind::Moved => None,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_double_click_and_throttles_wheel() {
        let mut state = MouseGestureState::new();
        let t = Instant::now();
        let click = |kind| MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        };
        assert_eq!(
            state.recognize(&click(MouseEventKind::Down(MouseButton::Left)), t),
            Some(Gesture::Click)
        );
        assert_eq!(
            state.recognize(
                &click(MouseEventKind::Down(MouseButton::Left)),
                t + Duration::from_millis(100)
            ),
            Some(Gesture::DoubleClick)
        );
        assert_eq!(
            state.recognize(
                &click(MouseEventKind::ScrollUp),
                t + Duration::from_millis(100)
            ),
            Some(Gesture::Scroll(1))
        );
    }
}
