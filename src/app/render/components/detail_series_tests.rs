use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::tv_content::TvContent;
use crate::app::components::LibraryKind;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::test_helpers::buffer_to_string;
use crate::app::render::TvWideRenderCtx;
use crate::app::tests::make_item;
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;

/// TV Normal/Narrow detail is the panel's shared inline hero, not a
/// destination-specific Series painter. Keep this characterization at the
/// panel boundary so a TV-specific inline painter cannot return (task 8.4:
/// the owner is hosted by the mounted panel).
#[test]
fn narrow_series_detail_is_painted_by_the_shared_panel_skeleton() {
    let mut series = make_item("The Series", "Series");
    series.id = "series".into();
    series.overview = "A shared panel overview.".into();

    let mut owner = TvContent::new();
    owner.set_is_wide(false);
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0, 0),
        Some(series),
        None,
        0,
        None,
        false,
    ));
    let key = LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: "lib".into(),
        kind: LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key));

    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, frame.area()))
        .unwrap();
    let output = buffer_to_string(&terminal);
    assert!(output.contains("The Series"));
    assert!(output.contains("shared"));
}
