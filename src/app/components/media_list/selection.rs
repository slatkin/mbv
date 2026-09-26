use super::{MediaList, MediaListOperation, MediaListSurfaceInput};

impl MediaListSurfaceInput {
    pub fn into_operation<Target>(
        self,
        target: Option<Target>,
    ) -> Option<MediaListOperation<Target>> {
        Some(match self {
            Self::Move(delta) | Self::Wheel { delta, .. } => MediaListOperation::Move(delta),
            Self::Page(delta) => MediaListOperation::Page(delta),
            Self::First => MediaListOperation::First,
            Self::Last => MediaListOperation::Last,
            Self::Activate => MediaListOperation::ActivateCurrent,
            Self::Context => MediaListOperation::ContextCurrent,
            Self::Click(_) => MediaListOperation::Select(target?),
            Self::ToggleClick(_) => MediaListOperation::Toggle(target?),
            Self::RangeClick(_) => MediaListOperation::Range(target?),
            Self::DoubleClick(_) => MediaListOperation::Activate(target?),
            Self::ContextClick(_) => MediaListOperation::Context(target?),
        })
    }
}

impl<Target: Clone + Eq> MediaList<Target> {
    /// Toggle a target and freeze the resulting explicit set. The first toggle
    /// includes the row that was under the cursor, matching Ctrl+Click.
    pub fn toggle_selection(&mut self, target: &Target) {
        let was_empty = self.multi_selection.is_empty();
        if was_empty {
            let cursor = self.selected_target().cloned();
            if let Some(cursor) = cursor {
                self.multi_selection.add(cursor.clone());
                self.selection_anchor = Some(cursor);
            }
        }
        if self.multi_selection.contains(target) {
            if !was_empty {
                self.multi_selection.remove(target);
            }
        } else if self.position_of(target).is_some() {
            self.multi_selection.add(target.clone());
        }
        if self.multi_selection.is_empty() {
            self.clear_selection();
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(target.clone());
        }
        self.frozen_selection = self.multi_selection.targets().to_vec();
        self.live_range = false;
    }

    /// Extend the anchored range, unioning it with the frozen selection.
    pub fn extend_selection_to(&mut self, target: &Target) {
        let Some(end) = self.position_of(target) else {
            return;
        };
        let anchor = self
            .selection_anchor
            .clone()
            .or_else(|| self.selected_target().cloned())
            .unwrap_or_else(|| target.clone());
        let Some(start) = self.position_of(&anchor) else {
            self.selection_anchor = Some(target.clone());
            self.multi_selection.set_targets([target.clone()]);
            return;
        };
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let selected: Vec<Target> = self
            .selectable
            .iter()
            .enumerate()
            .filter_map(|(index, &row)| {
                let candidate = self.rows[row].selectable_target()?;
                (self.frozen_selection.iter().any(|item| item == candidate)
                    || (lo..=hi).contains(&index))
                .then(|| candidate.clone())
            })
            .collect();
        self.multi_selection.set_targets(selected);
        self.selection_anchor = Some(anchor);
    }

    /// Exit Visual mode and discard all selected targets.
    pub fn clear_selection(&mut self) {
        self.multi_selection.clear();
        self.frozen_selection.clear();
        self.selection_anchor = None;
        self.live_range = false;
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MediaList, MediaListRow};

    fn list() -> MediaList<u8> {
        let mut list = MediaList::new();
        list.set_content((1..=8).map(item).collect());
        list
    }

    fn item(target: u8) -> MediaListRow<u8> {
        MediaListRow::Item {
            target,
            primary: target.to_string(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: super::super::MediaKind::Media,
            semantic_state: super::super::MediaSemanticState::Ordinary,
        }
    }

    #[test]
    fn toggle_adds_and_removes() {
        let mut list = list();
        list.delegate_operation(
            super::super::MediaListSurfaceInput::Click(ratatui::layout::Position { x: 0, y: 0 })
                .into_operation(Some(2))
                .expect("resolved media-list pointer target"),
        );
        list.toggle_selection(&2);
        assert_eq!(list.multi_selection(), &[2]);
        list.toggle_selection(&2);
        assert!(list.multi_selection().is_empty());
    }

    #[test]
    fn first_toggle_includes_prior_cursor() {
        let mut list = list();
        list.select_target(&3);
        list.toggle_selection(&6);
        assert_eq!(list.multi_selection(), &[3, 6]);
    }

    #[test]
    fn range_recomputes_from_anchor() {
        let mut list = list();
        list.select_target(&2);
        list.toggle_selection(&5);
        list.extend_selection_to(&7);
        assert_eq!(list.multi_selection(), &[2, 3, 4, 5, 6, 7]);
        list.extend_selection_to(&4);
        assert_eq!(list.multi_selection(), &[2, 3, 4, 5]);
    }

    #[test]
    fn refresh_keeps_surviving_multi_selection_and_reanchors_vanished_anchor() {
        let mut list = list();
        list.select_target(&3);
        list.toggle_selection(&5);
        list.toggle_selection(&7);
        assert_eq!(list.multi_selection(), &[3, 5, 7]);

        list.set_content(vec![item(2), item(5), item(7), item(8)]);
        assert_eq!(list.multi_selection(), &[5, 7]);
        assert_eq!(list.selected_target(), Some(&2));
        assert_eq!(list.selection_anchor.as_ref(), Some(&2));
    }

    #[test]
    fn visual_mode_starts_at_cursor_and_movement_extends_from_anchor() {
        let mut list = list();
        list.select_target(&3);
        list.enter_visual_mode();
        assert_eq!(list.multi_selection(), &[3]);
        list.delegate_operation(
            super::super::MediaListSurfaceInput::Move(2)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        assert_eq!(list.multi_selection(), &[3, 4, 5]);
        list.delegate_operation(
            super::super::MediaListSurfaceInput::Move(-1)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        assert_eq!(list.multi_selection(), &[3, 4]);
    }

    #[test]
    fn refresh_reanchors_when_the_cursor_row_leaves_the_flow() {
        let mut list = list();
        list.select_target(&3);
        list.toggle_selection(&6);
        list.set_content(vec![item(2), item(4), item(6), item(7)]);
        assert_eq!(list.multi_selection(), &[6]);
        assert_eq!(list.selected_target(), Some(&2));
        assert_eq!(list.selection_anchor.as_ref(), Some(&2));
    }

    #[test]
    fn keyboard_disjoint_selection_freezes_and_reanchors() {
        let mut list = list();
        list.select_target(&3);
        list.enter_visual_mode();
        list.delegate_operation(
            super::super::MediaListSurfaceInput::Move(1)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        list.toggle_selection(&4);
        list.delegate_operation(
            super::super::MediaListSurfaceInput::Move(2)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        assert_eq!(list.multi_selection(), &[3]);
        list.enter_visual_mode();
        list.delegate_operation(
            super::super::MediaListSurfaceInput::Move(0)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        assert_eq!(list.multi_selection(), &[3, 6]);
    }

    #[test]
    fn shift_range_unions_frozen_selection() {
        let mut list = list();
        list.select_target(&2);
        list.toggle_selection(&5);
        list.extend_selection_to(&4);
        assert_eq!(list.multi_selection(), &[2, 3, 4, 5]);
    }

    #[test]
    fn esc_after_freeze_clears_everything() {
        let mut list = list();
        list.select_target(&3);
        list.toggle_selection(&4);
        list.clear_selection();
        assert!(list.multi_selection().is_empty());
        assert!(!list.is_visual_mode());
    }
}
