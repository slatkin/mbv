use super::*;
use crate::app::render::make_music_group_app;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn narrow_grouped_music_prewarms_neighbour_album_images() {
    let mut model = Model::new(make_music_group_app());
    model.app.image_protocol_enabled = true;
    let albums = &mut model.app.libs[0].nav_stack[1].items;
    albums[0].name = "Album 1".into();
    for number in 2..=7 {
        let mut album = make_item(&format!("Album {number}"), "MusicAlbum");
        album.id = format!("album-{number}");
        album.artist = "Alpha".into();
        albums.push(album);
    }
    model.app.libs[0].nav_stack[1].set_resting_cursor(2);
    model.app.layout.main.left_area = ratatui::layout::Rect::new(0, 0, 81, 20);
    model.sync_music_workspace();

    let mut terminal = Terminal::new(TestBackend::new(81, 20)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();

    assert!(model.app.card_image_loading.contains("album-2:P"));
    assert!(model.app.card_image_loading.contains("album-4:P"));
    assert!(!model.app.card_image_loading.contains("album-1:P"));
    assert!(!model.app.card_image_loading.contains("album-7:P"));
}
