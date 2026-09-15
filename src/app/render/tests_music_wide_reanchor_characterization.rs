//! Task 1.3 characterization: grouped Music Wide re-anchor behavior at the
//! feature-branch baseline `819dbd0c`, before the §2.1 canonical replacement.
//!
//! Grouped Music composes the canonical fixed-row owner today; the mounted
//! `MusicWorkspaceComponent` owns its destination state and row presentation.
//! and the wide rail is a bespoke row loop
//! (`render_wide_right_album_browser_with_ctx`). There is no `ViewportAnchor`
//! hand-off yet: a breakpoint flip keeps the component's local cursor/scroll,
//! and only an explicit shell `re_anchor` adopts the resting position.
//!
//! This single test is the reference the §2.1 replacement is checked against.

use super::test_helpers::{
    draw_mounted_frame, make_music_group_app, mounted_model_at, mounted_music_layout,
    mounted_music_scroll,
};
use super::*;
use crate::app::components::ComponentId;
use crate::app::shell::Model;
use crate::app::tests::make_item;
use crate::app::PanelFocus;

const ALBUM_COUNT: usize = 40;

fn music_app_many_albums() -> App {
    let mut app = make_music_group_app();
    app.panel_focus = PanelFocus::Library;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    for i in 1..ALBUM_COUNT {
        let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
        album.id = format!("album-extra-{i}");
        album.artist = "Alpha".into();
        level.items.push(album);
    }
    level.total_count = ALBUM_COUNT;
    app
}

fn album_cursor(model: &Model, _id: &ComponentId) -> usize {
    model.test_music_owner().album_cursor()
}

/// Shell-driven re-anchor: move the resting cursor/scroll and fire the
/// one-shot `music_workspace_reanchor` trigger, exactly as a navigation event
/// (group switch, saved-position restore, recursive activation) does.
fn re_anchor_to(model: &mut Model, cursor: usize, scroll: usize) {
    let level = model.app.libs[0].nav_stack.last_mut().unwrap();
    level.set_resting_cursor(cursor);
    level.set_resting_scroll(scroll);
    model.music_workspace_reanchor = true;
    model.push_music_workspace_content();
}

fn album_targets(model: &Model, want: usize) -> Vec<usize> {
    model.test_music_owner().album_target_rows(want)
}

#[test]
fn grouped_music_wide_reanchor_characterization() {
    let last = ALBUM_COUNT - 1;

    // ---- Wide: shell re-anchor to a mid-list album --------------------------
    let mut model = mounted_model_at(music_app_many_albums(), 160, 40);
    let _ = draw_mounted_frame(&mut model, 160, 40);
    let id = ComponentId::Library;
    assert!(
        super::test_helpers::mounted_music_wide_geometry(&model)
            .hero
            .width
            > 0
    );
    assert_eq!(
        album_cursor(&model, &id),
        0,
        "first mount re-anchors to the shell resting cursor"
    );

    re_anchor_to(&mut model, 6, 0);
    let _ = draw_mounted_frame(&mut model, 160, 40);

    assert_eq!(
        album_cursor(&model, &id),
        6,
        "the shell re-anchor adopts the resting cursor"
    );
    assert_eq!(
        mounted_music_scroll(&model),
        0,
        "a top-of-list album needs no scroll"
    );
    let selected_rows = album_targets(&model, 6);
    assert_eq!(
        selected_rows,
        vec![7],
        "wide rail publishes one Album(6) target below the artist header + Album(0..=5)"
    );
    {
        let geometry = super::test_helpers::mounted_music_wide_geometry(&model);
        let rect = geometry.selected.expect("wide selected-row rect published");
        assert_eq!(
            rect.y - geometry.list_area.y,
            selected_rows[0] as u16,
            "selected-row screen offset agrees with the panel list geometry"
        );
    }

    // ---- Wide: re-anchor to the last album settles a non-zero scroll --------
    re_anchor_to(&mut model, last, 0);
    let _ = draw_mounted_frame(&mut model, 160, 40);
    assert_eq!(album_cursor(&model, &id), last);
    let wide_scroll = mounted_music_scroll(&model);
    assert!(
        wide_scroll > 0,
        "the bottom album forces a non-zero album_scroll write-back"
    );
    assert_eq!(album_targets(&model, last).len(), 1);

    // ---- Wide -> Narrow -> Wide: bare presentation flips keep the selection -
    // No shell re-anchor fires, so the kept-mounted component holds its
    // album_cursor across the round trip and the wide scroll recomputes to the
    // identical bottom-anchored offset.
    let _ = draw_mounted_frame(&mut model, 60, 30);
    assert_eq!(
        album_cursor(&model, &id),
        last,
        "the breakpoint flip keeps the component's local cursor"
    );
    let _ = draw_mounted_frame(&mut model, 160, 40);
    assert!(
        super::test_helpers::mounted_music_wide_geometry(&model)
            .hero
            .width
            > 0
    );
    assert_eq!(album_cursor(&model, &id), last);
    assert_eq!(
        mounted_music_scroll(&model),
        wide_scroll,
        "wide album_scroll is recomputed to the identical bottom-anchored offset"
    );
    assert_eq!(album_targets(&model, last).len(), 1);

    // ---- Normal/Narrow: selected target + on-screen selected row -----------
    let mut narrow_model = mounted_model_at(music_app_many_albums(), 60, 30);
    let _ = draw_mounted_frame(&mut narrow_model, 60, 30);
    re_anchor_to(&mut narrow_model, 6, 0);
    let narrow = draw_mounted_frame(&mut narrow_model, 60, 30);
    let n_id = ComponentId::Library;
    assert_eq!(album_cursor(&narrow_model, &n_id), 6);
    assert!(
        narrow.contains("Album 06"),
        "narrow hero paints the selected album:\n{narrow}"
    );
    assert_eq!(
        album_targets(&narrow_model, 6).len(),
        1,
        "narrow publishes exactly one selected-album target"
    );
    assert!(
        mounted_music_layout(&narrow_model)
            .selected_item_rect
            .is_some(),
        "narrow publishes the selected-row rect"
    );

    // ---- Shell re-anchor resets cursor + scroll to the resting position ----
    re_anchor_to(&mut model, 0, 0);
    let _ = draw_mounted_frame(&mut model, 160, 40);
    assert_eq!(
        album_cursor(&model, &id),
        0,
        "an explicit shell re-anchor adopts the resting cursor unconditionally"
    );
    assert_eq!(
        mounted_music_scroll(&model),
        0,
        "the re-anchored scroll returns to the resting offset"
    );
    assert_eq!(album_targets(&model, 0), vec![1]);
}
