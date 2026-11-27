mod ctrl {
    use mbv_ctrl::player::{PlayerCommand, PlayerStatus};
    use mbv_ctrl::*;

    mod conformance;
    mod handshake;
    mod queue;
    mod wire;

    /// A minimal Emby item for queue wire shapes; test modules serialize it
    /// and strip fields to pin additive-decode tolerances.
    pub(super) fn stub_media_item() -> mbv_emby_model::EmbyItem {
        mbv_emby_model::EmbyItem {
            id: "item1".into(),
            name: "Test Item".into(),
            item_type: "Episode".into(),
            is_folder: false,
            child_count: None,
            media_type: "Video".into(),
            collection_type: String::new(),
            runtime_ticks: 0,
            played: false,
            is_favorite: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            parent_id: String::new(),
            path: String::new(),
            artist: String::new(),
            track_artist: String::new(),
            artist_items: Vec::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            studios: Vec::new(),
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genres: Vec::new(),
            people: Vec::new(),
            external_urls: Vec::new(),
            playlist_item_id: String::new(),
            image_tags: mbv_emby_model::EmbyImageTags::default(),
        }
    }
}
