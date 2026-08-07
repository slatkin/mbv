use crate::app::tests::*;

#[test]
fn home_section_cycle_includes_continue_watching_in_both_directions() {
    let mut app = make_app_stub();
    app.home.continue_items = make_items(1);
    app.home.latest = sections(2);

    app.home.section = 2;
    app.home_move_section(1);
    assert_eq!(app.home.section, 0);

    app.home_move_section(-1);
    assert_eq!(app.home.section, 2);
}
