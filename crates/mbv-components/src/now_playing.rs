//! The Emby workspace owners' shared now-playing row override
//! (hero-workspace-now-playing D2/D4).

use mbv_render::components::media_list::{ActiveProgress, MediaListRow, MediaSemanticState};
use mbv_ui_model::now_playing::WorkspaceNowPlaying;

/// The row whose target is the projection's Emby item becomes
/// `NowPlaying { progress }`; every other row keeps its derived state. A
/// `Chapter` or `None` projection overrides nothing.
pub(crate) fn apply_emby_now_playing(
    rows: &mut [MediaListRow<String>],
    now_playing: Option<&WorkspaceNowPlaying>,
) {
    let Some(WorkspaceNowPlaying::Item { item_id, progress }) = now_playing else {
        return;
    };
    for row in rows.iter_mut() {
        if let MediaListRow::Item {
            target,
            semantic_state,
            ..
        } = row
            && target == item_id.as_str()
        {
            *semantic_state = MediaSemanticState::NowPlaying {
                progress: progress.map(ActiveProgress::new),
            };
        }
    }
}
