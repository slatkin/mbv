use crate::app::App;

impl App {
    pub(in crate::app) fn adjust_volume(&mut self, delta: i64) {
        crate::app::LocalPlaybackTarget::adjust_volume(self, delta);
    }
}
