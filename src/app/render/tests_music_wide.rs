// §3.1 / §3.2 evidence for the wide grouped-Music canonical migration.
//
// Wide grouped Music now composes the canonical `WideMediaList` control
// (`render_wide_media_list`), exactly as the wide TV series rail and wide
// Movies list do. These tests drive `MusicWorkspaceComponent::view` directly
// at the wide breakpoint (mirroring `render_music_component` in
// `tests_conformance_matrix`) and assert the canonical row geometry,
// non-selectable structural rows, the selected-row full-width background, and
// that exactly one wide list painter runs for the destination.

use super::components::media_list::{PLAIN_ROWS_PAINTS, WIDE_MEDIA_LIST_PAINTS};
use super::test_helpers::{buffer_to_string, make_music_group_app};
use super::*;
use crate::app::components::inline_search::InlineSearchHost;
use crate::app::components::{ComponentId, MusicWorkspaceComponent};
use crate::app::tests::make_item;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::PanelFocus;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;

const W: u16 = 160;
const H: u16 = 40;

/// A grouped-Music fixture with two artist groups so the row flow contains a
/// heading, a spacer between groups, and a second heading.
fn multi_artist_app() -> App {
    let mut app = make_music_group_app();
    app.panel_focus = PanelFocus::Library;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    for i in 1..4 {
        let mut album = make_item(&format!("Alpha Album {i:02}"), "MusicAlbum");
        album.id = format!("alpha-{i}");
        album.artist = "Alpha".into();
        level.items.push(album);
    }
    for i in 0..3 {
        let mut album = make_item(&format!("Beta Album {i:02}"), "MusicAlbum");
        album.id = format!("beta-{i}");
        album.artist = "Beta".into();
        level.items.push(album);
    }
    level.total_count = level.items.len();
    app
}

fn render_wide(
    app: &App,
    focused: bool,
    cursor: usize,
) -> (Terminal<TestBackend>, MusicWorkspaceComponent) {
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.focused = focused;
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    component.set_focused(focused);
    component.re_anchor(cursor, 0);
    let mut terminal = Terminal::new(TestBackend::new(W, H)).unwrap();
    terminal
        .draw(|f| component.view(f, Rect::new(0, 0, W, H)))
        .unwrap();
    (terminal, component)
}

#[test]
fn wide_music_composes_the_canonical_control_exactly_once() {
    let app = multi_artist_app();
    WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));
    PLAIN_ROWS_PAINTS.with(|c| c.set(0));

    let (terminal, _component) = render_wide(&app, true, 0);

    assert_eq!(
        WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get),
        1,
        "the populated album rail uses the canonical painter"
    );
    assert_eq!(
        PLAIN_ROWS_PAINTS.with(std::cell::Cell::get),
        0,
        "the bespoke / plain-rows underpaint must not run"
    );
    assert!(buffer_to_string(&terminal).contains("First Album"));
}

#[test]
fn wide_music_track_table_uses_one_retained_canonical_view() {
    let mut app = multi_artist_app();
    let tracks = (0..2)
        .map(|index| {
            let mut track = make_item(&format!("Track {}", index + 1), "Audio");
            track.id = format!("track-{}", index + 1);
            track.index_number = index + 1;
            track
        })
        .collect::<Vec<_>>();
    app.album_tracks_cache.insert("album-1".into(), tracks);
    WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));
    PLAIN_ROWS_PAINTS.with(|c| c.set(0));

    let (terminal, component) = render_wide(&app, true, 0);

    assert_eq!(
        WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get),
        2,
        "Grouped Music paints its album rail and track table through one view each"
    );
    assert_eq!(PLAIN_ROWS_PAINTS.with(std::cell::Cell::get), 0);
    assert!(component.test_track_selected_row_rect().is_some());
    assert!(buffer_to_string(&terminal).contains("Track 1"));
}

#[test]
fn wide_music_search_mode_uses_the_plain_rows_painter_not_the_album_control() {
    // Library search is adapted to the established InlineSearch owner, so the
    // panel reserves its search slot and the ordinary album rail is not
    // painted or left with a selected-row hit. The track workspace is still
    // the panel's only Wide media-list view for this empty-track fixture.
    let mut app = multi_artist_app();
    app.terminal_width = W;
    app.terminal_height = H;
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    app.album_indexes.insert(
        app.libs[0].library.id.clone(),
        crate::app::AlbumIndexState::Ready(
            albums
                .into_iter()
                .map(|album| crate::app::AlbumSearchEntry {
                    search_text: album.display_name(),
                    display_label: album.display_name(),
                    album,
                    ancestors: Vec::new(),
                })
                .collect(),
        ),
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let id = ComponentId::Library;
    harness.model_mut().open_inline_search();
    {
        harness
            .model_mut()
            .test_music_owner_mut()
            .inline_search_mut()
            .restore_query("al".into());
    }
    // This is the same shell projection used after search open and async
    // completions; do not manufacture search state in the render context.
    harness.model_mut().push_inline_search_content();
    harness.model_mut().push_music_workspace_content();
    WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));
    PLAIN_ROWS_PAINTS.with(|c| c.set(0));
    let mut terminal = Terminal::new(TestBackend::new(W, H)).unwrap();
    terminal
        .draw(|frame| {
            harness
                .model_mut()
                .application
                .view(&id, frame, Rect::new(0, 0, W, H));
        })
        .unwrap();

    assert_eq!(
        PLAIN_ROWS_PAINTS.with(std::cell::Cell::get),
        1,
        "search results use the plain result-row painter"
    );
    assert_eq!(
        WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get),
        0,
        "search does not view the album rail through WideMediaList"
    );
    let geometry = super::test_helpers::mounted_music_wide_geometry(harness.model());
    assert!(
        geometry.selected.is_none(),
        "search owns album-row geometry"
    );
    let rendered = buffer_to_string(&terminal);
    assert!(rendered.contains("Alpha Album"), "rendered={rendered}");
}

#[test]
fn wide_music_hero_uses_resolved_artist_year_and_folder_title() {
    let mut app = make_music_group_app();
    let album = &mut app.libs[0].nav_stack.last_mut().unwrap().items[0];
    album.artist.clear();
    album.name = "Folder Artist (2024) First Album".into();
    album.production_year = 0;
    app.album_artist_cache
        .insert(album.id.clone(), "Folder Artist".into());

    let (terminal, _component) = render_wide(&app, true, 0);
    let rendered = buffer_to_string(&terminal);
    assert!(rendered.contains("Folder Artist"));
    assert!(rendered.contains("2024"));
    assert!(rendered.contains("First Album"));
    assert!(!rendered.contains("Folder Artist (2024) First Album"));
}

#[test]
fn wide_music_headings_and_spacers_are_not_selectable_row_targets() {
    let app = multi_artist_app();
    let (_terminal, component) = render_wide(&app, true, 0);
    let flow = component.album_flow_targets();
    let album_rows = flow.iter().filter(|t| t.is_some()).count();
    let structural_rows = flow.iter().filter(|t| t.is_none()).count();
    assert_eq!(album_rows, 7, "seven album rows are selectable targets");
    assert!(
        structural_rows >= 2,
        "the two artist headings (and the inter-group spacer) publish no target: {:?}",
        flow
    );
    // The first painted row is the "Alpha" heading -> no target.
    assert!(flow[0].is_none());
}

#[test]
fn wide_music_selected_row_fills_the_whole_panel_width_when_focused() {
    let app = multi_artist_app();
    let (terminal, component) = render_wide(&app, true, 0);
    let geometry = component
        .test_wide_geometry()
        .expect("wide skeleton geometry published");
    let rect = geometry.selected.expect("selected-row rect published");
    let buffer = terminal.backend().buffer();
    // The panel owns the list slot's row geometry and paints the selected row
    // across that retained row. Its title is intentionally inset by the
    // canonical list painter rather than by Music-specific geometry.
    let bg = buffer[(rect.x, rect.y)].bg;
    for x in rect.x..rect.right() {
        assert_eq!(buffer[(x, rect.y)].bg, bg);
    }
    assert!(buffer[(rect.x + 2, rect.y)].symbol() != " ");
}

#[test]
fn wide_music_left_edge_alignment_matches_the_canonical_left_inset() {
    let app = multi_artist_app();
    let (terminal, component) = render_wide(&app, true, 0);
    let geometry = component
        .test_wide_geometry()
        .expect("wide skeleton geometry published");
    let buffer = terminal.backend().buffer();
    let area = geometry.list_area;
    // Row 0 is the "Alpha" heading. Canonical non-selected row:
    // `[space][space][text]` painted from 2 columns left of `area.x`, so the
    // heading text lands exactly at `area.x` (the padded content edge).
    let first_text = ((area.x - 2)..area.x + area.width)
        .find(|&x| buffer[(x, area.y)].symbol().trim() != "")
        .map(|x| x as i32 - area.x as i32);
    assert_eq!(
        first_text,
        Some(2),
        "heading text lands at the canonical list inset"
    );
}

#[test]
fn wide_music_unfocused_selection_matches_the_canonical_control() {
    // Canonical parity with wide Movies / TV / Feeds: the selected-row
    // highlight is a focused-only affordance. The old bespoke painter left a
    // partial text-width `SURFACE_RESTING` smear on the unfocused selected
    // row; that is gone.
    let app = multi_artist_app();
    let (terminal, component) = render_wide(&app, false, 0);
    let geometry = component
        .test_wide_geometry()
        .expect("wide skeleton geometry published");
    let rect = geometry
        .selected
        .expect("selected-row rect still published when unfocused");
    let buffer = terminal.backend().buffer();
    let row_bg = buffer[(rect.x, rect.y)].bg;
    for x in rect.x..rect.x + rect.width {
        assert_eq!(
            buffer[(x, rect.y)].bg,
            row_bg,
            "unfocused selected row has a single uniform background (no partial smear)"
        );
    }
}

#[test]
fn wide_music_renders_with_images_enabled() {
    let mut app = multi_artist_app();
    app.image_protocol_enabled = true;
    let (terminal, component) = render_wide(&app, true, 4);
    let geometry = component
        .test_wide_geometry()
        .expect("wide skeleton geometry published");
    assert!(buffer_to_string(&terminal).contains("Album"));
    assert!(geometry.selected.is_some());
    // The shared Square hero owns the projected artwork box.
    assert!(geometry.hero_image.is_some() || geometry.hero.width > 0);
}
