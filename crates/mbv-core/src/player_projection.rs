#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectionMode {
    Eager,
    ActiveFile,
}

/// Owner-local projection coordinates. Canonical slots remain authoritative;
/// active-file mode intentionally has no index map back from mpv.
struct QueueProjection {
    mode: ProjectionMode,
}

impl QueueProjection {
    fn eager() -> Self {
        Self {
            mode: ProjectionMode::Eager,
        }
    }

    fn activate_for(&mut self, queue: &PlaybackQueue) {
        if queue.has_audiobookshelf_entries() {
            self.activate();
        }
    }

    fn activate(&mut self) {
        self.mode = ProjectionMode::ActiveFile;
    }

    fn is_active_file(&self) -> bool {
        self.mode == ProjectionMode::ActiveFile
    }

    #[cfg(test)]
    fn materialized_slots(&self, queue: &PlaybackQueue) -> Vec<QueueSlotId> {
        if self.is_active_file() {
            queue.active_slot_id().into_iter().collect()
        } else {
            queue.slots().iter().map(|slot| slot.slot_id).collect()
        }
    }

    fn observe_playlist_pos(&self, _queue: &mut PlaybackQueue, _pos: i64) {}

    fn observe_playlist_count(&self, _queue: &mut PlaybackQueue, _count: usize) {}

    #[cfg(test)]
    fn advance(&self, queue: &mut PlaybackQueue) -> Option<QueueSlotId> {
        let next = queue.active_index()?.checked_add(1)?;
        let slot_id = queue.slots().get(next)?.slot_id;
        let _ = queue.set_active_slot(slot_id);
        Some(slot_id)
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use crate::playback_queue::{AudiobookshelfQueueItem, QueueItem};

    fn episode(id: &str) -> QueueItem {
        QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
            library_item_id: "show".into(),
            episode_id: id.into(),
            title: id.into(),
            show_title: None,
            author: None,
            duration_ticks: Some(10),
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        })
    }

    #[test]
    fn active_file_projection_keeps_canonical_slot_identity_for_mutations() {
        let mut queue = PlaybackQueue::from_queue_items(
            vec![
                episode("duplicate"),
                episode("middle"),
                episode("duplicate"),
            ],
            Some(0),
        );
        let first = queue.slots()[0].slot_id;
        let middle = queue.slots()[1].slot_id;
        let duplicate = queue.slots()[2].slot_id;
        let mut projection = QueueProjection::eager();
        projection.activate_for(&queue);
        assert_eq!(projection.materialized_slots(&queue), vec![first]);

        let appended = queue.append(episode("appended"));
        let _ = queue.move_slot(appended, 1);
        assert_eq!(projection.materialized_slots(&queue), vec![first]);
        let _ = queue.remove_slot(middle);
        assert!(queue.slot(duplicate).is_some());
        let consumed = queue.consume_slot(first);
        assert!(matches!(consumed, QueueMutationResult::Applied(_)));
        assert!(
            queue.slot(duplicate).is_some(),
            "duplicate content keeps its slot"
        );
        let _ = queue.set_active_slot(duplicate);
        assert_eq!(projection.materialized_slots(&queue), vec![duplicate]);

        let before: Vec<_> = queue.slots().iter().map(|slot| slot.slot_id).collect();
        projection.observe_playlist_pos(&mut queue, 99);
        projection.observe_playlist_count(&mut queue, 0);
        assert_eq!(
            before,
            queue
                .slots()
                .iter()
                .map(|slot| slot.slot_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(queue.active_slot_id(), Some(duplicate));

        let _ = queue.set_active_slot(appended);
        let next = projection.advance(&mut queue).unwrap();
        assert_eq!(next, duplicate);
        assert_eq!(projection.materialized_slots(&queue), vec![duplicate]);
        queue.replace(vec![episode("replacement")]);
        let replacement_slot = queue.slots()[0].slot_id;
        let _ = queue.set_active_slot(replacement_slot);
        projection.activate_for(&queue);
        assert_eq!(projection.materialized_slots(&queue).len(), 1);
        queue.clear();
        assert!(projection.materialized_slots(&queue).is_empty());
    }
}
