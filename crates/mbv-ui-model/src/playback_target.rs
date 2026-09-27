/// The now-playing status word's source (design D10; folded change D2):
/// derived once per frame next to `effective_playback_state()` and consumed
/// by the Queue playback panel's header row and the idle collapse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NowPlayingStatus {
    /// Active and not paused.
    Playing,
    /// Active and paused.
    Paused,
    /// No transport active. A stale `paused` flag on an inactive transport
    /// is unreachable in practice and reads as `Idle`.
    Idle,
}
