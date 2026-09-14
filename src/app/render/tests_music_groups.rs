use super::test_helpers::*;
use super::*;
use crate::app::shell::Model;
use crate::app::tests::make_item;
use ratatui::layout::Rect;

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
    // Read the complete flow retained by the mounted InlineMediaBrowser;
    // legacy row maps are no longer populated by this owner.
    assert!(
        mounted_music_flow_targets(&model)
            .iter()
            .any(Option::is_none),
        "expected a non-album row (artist header) in the retained flow"
    );
}

#[test]
fn narrow_grouped_music_replaces_selected_album_row_with_hero_detail() {
    // Task 3.2: the selected album's row is replaced by the Model A hero
    // (title/meta/art), not an inline track table -- see
    // `tests_music_characterization.rs` for the text-level assertion that
    // the track table and action hint no longer render.
    let mut app = make_music_group_app();
    let tracks: Vec<mbv_core::api::EmbyItem> = (0..2)
        .map(|i| {
            let mut track = make_item(&format!("Track {}", i + 1), "Audio");
            track.id = format!("track-{}", i + 1);
            track.index_number = (i + 1) as i64;
            track
        })
        .collect();
    app.album_tracks_cache.insert("album-1".into(), tracks);
    let (model, output) = narrow_music_frame(app, 30);
    let layout = mounted_music_layout(&model);

    assert!(
        output.contains("First Album"),
        "selected album hero must render its title"
    );
    assert_eq!(
        mounted_music_album_target_rows(&model, 0).len(),
        1,
        "the selected album must publish one replacement parent target"
    );
    let selected = layout.selected_item_rect.expect("selected target");
    assert!(
        layout.hero_area.contains((selected.x, selected.y).into())
            && layout
                .hero_area
                .contains((selected.right() - 1, selected.bottom() - 1).into()),
        "the selected album target is owned by the replacement hero"
    );
}

#[test]
fn narrow_grouped_music_paints_the_album_hero_content() {
    // The album hero block's own content still paints over its placement (the
    // former `▁`/`▔` frame rows are gone; the block's internal spacing is not
    // re-asserted here).
    let app = make_music_group_app();
    let (model, output) = narrow_music_frame(app, 30);
    let layout = mounted_music_layout(&model);

    assert!(
        output.contains("First Album"),
        "the hero's own content still paints:\n{output}"
    );
    assert!(
        layout.hero_area.height > 0,
        "the album hero block is admitted: {:?}",
        layout.hero_area
    );
}

#[test]
fn short_grouped_music_restores_the_ordinary_selected_album_row() {
    // Measure the shared inline hero's admitted block height at a tall
    // viewport, then render with less room than that: after chrome
    // reservation the list box is strictly shorter than the block, so the
    // ordinary selected row must be restored.
    let mut tall_app = make_music_group_app();
    tall_app.image_protocol_enabled = true;
    let (tall_model, _) = narrow_music_frame(tall_app, 30);
    let admitted = mounted_music_layout(&tall_model).hero_area.height;
    assert!(admitted > 0, "the fixture admits the inline hero when tall");

    let mut app = make_music_group_app();
    app.image_protocol_enabled = true;
    let (model, output) = narrow_music_frame(app, admitted + 1);
    let layout = mounted_music_layout(&model);

    assert!(output.contains("First Album"));
    assert_eq!(layout.hero_area, Rect::default());
    assert!(
        layout.selected_item_rect.is_some(),
        "the ordinary selected album row remains targetable"
    );
    assert_eq!(mounted_music_album_target_rows(&model, 0).len(), 1);
}

#[test]
fn grouped_hero_art_follows_album_focus() {
    let mut album_app = make_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    album_app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second);
    album_app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .set_resting_cursor(1);
    album_app.image_protocol_enabled = true;
    // 60x30 so the list below the album hero still shows both albums.
    let (model, out) = narrow_music_frame(album_app, 30);
    assert!(out.contains("First Album"));
    // The hero renders the *selected* album's art (portrait `:P`), never a
    // square collage tile (`:sq`).
    assert!(model.app.card_image_loading.contains("album-2:P"));
    // Narrow grouped Music pre-warms neighbouring album art through the shell;
    // the row painter itself still emits only the selected album's hero art.
    assert!(!model.app.card_image_loading.contains("album-2:sq"));
}

#[test]
fn grouped_music_maps_reordered_non_contiguous_album_source() {
    let mut app = make_music_group_app();
    let mut beta = make_item("Beta Album", "MusicAlbum");
    beta.id = "album-beta".into();
    beta.artist = "Beta".into();
    let mut alpha_other = make_item("Alpha Other", "MusicAlbum");
    alpha_other.id = "album-alpha-other".into();
    alpha_other.artist = "Alpha".into();
    let mut selected = make_item("Selected Album", "MusicAlbum");
    selected.id = "album-selected".into();
    selected.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .extend([beta, alpha_other, selected]);
    let cursor = 3;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .set_resting_cursor(cursor);
    let (model, rendered) = narrow_music_frame(app, 30);
    let layout = mounted_music_layout(&model);

    assert!(rendered.contains("Selected Album"));
    assert_eq!(layout.selected_item_rect, Some(layout.hero_area));
    assert_eq!(mounted_music_album_target_rows(&model, cursor).len(), 1);
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
