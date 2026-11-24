use crate::app::BrowseLevel;
use mbv_emby_model::EmbyItem;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

mod group_loading;
mod group_nav;
mod mark_played_keeps_row;
mod mark_played_state;
mod menu;

/// The unfiltered root level of the `YouTube` home-video library.
fn youtube_root_level(rows: ServerRows, items: Vec<EmbyItem>, loading: bool) -> BrowseLevel {
    BrowseLevel {
        rows,
        parent_id: "lib-youtube".into(),
        title: "YouTube".into(),
        items,
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}
