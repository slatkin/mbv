use super::playback::PendingQueueAction;

#[derive(Default)]
pub(in crate::app) struct QueueDeferrals {
    local_play: Option<LocalPlay>,
}

struct LocalPlay(PendingQueueAction);

impl QueueDeferrals {
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
    pub(in crate::app) fn has_local_play(&self) -> bool {
        self.local_play.is_some()
    }
}
