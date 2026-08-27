use super::*;
use crate::app::components::{OverlayId, TvWorkspaceComponent};
use crate::app::images::CachedImage;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use crate::app::tests::make_app_stub;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tuirealm::component::AppComponent;
use tuirealm::event::{
    Event, Key as TuiKey, KeyEvent as TuiKeyEvent, KeyModifiers as TuiKeyModifiers,
};

#[test]
fn ui_root_terminal_key_reaches_legacy_help_handler() {
    let mut model = Model::new(make_app_stub());
    let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let routed = route_terminal_observer_message(
        Msg::TerminalEvent(TerminalObserverEvent::Key(key)),
        Some(&ComponentId::UiRoot),
    );
    let Some(Msg::TerminalEvent(TerminalObserverEvent::Key(key))) = routed else {
        panic!("UiRoot terminal key was not retained for the shell handler");
    };

    assert!(!model.handle_legacy_key(key));
    assert!(model
        .application
        .mounted(&ComponentId::Overlay(OverlayId::Help)));
}

#[test]
fn converted_surface_skips_observer_key_but_retains_redraw_signal() {
    let focused = ComponentId::Playback;
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    assert!(route_terminal_observer_message(
        Msg::TerminalEvent(TerminalObserverEvent::Key(key)),
        Some(&focused),
    )
    .is_none());
    assert!(matches!(
        route_terminal_observer_message(
            Msg::TerminalEvent(TerminalObserverEvent::NoOp),
            Some(&focused),
        ),
        Some(Msg::TerminalEvent(TerminalObserverEvent::NoOp))
    ));
}

#[test]
fn terminal_resize_observer_preserves_layout_side_effects() {
    let mut model = Model::new(make_app_stub());
    model.app.force_clear = false;
    model
        .app
        .card_image_states
        .insert("stale".into(), CachedImage::empty());
    model.app.card_image_loading.insert("stale".into());
    let mut music_resize = false;
    let mut tv_resize = false;
    let mut quit = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::Resize,
        Some(&ComponentId::Playback),
        &mut music_resize,
        &mut tv_resize,
        &mut quit,
    );
    assert!(model.app.force_clear);
    assert!(model.app.card_image_states.is_empty());
    assert!(model.app.card_image_loading.is_empty());
    assert!(music_resize && tv_resize);
    assert!(!quit);
}

#[test]
fn terminal_focus_observer_preserves_refocus_side_effects() {
    let mut model = Model::new(make_app_stub());
    let mut music_resize = false;
    let mut tv_resize = false;
    let mut quit = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusGained,
        Some(&ComponentId::Playback),
        &mut music_resize,
        &mut tv_resize,
        &mut quit,
    );
    assert!(model.app.refocus_at.is_some());
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusLost,
        Some(&ComponentId::Playback),
        &mut music_resize,
        &mut tv_resize,
        &mut quit,
    );
    assert!(model.app.refocus_at.is_none());
    assert!(!quit);
}

#[test]
fn global_view_key_from_media_surface_reaches_app_handler() {
    let mut surface = TvWorkspaceComponent::new();
    surface.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
        None,
        None,
        0,
        None,
        true,
        false,
    ));
    let Some(Msg::Shell(ShellRequest::GlobalViewKey(key))) =
        surface.on(&Event::Keyboard(TuiKeyEvent {
            code: TuiKey::Function(1),
            modifiers: TuiKeyModifiers::NONE,
        }))
    else {
        panic!("unmatched media-surface key did not use the global adapter");
    };

    let mut model = Model::new(make_app_stub());
    assert!(!model.handle_legacy_key(key));
    assert!(model
        .application
        .mounted(&ComponentId::Overlay(OverlayId::Help)));
}
