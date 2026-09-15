use super::test_helpers::*;
use super::*;
use crate::app::shell::Model;
use crate::app::tests::make_item;

/// Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
/// now (task 3.8): drive the real `Model::draw_frame` shell path and read the
/// component's own published legacy chrome geometry via `mounted_music_layout`.
fn narrow_music_frame(app: App, height: u16) -> (Model, String) {
    let mut model = mounted_model_at(app, 60, height);
    let output = draw_mounted_frame(&mut model, 60, height);
    (model, output)
}

#[test]
fn selectable_artist_headers_are_typed_row_targets() {
    let mut app = make_music_group_app();
    let mut alpha_album2 = make_item("Second Alpha Album", "MusicAlbum");
    alpha_album2.id = "album-1b".into();
    alpha_album2.artist = "Alpha".into();
    alpha_album2.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(alpha_album2);
    let mut beta_album = make_item("Beta Album", "MusicAlbum");
    beta_album.id = "album-2".into();
    beta_album.artist = "Beta".into();
    beta_album.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(beta_album);

    let (model, out) = narrow_music_frame(app, 20);
    assert!(
        out.contains("Alpha") && out.contains("Beta"),
        "expected both artist headers to render:\n{out}"
    );
    // Artist headers are display-only and must not appear as row targets.
    // Read the complete fixed-row flow retained by the mounted owner.
    assert!(
        mounted_music_flow_targets(&model)
            .iter()
            .any(Option::is_none),
        "expected a non-album row (artist header) in the retained flow"
    );
}

#[test]
fn wide_music_panel_uses_shared_skeleton_geometry() {
    let mut model = mounted_model_at(make_music_group_app(), 160, 24);
    let rendered = draw_mounted_frame(&mut model, 160, 24);
    assert!(rendered.contains("First Album"));
    let geometry = super::test_helpers::mounted_music_wide_geometry(&model);
    assert!(geometry.list_panel.width > 0);
    assert!(geometry.hero.width > 0);
}
