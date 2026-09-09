use super::music_workspace::MusicWorkspaceComponent;

impl MusicWorkspaceComponent {
    pub(in crate::app) fn track_selected_row(&self) -> Option<usize> {
        self.track_list.selected_display_row()
    }

    pub(in crate::app) fn album_selected_row_rect(&self) -> Option<ratatui::layout::Rect> {
        self.wide_list.current_selected_row_rect()
    }

    pub(in crate::app) fn album_flow_targets(&self) -> Vec<Option<String>> {
        if self.active_is_wide() {
            (0..self.wide_list.current_flow_len().unwrap_or_default())
                .map(|row| {
                    self.wide_list
                        .current_flow_target_at(row)
                        .flatten()
                        .cloned()
                })
                .collect()
        } else {
            (0..self.narrow_list.current_flow_len().unwrap_or_default())
                .map(|row| {
                    self.narrow_list
                        .current_flow_target_at(row)
                        .flatten()
                        .cloned()
                })
                .collect()
        }
    }

    pub(in crate::app) fn album_target_rows(&self, target: usize) -> Vec<usize> {
        let wanted = self.context.album_targets[target].clone();
        self.album_flow_targets()
            .iter()
            .enumerate()
            .filter_map(|(row, value)| (value.as_deref() == Some(wanted.as_str())).then_some(row))
            .collect()
    }

    pub(in crate::app) fn album_flow_offset(&self) -> usize {
        if self.active_is_wide() {
            self.wide_list.current_flow_offset().unwrap_or_default()
        } else {
            self.narrow_list.current_flow_offset().unwrap_or_default()
        }
    }

    pub(in crate::app) fn album_tracks_loading(&self) -> bool {
        self.context.album_tracks_loading
    }
}
