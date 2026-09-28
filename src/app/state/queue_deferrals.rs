use super::playback::{PendingQueueAction, ReplacementExecutor};

#[derive(Default)]
pub(in crate::app) struct QueueDeferrals {
    save: Option<SaveDeferral>,
    gate: Option<GatedReplacement>,
    local_play: Option<LocalPlay>,
}

enum SaveDeferral {
    AwaitingAnswer(PendingQueueAction),
    AwaitingSave {
        mutation_id: u64,
        action: PendingQueueAction,
    },
}

struct GatedReplacement(PendingQueueAction, ReplacementExecutor);

struct LocalPlay(PendingQueueAction);

impl QueueDeferrals {
    pub(in crate::app) fn defer_for_save_answer(&mut self, action: PendingQueueAction) {
        self.save = Some(SaveDeferral::AwaitingAnswer(action));
    }

    pub(in crate::app) fn save_answer_is_play(&self) -> bool {
        matches!(
            self.save,
            Some(SaveDeferral::AwaitingAnswer(
                PendingQueueAction::PlayItems { .. }
            ))
        )
    }

    pub(in crate::app) fn take_on_discard(&mut self) -> Option<PendingQueueAction> {
        match self.save.take() {
            Some(SaveDeferral::AwaitingAnswer(action)) => Some(action),
            Some(SaveDeferral::AwaitingSave {
                mutation_id,
                action,
            }) => {
                self.save = Some(SaveDeferral::AwaitingSave {
                    mutation_id,
                    action,
                });
                None
            }
            None => None,
        }
    }

    pub(in crate::app) fn bind_to_save(&mut self, mutation_id: Option<u64>) {
        self.save = match (self.save.take(), mutation_id) {
            (Some(SaveDeferral::AwaitingAnswer(action)), Some(mutation_id)) => {
                Some(SaveDeferral::AwaitingSave {
                    mutation_id,
                    action,
                })
            }
            _ => None,
        };
    }

    pub(in crate::app) fn cancel_save_answer(&mut self) {
        self.save = None;
    }

    pub(in crate::app) fn take_on_save_complete(
        &mut self,
        mutation_id: u64,
    ) -> Option<PendingQueueAction> {
        if matches!(self.save, Some(SaveDeferral::AwaitingSave { mutation_id: bound, .. }) if bound == mutation_id)
        {
            match self.save.take() {
                Some(SaveDeferral::AwaitingSave { action, .. }) => Some(action),
                _ => None,
            }
        } else {
            None
        }
    }

    pub(in crate::app) fn hold_gated_replacement(
        &mut self,
        action: PendingQueueAction,
        executor: ReplacementExecutor,
    ) {
        self.gate = Some(GatedReplacement(action, executor));
    }

    pub(in crate::app) fn take_confirmed_replacement(
        &mut self,
    ) -> Option<(PendingQueueAction, ReplacementExecutor)> {
        self.gate
            .take()
            .map(|GatedReplacement(action, executor)| (action, executor))
    }

    pub(in crate::app) fn cancel_gated_replacement(&mut self) {
        self.gate = None;
    }

    pub(in crate::app) fn hold_local_play(&mut self, action: PendingQueueAction) {
        self.local_play = Some(LocalPlay(action));
    }

    pub(in crate::app) fn take_confirmed_local_play(&mut self) -> Option<PendingQueueAction> {
        self.local_play.take().map(|LocalPlay(action)| action)
    }

    pub(in crate::app) fn cancel_local_play(&mut self) {
        self.local_play = None;
    }

    #[cfg(test)]
    pub(in crate::app) fn is_save_deferred(&self) -> bool {
        self.save.is_some()
    }

    #[cfg(test)]
    pub(in crate::app) fn has_gated_replacement(&self) -> bool {
        self.gate.is_some()
    }

    #[cfg(test)]
    pub(in crate::app) fn has_local_play(&self) -> bool {
        self.local_play.is_some()
    }
}
