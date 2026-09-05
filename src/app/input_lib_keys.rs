use super::App;

impl App {
    pub(super) fn adjust_volume(&mut self, delta: i64) {
        self.playback_target().adjust_volume(self, delta);
    }
}
