use super::test_helpers::render_queue_shell;
use crate::app::components::{ComponentId, QueueComponent};
use crate::app::tests::{make_app_stub, make_item, make_remote_app_stub, make_session};
use crate::app::{palette, App, PanelFocus, PanelMode, QueueScope};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

const WIDTH: u16 = 120;
const HEIGHT: u16 = 30;

fn attached_app(client: &str) -> App {
    let mut app = make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_session("device", client));
    app
}

fn direct_app() -> App {
    let mut app = make_remote_app_stub(
        vec![make_item("local", "Movie")],
        vec![make_item("remote", "Movie")],
    );
    app.direct_remote_label = Some("direct-device".into());
    app.queue_scope = QueueScope::Local;
    app
}

fn title_area(app: &App) -> Rect {
    let area = app.layout.main.queue_area;
    Rect {
        x: area.x + 2,
        y: area.y.saturating_sub(2),
        width: area.width.saturating_sub(4),
        height: 1,
    }
}

fn row_symbols(app: &App, term: &Terminal<TestBackend>) -> Vec<String> {
    let area = title_area(app);
    let buffer = term.backend().buffer();
    (area.x..area.right())
        .map(|x| buffer[(x, area.y)].symbol().to_string())
        .collect()
}

fn put_text(row: &mut [String], offset: usize, text: &str) {
    for (index, ch) in text.chars().enumerate() {
        if let Some(cell) = row.get_mut(offset + index) {
            *cell = ch.to_string();
        }
    }
}

fn assert_style(cell: &ratatui::buffer::Cell, fg: Color, bg: Color, bold: bool) {
    assert_eq!(cell.style().fg, Some(fg));
    assert_eq!(cell.style().bg, Some(bg));
    assert_eq!(cell.style().add_modifier.contains(Modifier::BOLD), bold);
}

fn assert_title(
    app: &App,
    term: &Terminal<TestBackend>,
    nerd_fonts: bool,
    split: bool,
    mbv_session: bool,
    target: Option<&str>,
    scopes: (Rect, Rect),
    local_selected: bool,
) {
    let area = title_area(app);
    assert!(
        area.width > 20,
        "characterization needs a complete title row"
    );
    let icon = if nerd_fonts { "\u{f0afe}" } else { "🖧" };
    let remote_icon = if nerd_fonts { "\u{f1616}" } else { "🖧" };
    let hostname = mbv_core::api::device_name().to_uppercase();
    let prefix = if !split {
        format!(" {icon}{}{hostname}", if nerd_fonts { " " } else { "  " })
    } else if mbv_session {
        format!(" {icon} CONNECTED:  {}", target.expect("split target"))
    } else {
        format!(" {icon} CONNECTED: {}", target.expect("split target"))
    };

    let mut expected = vec![" ".to_string(); area.width as usize];
    put_text(&mut expected, 0, &prefix);
    if split && mbv_session {
        let (local_scope, remote_scope) = scopes;
        put_text(&mut expected, (local_scope.x - area.x) as usize, " ⌂ ");
        put_text(
            &mut expected,
            (remote_scope.x - area.x) as usize,
            &format!(" {remote_icon} "),
        );
    }
    assert_eq!(row_symbols(app, term), expected, "queue title text");

    let buffer = term.backend().buffer();
    let base = palette::SURFACE_CHROME;
    let (local_scope, remote_scope) = scopes;
    for x in area.x..area.right() {
        if !local_scope.contains((x, area.y).into()) && !remote_scope.contains((x, area.y).into()) {
            assert_eq!(buffer[(x, area.y)].style().bg, Some(base));
        }
    }
    let icon_x = area.x + 1;
    assert_style(
        &buffer[(icon_x, area.y)],
        palette::TEXT_METADATA,
        base,
        false,
    );

    let local_fg = palette::TEXT_FOCUS_ACCENT;
    let local_start = area.x + if split && mbv_session { 0 } else { 2 };
    let local_end = if split {
        area.x + 14
    } else {
        let label_len = if nerd_fonts { 1 } else { 2 } + hostname.chars().count();
        area.x + 2 + label_len as u16
    };
    for x in local_start..local_end {
        if x != icon_x {
            assert_style(&buffer[(x, area.y)], local_fg, base, false);
        }
    }
    if !split {
        assert_style(&buffer[(area.x, area.y)], Color::Reset, base, false);
    }

    if split {
        let target_start = area.x + if mbv_session { 15 } else { 14 };
        let target_len = target.expect("split target").chars().count() as u16;
        let target_fg = if mbv_session {
            palette::ACCENT
        } else {
            palette::TEXT_FOCUS_ACCENT
        };
        let (local_scope, remote_scope) = scopes;
        let target_end = (target_start + target_len).min(local_scope.x);
        for x in target_start..target_end {
            assert_style(&buffer[(x, area.y)], target_fg, base, true);
        }

        if mbv_session {
            assert!(local_scope.width > 0 && remote_scope.width > 0);
            for x in local_scope.x..local_scope.right() {
                if area.contains((x, area.y).into()) {
                    assert_style(
                        &buffer[(x, area.y)],
                        if local_selected {
                            palette::TEXT_FOCUS_ACCENT
                        } else {
                            palette::PILL_FG
                        },
                        if local_selected {
                            palette::ACCENT
                        } else {
                            palette::PILL_BG
                        },
                        false,
                    );
                }
            }
            for x in remote_scope.x..remote_scope.right() {
                if area.contains((x, area.y).into()) {
                    assert_style(
                        &buffer[(x, area.y)],
                        if local_selected {
                            palette::PILL_FG
                        } else {
                            palette::TEXT_FOCUS_ACCENT
                        },
                        if local_selected {
                            palette::PILL_BG
                        } else {
                            palette::ACCENT
                        },
                        false,
                    );
                }
            }
        } else {
            assert_eq!(local_scope, Rect::default());
            assert_eq!(remote_scope, Rect::default());
        }
    } else {
        let (local_scope, remote_scope) = scopes;
        assert_eq!(local_scope, Rect::default());
        assert_eq!(remote_scope, Rect::default());
    }
}

fn assert_state(
    mut app: App,
    nerd_fonts: bool,
    split: bool,
    mbv_session: bool,
    target: Option<&str>,
) {
    app.use_nerd_fonts = nerd_fonts;
    let (model, term) = render_queue_shell(app, WIDTH, HEIGHT);
    let title = model.app.queue_title_model();
    assert_eq!(title.show_split, split);
    assert_eq!(title.is_mbv_session, mbv_session);
    let scopes = model
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
        .map(QueueComponent::test_scope_pill_areas)
        .expect("QueueComponent should be mounted");
    assert_title(
        &model.app,
        &term,
        nerd_fonts,
        split,
        mbv_session,
        target,
        scopes,
        title.local_selected,
    );
}

#[test]
fn queue_title_off_without_nerd_fonts() {
    assert_state(make_app_stub(), false, false, false, None);
}

#[test]
fn queue_title_off_with_nerd_fonts() {
    assert_state(make_app_stub(), true, false, false, None);
}

#[test]
fn queue_title_direct_remote_without_nerd_fonts() {
    assert_state(direct_app(), false, true, true, Some("direct-device"));
}

#[test]
fn queue_title_direct_remote_with_nerd_fonts() {
    assert_state(direct_app(), true, true, true, Some("direct-device"));
}

#[test]
fn queue_title_attached_mbv_without_nerd_fonts() {
    assert_state(attached_app("mbv"), false, true, true, Some("device"));
}

#[test]
fn queue_title_attached_mbv_with_nerd_fonts() {
    assert_state(attached_app("mbv"), true, true, true, Some("device"));
}

#[test]
fn queue_title_attached_emby_without_nerd_fonts() {
    assert_state(attached_app("Emby"), false, true, false, Some("device"));
}

#[test]
fn queue_title_attached_emby_with_nerd_fonts() {
    assert_state(attached_app("Emby"), true, true, false, Some("device"));
}

/// `unify-surface-colour` 4.2: the queue panel's title band is the chrome-band
/// `QueuePanelBand` surface, so it keeps the chrome value in both focus states
/// and never follows the queue column's focus green.
#[test]
fn queue_panel_band_keeps_the_chrome_band_surface_in_both_focus_states() {
    let expected =
        palette::surface_colors_for_column_focus(palette::Surface::QueuePanelBand, false).fill;
    assert_eq!(
        expected,
        palette::SURFACE_CHROME,
        "the queue band keeps today's chrome value"
    );

    for focus in [PanelFocus::Queue, PanelFocus::Library] {
        let mut app = make_app_stub();
        app.panel_mode = PanelMode::Both;
        app.panel_focus = focus;
        let (model, term) = render_queue_shell(app, WIDTH, HEIGHT);
        let area = title_area(&model.app);
        assert!(
            area.width > 0 && area.y < HEIGHT,
            "characterization needs the queue title row"
        );
        assert_eq!(
            model.application.focus() == Some(&ComponentId::Queue),
            focus == PanelFocus::Queue,
            "the queue component's focus must follow the panel focus (focus={focus:?})"
        );
        let buffer = term.backend().buffer();
        assert_eq!(
            buffer[(area.x, area.y)].style().bg,
            Some(expected),
            "queue title band must not follow panel focus (focus={focus:?})"
        );
    }
}

/// `unify-surface-colour` 4.2 review: the queue's **selected** scope pill is the
/// table's `QueueScopePillSelected` surface (the Direct-remote aqua), not a
/// screen-named accent, so both arms keep that value in both focus states.
#[test]
fn queue_scope_pill_selected_keeps_the_aqua_surface_in_both_focus_states() {
    let selected_pill =
        palette::surface_colors_for_column_focus(palette::Surface::QueueScopePillSelected, false)
            .fill;
    let unselected_pill =
        palette::surface_colors_for_column_focus(palette::Surface::PillChip, false).fill;
    assert_eq!(
        selected_pill,
        palette::ACCENT,
        "the selected scope pill keeps today's aqua value"
    );
    assert_ne!(
        selected_pill, unselected_pill,
        "the selected and unselected scope pills differ"
    );

    for local_selected in [true, false] {
        for focus in [PanelFocus::Queue, PanelFocus::Library] {
            let mut app = direct_app();
            app.panel_mode = PanelMode::Both;
            app.panel_focus = focus;
            app.set_queue_scope(if local_selected {
                QueueScope::Local
            } else {
                QueueScope::Remote
            });
            assert_eq!(
                app.viewed_queue_scope() == QueueScope::Local,
                local_selected,
                "the test needs the intended scope selected"
            );
            let (model, term) = render_queue_shell(app, WIDTH, HEIGHT);
            let (local_scope, remote_scope) = model
                .application
                .get_component(&ComponentId::Queue)
                .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
                .map(QueueComponent::test_scope_pill_areas)
                .expect("QueueComponent should be mounted");
            assert!(
                local_scope.width > 0 && remote_scope.width > 0,
                "characterization needs both scope pills \
                 (local_selected={local_selected}, focus={focus:?})"
            );
            let buffer = term.backend().buffer();
            let (local_bg, remote_bg) = if local_selected {
                (selected_pill, unselected_pill)
            } else {
                (unselected_pill, selected_pill)
            };
            assert_eq!(
                buffer[(local_scope.x, local_scope.y)].style().bg,
                Some(local_bg),
                "local scope pill background \
                 (local_selected={local_selected}, focus={focus:?})"
            );
            assert_eq!(
                buffer[(remote_scope.x, remote_scope.y)].style().bg,
                Some(remote_bg),
                "remote scope pill background \
                 (local_selected={local_selected}, focus={focus:?})"
            );
        }
    }
}
