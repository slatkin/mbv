use super::test_helpers::*;
use crate::app::tests::make_app_stub;
use crate::app::{PanelFocus, TabSelection};

fn home_emby_app() -> crate::app::App {
    let mut app = make_app_stub();
    let movie_app = make_movie_app();
    app.home.continue_items = vec![movie_app.libs[0].nav_stack[0].items[0].clone()];
    app.home.section = 0;
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app
}

#[test]
fn narrow_home_inserts_selected_detail_into_the_section_flow() {
    let mut app = home_emby_app();
    let layout = render_view(&mut app, 60, 40);

    assert!(layout.hero_area.height > 0);
    assert!(layout.hero_area.y >= layout.left_area.y);
}

#[test]
fn narrow_home_suppresses_detail_when_the_viewport_is_too_short() {
    let mut app = home_emby_app();
    let layout = render_view(&mut app, 60, 4);

    assert_eq!(layout.hero_area.height, 0);
}
