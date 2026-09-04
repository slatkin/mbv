use super::*;
use crate::app::components::msg::{PodcastEpisodeIntent, PodcastEpisodeTransition};
use crate::app::components::{Msg, ShellRequest};
use crate::app::render::HomeImagePaint;
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_core::audiobookshelf::{AudiobookshelfLibrary, AudiobookshelfShow};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn abs_podcast_shell_mounts_and_routes_component() {
    let mut model = Model::new(audiobookshelf_app());
    model.app.audiobookshelf_browse[0].append_page(
        1,
        20,
        2,
        vec![AudiobookshelfShow {
            library_item_id: "show-b".into(),
            title: "Show B".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    model.sync_audiobookshelf_podcast();
    let id = model
        .abs_podcast_id
        .clone()
        .expect("podcast component mounted");
    let message = model
        .application
        .get_component_mut(&id)
        .expect("podcast component")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { index })) = message else {
        panic!("show movement should carry the resolved show index");
    };
    assert_eq!(index, 1, "component resolved the next row locally");
    // The shell applies the resolved index directly through the index-taking
    // entry point (split-audiobookshelf-cursor-ownership D1), preserving the
    // detail-fetch / position-save target.
    model.app.select_audiobookshelf_show(index);
    assert_eq!(model.app.audiobookshelf_browse[0].cursor(), 1);
    let unclaimed = model
        .application
        .get_component_mut(&id)
        .expect("podcast component")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Char('z'),
            modifiers: KeyModifiers::NONE,
        }));
    assert_eq!(unclaimed, None);
}

/// split-audiobookshelf-cursor-ownership D4 / task 5.3: a real shell content
/// push that drops the component's selected show must not leave any
/// App-sourced interaction value in the component.
#[test]
fn abs_podcast_shell_push_drops_stale_component_episode_state() {
    let mut model = Model::new(audiobookshelf_app());
    model.app.audiobookshelf_browse[0].append_page(
        1,
        20,
        2,
        vec![AudiobookshelfShow {
            library_item_id: "show-b".into(),
            title: "Show B".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    model.sync_audiobookshelf_podcast();
    model
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted")
        .set_episode_selection(Some(0));

    // App content changes: show-a (the component's selected show) is removed.
    let state = &mut model.app.audiobookshelf_browse[0];
    state.shows.retain(|show| show.library_item_id != "show-a");
    state.selected_id = Some("show-b".into());
    model.push_audiobookshelf_podcast_content();

    assert_eq!(
        model
            .abs_podcast_component_mut(0)
            .expect("podcast component mounted")
            .episode_selection(),
        None,
        "the content push must drop the component's stale episode selection"
    );
}

#[test]
fn abs_podcast_shell_routes_episode_transition_to_app() {
    let mut model = Model::new(audiobookshelf_app());
    // Two episodes so a Down move has a real second target.
    model.app.audiobookshelf_browse[0].episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: None,
        },
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(),
            title: "Episode B".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);
    model.sync_audiobookshelf_podcast();
    let id = model
        .abs_podcast_id
        .clone()
        .expect("podcast component mounted");
    model
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted")
        .set_episode_selection(Some(0));
    let message = model
        .application
        .get_component_mut(&id)
        .expect("podcast component")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeTransition(transition))) =
        message
    else {
        panic!("episode movement should be routed as a typed episode transition");
    };
    assert_eq!(transition, PodcastEpisodeTransition::NextEpisode);
    // The mounted component owns episode selection; NextEpisode already
    // moved its own selection into the second row. Assert from the
    // component accessor, not the App mirror, since the legacy App move
    // handler is removed (5.3d.11 U2) and only the shell re-projection
    // keeps the two in sync (D17).
    let component = model
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted");
    assert_eq!(component.episode_selection(), Some(1));
}

#[test]
fn abs_podcast_shell_routes_action_intent_to_app() {
    let mut model = Model::new(audiobookshelf_app());
    // FocusOrPlay with no episode selection enters episode selection on the
    // mounted component (task 5.3d.11 U2), so mount it first and read the
    // selection back through the U0 accessor.
    model.sync_audiobookshelf_podcast();
    model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::FocusOrPlay);
    assert_eq!(
        model
            .abs_podcast_component_mut(0)
            .expect("podcast component mounted")
            .episode_selection(),
        Some(0)
    );

    // With episode selection active on the component, the enqueue intent
    // reaches the App enqueue seam (the default fixture has one downloaded
    // episode).
    model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::Enqueue);
    assert_eq!(model.app.player_tab.total_queue_len(), 1);
}

/// U5 regression: playback target resolution reads the mounted component's
/// authoritative episode selection through the U0 accessor, never the App
/// `episode_selection` mirror. When the component selection is present but
/// the App mirror is stale (None), FocusOrPlay must still resolve the
/// component-selected episode into a real play attempt rather than
/// re-entering episode selection. Without an eligible owner the play is
/// inert, surfacing the owner-unavailable status as the observable.
#[test]
fn abs_podcast_focus_play_uses_component_selection_not_stale_app_mirror() {
    let mut model = Model::new(audiobookshelf_app());
    crate::app::tests_podcast::add_emby_movie_library(&mut model.app);
    // Only the component owns the resolved episode target.
    model.sync_audiobookshelf_podcast();
    model
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted")
        .set_episode_selection(Some(0));

    // A real FocusOrPlay through the Model handler must not re-enter
    // selection: the component's owned selection resolves the play target,
    // which is inert here only because the owner is unavailable.
    model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::FocusOrPlay);
    assert!(
        model
            .app
            .status
            .contains("Audiobookshelf playback owner is unavailable"),
        "component-resolved FocusOrPlay must attempt playback, not re-enter selection"
    );
    assert_eq!(
        model
            .abs_podcast_component_mut(0)
            .expect("podcast component mounted")
            .episode_selection(),
        Some(0),
        "component selection must remain the resolved episode target"
    );
    assert_eq!(
        model.app.player_tab.total_queue_len(),
        0,
        "inert play attempt must not enqueue"
    );
}

/// U5 regression: Enqueue resolves the component-owned episode index the
/// same way, editing the Composed queue even when the App mirror is stale.
#[test]
fn abs_podcast_enqueue_uses_component_selection_over_stale_app_mirror() {
    let mut model = Model::new(audiobookshelf_app());
    model.sync_audiobookshelf_podcast();
    model
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted")
        .set_episode_selection(Some(0));

    model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::Enqueue);
    assert_eq!(
        model.app.player_tab.total_queue_len(),
        1,
        "Enqueue must resolve the component-owned episode target"
    );
    let component = model
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted");
    assert_eq!(component.episode_selection(), Some(0));
}

/// The load-bearing space/ctrl-a contract (task 5.3d.7), with an Emby
/// library present for leash-checking, space/ctrl-a on the mounted podcast
/// component report the typed action intents with episode selection active,
/// and the shell's FocusOrPlay (space) play is inert on an unsupported
/// owner while ctrl-a Enqueue still edits the Composed queue. The action
/// resolves episode-selection and owner eligibility at the Model boundary,
/// so the episode selection stays Some(0) throughout. Mirrors the
/// component/shell route, not direct field assignment.
#[test]
fn abs_podcast_shell_space_and_ctrla_are_inert_without_owner() {
    let mut model = Model::new(audiobookshelf_app());
    crate::app::tests_podcast::add_emby_movie_library(&mut model.app);
    let nav_len = model.app.libs[0].nav_stack.len();
    model.sync_audiobookshelf_podcast();
    let id = model
        .abs_podcast_id
        .clone()
        .expect("podcast component mounted");
    model
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted")
        .set_episode_selection(Some(0));

    // Space -> FocusOrPlay: reported with selection, resolved inert at the
    // App boundary without an eligible owner.
    let space = model
        .application
        .get_component_mut(&id)
        .expect("podcast component")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent))) = space else {
        panic!("space should report a typed action intent");
    };
    assert_eq!(intent, PodcastEpisodeIntent::FocusOrPlay);
    // Resolve the reported intent at the Model boundary; the play attempt
    // is inert on an unsupported owner, surfacing the owner-unavailable
    // status without enqueuing.
    model.handle_audiobookshelf_podcast_episode_intent(intent);
    assert_eq!(
        model
            .abs_podcast_component_mut(0)
            .expect("podcast component mounted")
            .episode_selection(),
        Some(0)
    );
    assert_eq!(
        model.app.player_tab.total_queue_len(),
        0,
        "inert space must not enqueue"
    );
    assert!(
        model
            .app
            .status
            .contains("Audiobookshelf playback owner is unavailable"),
        "inert FocusOrPlay must surface the owner-unavailable status"
    );

    // Ctrl+A = Enqueue intent, resolved by the shell App effect.
    let ctrl_a = model
        .application
        .get_component_mut(&id)
        .expect("podcast component")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Char('a'),
            modifiers: KeyModifiers::CONTROL,
        }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent))) = ctrl_a else {
        panic!("ctrl-a should report as a typed action intent");
    };
    assert_eq!(intent, PodcastEpisodeIntent::Enqueue);
    // Resolve the reported intent at the Model boundary so the Composed
    // queue is edited; the component only reports, it does not enqueue.
    model.handle_audiobookshelf_podcast_episode_intent(intent);
    assert_eq!(
        model
            .abs_podcast_component_mut(0)
            .expect("podcast component mounted")
            .episode_selection(),
        Some(0),
        "enqueue intent must preserve the selected episode"
    );
    assert_eq!(model.app.player_tab.total_queue_len(), 1);
    assert_eq!(
        model.app.libs[0].nav_stack.len(),
        nav_len,
        "inert activation must not navigate the Emby library"
    );
}

/// Synchronous image-plan lifecycle for the podcast component (task
/// 5.3d.10b): images disabled yields no plan, images enabled with a
/// selected show yields the correct `AudiobookshelfCover` id + nonzero
/// paint rect, and a subsequent disabled view clears the plan with no
/// stale paint. No network fetch or secret is scheduled.
#[test]
fn abs_podcast_component_image_plan_lifecycle() {
    let mut component = AudiobookshelfPodcastComponent::new();
    let show = AudiobookshelfShow {
        library_item_id: "show-paint".into(),
        title: "Show".into(),
        author: None,
        description: None,
        cover_path: None,
    };
    let mut state = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    });
    state.append_page(0, 20, 1, vec![show]);
    state.select(0);

    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let area = Rect::new(0, 0, 100, 40);

    // Images disabled: no plan, even with a selected show.
    component.set_content(&state, true, false);
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    assert!(component.take_image_paint().is_none());

    // Images enabled with a selected show: plan carries the show id and a
    // nonzero paint rect (mirrors the ABS Book component handoff).
    component.set_content(&state, true, true);
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    let plan = component.take_image_paint();
    let Some(HomeImagePaint::AudiobookshelfCover {
        area: paint_area,
        library_item_id,
        show_placeholder,
    }) = plan
    else {
        panic!("expected AudiobookshelfCover plan when images enabled");
    };
    assert_eq!(library_item_id, "show-paint");
    assert!(
        paint_area.width > 0 && paint_area.height > 0,
        "paint rect must be nonzero"
    );
    assert!(
        show_placeholder,
        "podcast hero uses the placeholder cover path"
    );

    // A subsequent disabled view must clear the plan (no stale paint).
    component.set_content(&state, true, false);
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    assert!(component.take_image_paint().is_none());
}

/// Shell projection (task 5.3d.10d): the component owns painting and
/// computes its geometry during `view`; the shell mirrors the still-required
/// fields into `LayoutMain` so legacy readers stay correct. Wide keeps a
/// nonzero podcast right area; narrow resets it to zero, including the
/// wide-to-narrow redraw.
#[test]
fn abs_podcast_shell_projection_wide_narrow_right_area() {
    let backend = TestBackend::new(200, 50);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new(audiobookshelf_app());
    model.sync_audiobookshelf_podcast();

    // Wide presentation: area clears the two-column breakpoint and height.
    let wide = Rect::new(0, 0, 200, 50);
    model.app.layout.main.audiobookshelf_podcast_area = wide;
    terminal
        .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
        .unwrap();
    assert!(
        model
            .app
            .layout
            .main
            .audiobookshelf_podcast_right_area
            .width
            > 0
            && model
                .app
                .layout
                .main
                .audiobookshelf_podcast_right_area
                .height
                > 0,
        "wide podcast right area must stay nonzero"
    );
    assert!(
        model.app.layout.main.left_area.width > 0 && model.app.layout.main.left_area.height > 0,
        "wide list_area must project a nonzero left_area"
    );

    // Narrow presentation: below the two-column width threshold.
    let narrow = Rect::new(0, 0, 60, 50);
    model.app.layout.main.audiobookshelf_podcast_area = narrow;
    terminal
        .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
        .unwrap();
    assert_eq!(
        model.app.layout.main.audiobookshelf_podcast_right_area,
        Rect::default(),
        "narrow podcast right area must reset to zero"
    );
    assert!(
        model.app.layout.main.left_area.width > 0 && model.app.layout.main.left_area.height > 0,
        "narrow list_area must project a nonzero left_area"
    );
    assert!(
        !model.app.layout.main.selector_tabs.is_empty(),
        "narrow pill bar must project selector_tabs"
    );

    // Wide -> narrow redraw must also clear the right area.
    model.app.layout.main.audiobookshelf_podcast_area = wide;
    terminal
        .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
        .unwrap();
    assert!(
        model
            .app
            .layout
            .main
            .audiobookshelf_podcast_right_area
            .width
            > 0,
        "wide->narrow must re-establish a nonzero right area"
    );
    model.app.layout.main.audiobookshelf_podcast_area = narrow;
    terminal
        .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
        .unwrap();
    assert_eq!(
        model.app.layout.main.audiobookshelf_podcast_right_area,
        Rect::default(),
        "wide->narrow redraw must reset the right area to zero"
    );
}

/// keep-destination-components-mounted task 3.3: the ABS podcast browser
/// stays mounted across a tab switch and back (keep-mounted, D1).
/// Switching away must not unmount the podcast component, and switching
/// back must re-point the SAME component (not remount), preserving its
/// private selection.
#[test]
fn abs_podcast_stays_mounted_and_preserves_selection_across_switch() {
    let mut app = audiobookshelf_app();
    // A second ABS library of Book kind, so switching tabs changes the
    // active destination component (podcast -> book -> podcast).
    app.audiobookshelf_libraries.push(AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    });
    app.audiobookshelf_book_browse.push(
        crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState::new(
            mbv_core::audiobookshelf::AudiobookshelfLibrary {
                id: "abs-books".into(),
                name: "ABS Books".into(),
                media_type: "book".into(),
            },
        ),
    );
    let mut model = Model::new(app);
    model.sync_audiobookshelf_podcast();
    let id = model
        .abs_podcast_id
        .clone()
        .expect("podcast component mounted");
    let selected_id = |model: &Model| {
        model
            .application
            .get_component(
                &model
                    .abs_podcast_id
                    .clone()
                    .expect("podcast component mounted"),
            )
            .and_then(|comp| {
                comp.as_any()
                    .downcast_ref::<AudiobookshelfPodcastComponent>()
            })
            .and_then(AudiobookshelfPodcastComponent::selected_id)
    };
    // Drive the selection to a non-default value: add a second show, move
    // Down (which selects it), and apply the resulting request to App so
    // content and interaction agree on the second show before the switch.
    model.app.audiobookshelf_browse[0].append_page(
        1,
        20,
        2,
        vec![AudiobookshelfShow {
            library_item_id: "show-b".into(),
            title: "Show B".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    model.push_audiobookshelf_podcast_content();
    let message = model
        .application
        .get_component_mut(&id)
        .expect("podcast component")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            index: 1
        }))
    ));
    model.app.select_audiobookshelf_show(1);
    model.push_audiobookshelf_podcast_content();
    assert_eq!(
        selected_id(&model),
        Some("show-b".into()),
        "component selection must have moved to the second show"
    );

    // Switch to the Book library: the podcast component stays mounted.
    model.app.tab = TabSelection::AudiobookshelfLibrary(1);
    model.sync_audiobookshelf_podcast();
    assert_eq!(model.abs_podcast_id, None);
    assert!(
        model.application.mounted(&id),
        "the podcast browser must stay mounted across the switch"
    );

    // Switch back: the SAME component is re-pointed, still mounted, and
    // its selection is preserved.
    model.app.tab = TabSelection::AudiobookshelfLibrary(0);
    model.sync_audiobookshelf_podcast();
    assert_eq!(
        model.abs_podcast_id.as_ref(),
        Some(&id),
        "re-point must restore the same podcast component id"
    );
    assert!(model.application.mounted(&id));
    assert_eq!(
        selected_id(&model),
        Some("show-b".into()),
        "the podcast selection must survive the switch-and-return round trip"
    );
}

// Task 4.5: the podcast show-move shell arm pulls panel focus to the Library
// (click-to-focus on the component-owned cursor path).
#[test]
fn abs_podcast_show_move_pulls_panel_focus_to_library() {
    let mut model = Model::new(audiobookshelf_app());
    model.sync_audiobookshelf_podcast();
    model.app.panel_focus = PanelFocus::Queue;
    model.handle_terminal_message(
        Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { index: 0 }),
        None,
        &mut false,
        &mut false,
    );
    assert_eq!(model.app.panel_focus, PanelFocus::Library);
}
