use super::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_core::audiobookshelf::{AudiobookshelfLibrary, AudiobookshelfShow};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;

pub(super) fn narrow_grid_component_state() -> AudiobookshelfBrowseState {
    let library = AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state = AudiobookshelfBrowseState::new(library);
    state.append_page(
        0,
        20,
        12,
        (0..12)
            .map(|i| AudiobookshelfShow {
                library_item_id: format!("show-{i}"),
                title: format!("Show {i}"),
                author: None,
                description: None,
                cover_path: None,
            })
            .collect(),
    );
    state.select(2);
    state
}
pub(super) fn view_narrow(component: &mut AudiobookshelfPodcastComponent, width: u16, height: u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
}
