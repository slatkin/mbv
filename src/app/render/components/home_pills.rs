// Home section selector painting is centralized with the other Home components.
use crate::app::render::components::widgets::{render_pill_bar, PillBar};
use crate::app::types_playback::HomeLatestSource;
use crate::app::ui_util::*;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::Frame;

/// Renders the Home pill bar for the given (already-resolved) `labels`/
/// `selected_section`, returning the click targets. Pure geometry/painting;
/// callers own resolving which section is selected (restoring a persisted
/// preference is a one-time, not per-render, concern -- see
/// `App::render_home_list`'s prefs-restore step for the legacy render path
/// and `HomeComponent::restore_section` for the component's).
pub(in crate::app::render) fn render_home_pills(
    f: &mut Frame,
    area: Rect,
    labels: &[(usize, String)],
    selected_section: usize,
) -> Vec<(Rect, usize)> {
    if area.width == 0 || area.height == 0 || labels.is_empty() {
        return Vec::new();
    }
    const MAX_LABEL: usize = 18;
    let selected_pos = labels
        .iter()
        .position(|(section_idx, _)| *section_idx == selected_section)
        .unwrap_or(0);
    // Pre-truncated pill labels; ids are the section indices used as click
    // targets, distinct from the pill's display position.
    let label_strs: Vec<String> = labels
        .iter()
        .map(|(_, label)| trunc_str(label, MAX_LABEL).to_string())
        .collect();
    let ids: Vec<usize> = labels.iter().map(|(section_idx, _)| *section_idx).collect();
    render_pill_bar(
        f,
        area,
        PillBar {
            labels: &label_strs,
            ids: &ids,
            selected_pos,
            prefix: Some(" ⌘ "),
        },
    )
}

/// Builds the Home pill label list: "Continue" for section 0, then every
/// `home.latest` section's title (even an empty section renders as a real,
/// discoverable pill).
pub(in crate::app::render) fn home_pill_labels(
    latest: &[(String, HomeLatestSource, Vec<QueueItem>)],
) -> Vec<(usize, String)> {
    let mut labels: Vec<(usize, String)> = vec![(0, "Continue".to_string())];
    for (idx, (title, _source, _items)) in latest.iter().enumerate() {
        labels.push((idx + 1, title.clone()));
    }
    labels
}
