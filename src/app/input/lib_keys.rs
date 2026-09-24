use crate::app::App;

impl App {
    pub(in crate::app) fn adjust_volume(&mut self, delta: i64) {
        self.playback_target().adjust_volume(self, delta);
    }
}
