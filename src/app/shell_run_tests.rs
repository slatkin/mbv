use super::*;
use crate::app::images::{series_image_cache_key, CachedImage};
use crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;
use crate::app::render::make_movie_app;
use crate::app::render::HomeImagePaint;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;

fn mounted_wide_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
    let mut model = Model::new(app);
    model.sync_tv_workspace();
    model
}

/// The Series artwork placeholder state the wide workspace is currently painting.
fn wide_tv_shows_placeholder(model: &mut Model) -> bool {
    let id = model
        .tv_workspace_id
        .clone()
        .expect("TV workspace component mounted");
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let component = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type");
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    match component.take_image_paint() {
        Some(HomeImagePaint::Series {
            show_placeholder, ..
        }) => show_placeholder,
        _ => panic!("expected a Series image request"),
    }
}

/// The fixture has no Emby client, so `spawn_image_fetch` resolves its own
/// request synchronously into `card_image_rx`. A step that must start from a
/// quiet channel drops those completions first.
fn drop_pending_image_completions(model: &mut Model) {
    while model.app.card_image_rx.try_recv().is_ok() {}
}

/// Task 2.3: a Series completion is what re-projects the wide workspace, so the
/// cached Thumb-first entry replaces the placeholder. The gate covers the
/// `:ser:` family the painter builds its keys under.
#[test]
fn series_image_completion_repushes_tv_workspace_content() {
    let mut model = mounted_wide_tv_model();
    model.app.image_protocol_enabled = true;
    model.push_tv_workspace_content();
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "an uncached push must project the placeholder"
    );

    let painted_key = series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES);
    assert!(
        model.drain_card_image_completions(),
        "the Series prefetch must resolve into the cache"
    );
    assert!(
        model.app.card_image_states.contains_key(&painted_key),
        "the painted Series key must be cached"
    );
    assert!(
        !wide_tv_shows_placeholder(&mut model),
        "the cached Series entry must replace the placeholder"
    );
}

/// Task 2.3: no other image namespace may drive the TV projection. The cached
/// Series entry is planted without a re-push, so only a gate that wrongly
/// matches this key can clear the placeholder the component still holds.
#[test]
fn non_series_image_completion_leaves_tv_projection_alone() {
    let mut model = mounted_wide_tv_model();
    model.app.image_protocol_enabled = true;
    model.push_tv_workspace_content();
    drop_pending_image_completions(&mut model);
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "the uncached push must project the placeholder"
    );

    model.app.card_image_states.insert(
        series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES),
        CachedImage::empty(),
    );
    model
        .app
        .card_image_tx
        .send(("movie-focused:P".into(), None))
        .expect("image completion channel");

    assert!(
        model.drain_card_image_completions(),
        "the card completion must be drained"
    );
    assert!(
        model.app.card_image_states.contains_key("movie-focused:P"),
        "the drained entry must reach the cache"
    );
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "a non-Series completion must leave the TV projection alone"
    );
}
