// §3.1 evidence for the narrow grouped-Music canonical migration.
//
// Narrow grouped Music now composes the persistent canonical
// `InlineMediaBrowser<String>` (`render_inline_media_browser`), exactly as the
// narrow TV series list does. These tests drive `MusicWorkspaceComponent::view`
// at the narrow breakpoint (and the full mounted `Model` for the breakpoint
// hand-off), asserting one-column geometry, non-selectable structural rows,
// selected-row replacement admission / ordinary-row fallback, the focused
// selection affordance, image-bearing fixtures, ordinary-refresh target
// retention, and the target/offset `ViewportAnchor` round trip across
// Wide -> Narrow -> Wide.

use super::test_helpers::{
    buffer_to_string, draw_mounted_frame, make_music_group_app, mounted_model_at,
    mounted_music_scroll,
};
use super::*;
use crate::app::components::{ComponentId, MusicWorkspaceComponent};
use crate::app::shell::Model;
use crate::app::tests::make_item;
use crate::app::PanelFocus;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

const NW: u16 = 60;
const NH: u16 = 30;

/// A grouped-Music fixture with two artist groups (heading, inter-group
/// spacer, second heading) plus enough albums to overflow a narrow viewport.
fn multi_artist_app() -> App {
    let mut app = make_music_group_app();
    app.panel_focus = PanelFocus::Library;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    for i in 1..40 {
        let mut album = make_item(&format!("Alpha Album {i:02}"), "MusicAlbum");
        album.id = format!("alpha-{i}");
        album.artist = "Alpha".into();
        level.items.push(album);
    }
    for i in 0..6 {
        let mut album = make_item(&format!("Beta Album {i:02}"), "MusicAlbum");
        album.id = format!("beta-{i}");
        album.artist = "Beta".into();
        level.items.push(album);
    }
    level.total_count = level.items.len();
    app
}

fn render_narrow(
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
    let mut terminal = Terminal::new(TestBackend::new(NW, NH)).unwrap();
    terminal
        .draw(|f| component.view(f, Rect::new(0, 0, NW, NH)))
        .unwrap();
    (terminal, component)
}

fn press(model: &mut Model, code: Key) {
    let focused = model.application.focus().cloned();
    if let Some(id) = &focused {
        let msg = if matches!(id, ComponentId::Library) {
            use crate::app::components::library_panel::owner::LibraryContentOwner;
            model
                .test_music_owner_mut()
                .on_key(&tuirealm::event::KeyEvent {
                    code,
                    modifiers: tuirealm::event::KeyModifiers::NONE,
                })
        } else {
            model
                .application
                .get_component_mut(id)
                .expect("focused component mounted")
                .on(&Event::Keyboard(KeyEvent {
                    code,
                    modifiers: KeyModifiers::NONE,
                }))
        };
        if let Some(msg) = msg {
            let mut music_resize = false;
            let mut tv_resize = false;
            model.handle_terminal_message(msg, &mut music_resize, &mut tv_resize);
        }
    }
    model.sync_mounted_surfaces();
}

fn album_cursor(model: &Model, _id: &ComponentId) -> usize {
    model.test_music_owner().album_cursor()
}

#[test]
fn narrow_music_one_column_geometry_and_non_selectable_structural_rows() {
    let app = multi_artist_app();
    let (_terminal, component) = render_narrow(&app, true, 0);
    let flow = component.album_flow_targets();

    // The retained control owns one-dimensional display flow.
    let album_rows = flow.iter().filter(|t| t.is_some()).count();
    let structural_rows = flow.iter().filter(|t| t.is_none()).count();
    assert!(album_rows >= 3, "album rows are selectable targets");
    assert!(
        structural_rows >= 2,
        "artist headings + inter-group spacer publish no target: {:?}",
        flow
    );
    // The first painted row is the "Alpha" heading -> no target.
    assert_eq!(flow.first(), Some(&None));
}

#[test]
fn narrow_music_publishes_full_flow_rows_and_viewport_offset_after_scroll() {
    let app = multi_artist_app();
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.focused = true;
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    component.set_focused(true);
    component.re_anchor(12, 12);

    let mut terminal = Terminal::new(TestBackend::new(NW, 8)).unwrap();
    terminal
        .draw(|f| component.view(f, Rect::new(0, 0, NW, 8)))
        .unwrap();
    let offset = component.album_flow_offset();
    let flow = component.album_flow_targets();
    assert!(offset > 0);
    assert!(!flow.is_empty());
}

#[test]
fn narrow_music_admits_the_selected_album_detail_block() {
    let app = multi_artist_app();
    let (_terminal, component) = render_narrow(&app, true, 0);
    let hero_area = component.test_hero_area();

    assert!(
        hero_area.height > 0,
        "the selected album's inline detail block is admitted at a tall viewport"
    );
    assert_eq!(component.test_selected_item_rect(), Some(hero_area));
    // The selected row and detail geometry are retained by the control.
    assert!(component.album_flow_targets().iter().any(|t| t.is_some()));
}

#[test]
fn narrow_music_falls_back_to_the_ordinary_selected_row_when_the_block_cannot_fit() {
    let app = multi_artist_app();
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.focused = true;
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    component.set_focused(true);
    component.re_anchor(0, 0);

    // A viewport barely taller than the pill row: the detail block cannot be
    // admitted, so the ordinary selected row is restored.
    let short = Rect::new(0, 0, NW, 8);
    let mut terminal = Terminal::new(TestBackend::new(NW, 8)).unwrap();
    terminal.draw(|f| component.view(f, short)).unwrap();

    assert_eq!(
        component.test_hero_area(),
        Rect::default(),
        "no detail block admitted"
    );
    let selected = component
        .test_selected_item_rect()
        .expect("ordinary selected-row rect published on fallback");
    assert_eq!(selected.height, 1, "fallback restores a one-line row");
    assert!(buffer_to_string(&terminal).contains("First Album"));
}

#[test]
fn narrow_music_focused_selection_carries_the_canonical_highlight() {
    let app = multi_artist_app();
    // Fallback viewport so the ordinary selected row (not the hero) is painted.
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.focused = true;
    let mut focused = MusicWorkspaceComponent::new();
    focused.set_content(context.clone());
    focused.set_focused(true);
    focused.re_anchor(0, 0);
    let mut unfocused = MusicWorkspaceComponent::new();
    context.focused = false;
    unfocused.set_content(context);
    unfocused.set_focused(false);
    unfocused.re_anchor(0, 0);

    let area = Rect::new(0, 0, NW, 8);
    let mut ft = Terminal::new(TestBackend::new(NW, 8)).unwrap();
    ft.draw(|f| focused.view(f, area)).unwrap();
    let mut ut = Terminal::new(TestBackend::new(NW, 8)).unwrap();
    ut.draw(|f| unfocused.view(f, area)).unwrap();

    let rect = focused.test_selected_item_rect().unwrap();
    let fbg = ft.backend().buffer()[(rect.x + 4, rect.y)].bg;
    let ubg = ut.backend().buffer()[(rect.x + 4, rect.y)].bg;
    assert_ne!(
        fbg, ubg,
        "the focused selected row is highlighted; the unfocused one is not"
    );
}

#[test]
fn narrow_music_image_bearing_fixture_emits_the_selected_album_art() {
    let mut app = multi_artist_app();
    app.image_protocol_enabled = true;
    let mut model = mounted_model_at(app, NW, NH);
    let _ = draw_mounted_frame(&mut model, NW, NH);
    // The fixture's resting cursor (0) lands on "album-1" ("First Album"),
    // the fixture's original first item; task 9.4's `sync_library_hero_images`
    // projects only the selected album's own hero art (design D9: no
    // paint-time neighbour pre-warm survives the panel migration).
    assert!(
        model
            .app
            .card_image_loading
            .iter()
            .any(|k| k == "album-1:P"),
        "the selected album's projected hero art is requested: {:?}",
        model.app.card_image_loading
    );
}

#[test]
fn narrow_music_ordinary_refresh_retains_the_selected_album_target() {
    let mut model = mounted_model_at(multi_artist_app(), NW, NH);
    let _ = draw_mounted_frame(&mut model, NW, NH);
    let id = ComponentId::Library;

    // Move the component's own cursor, then push an ordinary content refresh:
    // the selected album survives without a shell cursor mirror.
    press(&mut model, Key::Char('j'));
    let _ = draw_mounted_frame(&mut model, NW, NH);
    let moved = album_cursor(&model, &id);
    assert!(moved > 0, "j moved the album cursor");

    model.push_music_workspace_content();
    let _ = draw_mounted_frame(&mut model, NW, NH);
    assert_eq!(
        album_cursor(&model, &id),
        moved,
        "an ordinary refresh keeps the component's divergent cursor"
    );
    assert_eq!(album_cursor(&model, &id), moved);
}

/// Draw one frame after syncing `app.terminal_width/height` to the new size,
/// mirroring the production resize-event path (`apply_terminal_observer`) that
/// `draw_mounted_frame` alone does not emulate.
fn resize_draw(model: &mut Model, width: u16, height: u16) -> String {
    model.app.terminal_width = width;
    model.app.terminal_height = height;
    draw_mounted_frame(model, width, height)
}

#[test]
fn narrow_music_viewport_anchor_round_trips_across_wide_narrow_wide() {
    let mut app = multi_artist_app();
    // Select the last album so the wide rail must scroll: the anchor has a
    // non-trivial target and screen-row offset to preserve.
    let last = app.libs[0].nav_stack.last().unwrap().items.len() - 1;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .set_resting_cursor(last);

    let mut model = mounted_model_at(app, 160, 40);
    let _ = resize_draw(&mut model, 160, 40);
    let id = ComponentId::Library;
    assert!(
        super::test_helpers::mounted_music_wide_geometry(&model)
            .hero
            .width
            > 0
    );
    assert_eq!(album_cursor(&model, &id), last);
    let wide_scroll = mounted_music_scroll(&model);
    assert!(wide_scroll > 0, "the bottom album scrolls the wide rail");
    let wide_offset = wide_anchor_offset(&model);

    // Wide -> Narrow: the selected album and its screen-row offset carry over.
    let narrow = resize_draw(&mut model, 60, 30);
    assert_eq!(
        album_cursor(&model, &id),
        last,
        "selection survives the flip:\n{narrow}"
    );
    assert!(
        narrow.contains("Beta Album"),
        "the narrow hero paints the carried-over selected album:\n{narrow}"
    );

    // Narrow -> Wide: the album, cursor, scroll offset, and screen-row offset
    // all return to the wide arrangement.
    let _ = resize_draw(&mut model, 160, 40);
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
        "wide album_scroll recomputes to the identical bottom-anchored offset"
    );
    assert_eq!(
        wide_anchor_offset(&model),
        wide_offset,
        "the selected-row screen offset is preserved across the round trip"
    );
}

#[test]
fn narrow_music_reused_model_paints_after_a_wide_to_narrow_resize() {
    // Unit-1 investigation: a single `Model` resized Wide -> Narrow was
    // reported painting the narrow grouped-Music buffer blank. The cause is
    // the test harness, not the painter -- `draw_mounted_frame` does not sync
    // `app.terminal_width/height`, so root frame composition collapses
    // `left_area` to 0x0 and the workspace `view` early-returns. With the
    // dimensions synced (as the real resize-event path does), the canonical
    // persistent `InlineMediaBrowser` recomputes its flow and paints the
    // album rows on the resized frame.
    let mut model = mounted_model_at(multi_artist_app(), 160, 40);
    let _ = resize_draw(&mut model, 160, 40);
    assert!(
        super::test_helpers::mounted_music_wide_geometry(&model)
            .hero
            .width
            > 0
    );

    let narrow = resize_draw(&mut model, 60, 30);
    assert!(
        narrow.contains("First Album") || narrow.contains("Alpha Album"),
        "narrow grouped Music must paint album rows after the resize:\n{narrow}"
    );

    let wide = resize_draw(&mut model, 160, 40);
    assert!(
        super::test_helpers::mounted_music_wide_geometry(&model)
            .hero
            .width
            > 0
    );
    assert!(wide.contains("First Album") || wide.contains("Alpha Album"));
}

#[test]
fn narrow_music_paints_only_through_the_shared_inline_presentation() {
    use super::components::media_list::{
        INLINE_MEDIA_BROWSER_PAINTS, PLAIN_ROWS_PAINTS, WIDE_MEDIA_LIST_PAINTS,
    };
    let app = multi_artist_app();
    INLINE_MEDIA_BROWSER_PAINTS.with(|c| c.set(0));
    PLAIN_ROWS_PAINTS.with(|c| c.set(0));
    WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));

    let (terminal, _component) = render_narrow(&app, true, 0);

    assert_eq!(
        INLINE_MEDIA_BROWSER_PAINTS.with(std::cell::Cell::get),
        1,
        "the narrow album flow uses the canonical Inline presentation"
    );
    assert_eq!(
        PLAIN_ROWS_PAINTS.with(std::cell::Cell::get),
        0,
        "the deleted narrow Music painter's plain-rows path must not run"
    );
    assert_eq!(
        WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get),
        0,
        "the narrow surface must not paint the Wide rail"
    );
    assert!(buffer_to_string(&terminal).contains("First Album"));
}

/// The Narrow inline hero reads only the shell-projected image state (design
/// D9): the painter reserves the policy's box and the shell paints the cached
/// protocol into it, so no paint-time album-art fetch exists.
#[test]
fn narrow_music_inline_hero_uses_the_projected_image_state() {
    use crate::app::components::library_panel::HeroImageState;

    let app = multi_artist_app();
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.focused = true;
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    component.set_focused(true);
    component.re_anchor(0, 0);
    // The projection's `Ready` state: the box is reserved and the shell
    // paints the cached protocol after `view` returns.
    component.set_hero_image(HeroImageState::Ready {
        cache_key: "album-1:P".into(),
        decoded: Some((600, 600)),
    });

    let mut terminal = Terminal::new(TestBackend::new(NW, NH)).unwrap();
    terminal
        .draw(|f| component.view(f, Rect::new(0, 0, NW, NH)))
        .unwrap();

    let paint = component
        .take_panel_image_paint()
        .expect("the inline hero reserves the projected image box");
    assert_eq!(paint.cache_key, "album-1:P");
    assert!(!paint.centered, "the narrow inline hero is right-aligned");
    assert!(paint.area.width > 0 && paint.area.height > 0);
    assert!(component.test_hero_area().height > 0);
}

/// While the projection is still loading, the inline hero keeps its box and
/// paints the shared placeholder — it neither fetches nor drops the block.
#[test]
fn narrow_music_inline_hero_keeps_the_box_while_the_image_loads() {
    use crate::app::components::library_panel::HeroImageState;

    let app = multi_artist_app();
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.focused = true;
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    component.set_focused(true);
    component.re_anchor(0, 0);
    component.set_hero_image(HeroImageState::Loading);

    let mut terminal = Terminal::new(TestBackend::new(NW, NH)).unwrap();
    terminal
        .draw(|f| component.view(f, Rect::new(0, 0, NW, NH)))
        .unwrap();

    assert!(
        component.take_panel_image_paint().is_none(),
        "a loading image is not painted by the shell yet"
    );
    assert!(
        component.test_hero_area().height > 0,
        "the inline hero block still reserves its box"
    );
}

/// The wide selected-row screen offset the mounted `LibraryPanel` published
/// this frame (index of the selected album row below the artist header +
/// earlier albums).
fn wide_anchor_offset(model: &Model) -> usize {
    let layout = super::test_helpers::mounted_music_layout(model);
    let rect = layout.selected_item_rect.expect("wide selected-row rect");
    (rect.y - layout.left_area.y) as usize
}
