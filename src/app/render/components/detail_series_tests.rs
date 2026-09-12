use crate::app::components::TvWorkspaceComponent;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::test_helpers::buffer_to_string;
use crate::app::render::TvWideRenderCtx;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;

/// TV Normal/Narrow detail is the panel's shared inline hero, not a
/// destination-specific Series painter. Keep this characterization at the
/// panel boundary so a TV-specific inline painter cannot return.
#[test]
fn narrow_series_detail_is_painted_by_the_shared_panel_skeleton() {
    let mut series = make_item("The Series", "Series");
    series.id = "series".into();
    series.overview = "A shared panel overview.".into();

    let mut component = TvWorkspaceComponent::new();
    component.set_is_wide(false);
    component.set_focused(true);
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0, 0),
        Some(series),
        None,
        0,
        None,
        false,
    ));

    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let output = buffer_to_string(&terminal);
    assert!(output.contains("The Series"));
    assert!(output.contains("shared"));
}
