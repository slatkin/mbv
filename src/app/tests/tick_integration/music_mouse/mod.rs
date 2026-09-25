use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::list::tree_browser::TreeOperation;
use crate::app::components::music_tree_target::MusicTreeTarget;
use crate::app::components::{ComponentId, ModalId, Msg, ShellRequest, UserEvent};
use crate::app::render::{make_music_group_app, make_music_group_app_with_second_album};
use crate::app::tests::make_item;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};

mod tree_click_context;
mod tree_double_click;
mod tree_track_activation;
mod wide_track_table;

// ── Row 5.2/5.3: filtered tree pointer routing and grouped-track playback ──

/// Dispatch every surviving message of one step through the shell and re-run
/// the production sync pass.
fn dispatch_step(harness: &mut TickHarness) {
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
}

fn draw_frame(harness: &mut TickHarness) {
    harness.model_mut().sync_mounted_surfaces();
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .expect("music frame");
}

/// Inject one key through the real router, dispatch every surviving message
/// through the shell, and re-run the production sync pass.
fn inject_key(harness: &mut TickHarness, code: Key) {
    harness.inject(Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }));
    dispatch_step(harness);
}

fn left_click(column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn music_panel(harness: &TickHarness) -> &crate::app::components::library_panel::LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("LibraryPanel")
}

/// The settled album leaf's stable target, resolved from the tree projection
/// without needing a completed frame.
fn album_target(harness: &TickHarness, target: &str) -> MusicTreeTarget {
    let music = harness.model().test_music_owner();
    music
        .browser
        .visible_targets()
        .into_iter()
        .find(|candidate| candidate.album_leaf_target() == Some(target))
        .expect("projected album node")
}

// ── Row 5.5: double-click and track-activation tick coverage ──

/// The two expandable tree rows a double-click case can claim.
#[derive(Clone, Copy, Debug)]
enum TreeDoubleClickNode {
    ArtistRoot,
    AlbumLeaf,
}

impl TreeDoubleClickNode {
    /// The case's settled node, resolved from the projection by identity.
    fn resolve(self, harness: &TickHarness) -> MusicTreeTarget {
        let music = harness.model().test_music_owner();
        music
            .browser
            .visible_targets()
            .into_iter()
            .find(|candidate| match self {
                TreeDoubleClickNode::ArtistRoot => candidate.is_artist(),
                TreeDoubleClickNode::AlbumLeaf => candidate.album_leaf_target() == Some("album-1"),
            })
            .expect("the case's projected tree node")
    }

    /// The filter query that keeps this node's own row visible.
    fn filter_query(self) -> &'static str {
        match self {
            TreeDoubleClickNode::ArtistRoot => "Alpha",
            TreeDoubleClickNode::AlbumLeaf => "First Album",
        }
    }
}

/// One cached `album-1` track for the tree-track activation cases.
fn cached_track(id: &str, number: i64) -> mbv_core::api::EmbyItem {
    let mut track = make_item(&format!("Track {number}"), "Audio");
    track.id = id.into();
    track.album_id = "album-1".into();
    track.media_type = "Audio".into();
    track.index_number = number;
    track
}

/// The painted point of any tree row, resolved from the tree's completed
/// frame through the shared read-only stable-target row geometry.
fn tree_node_point(harness: &TickHarness, target: &MusicTreeTarget) -> (u16, u16) {
    let music = harness.model().test_music_owner();
    let list_area = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_list_rect())
        .expect("painted tree list area");
    let point = (list_area.y..list_area.bottom())
        .flat_map(|y| (list_area.x..list_area.right()).map(move |x| Position::new(x, y)))
        .find(|point| music.browser.resolve_current_point(*point) == Some(target))
        .expect("painted tree row");
    (point.x, point.y)
}

/// Whether the shared confirm modal is mounted in the current composition.
fn confirm_mounted(harness: &TickHarness) -> bool {
    harness
        .model()
        .application
        .mounted(&ComponentId::Modal(ModalId::Confirm))
}

/// A Wide `make_music_group_app` whose `album-1` leaf owns two cached tracks,
/// ready for a tree-track activation case.
fn music_tree_track_app(autoload: bool) -> crate::app::App {
    let mut app = make_music_group_app();
    app.config.lock().unwrap().autoload = autoload;
    app.terminal_width = 160;
    app.terminal_height = 40;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    app.album_tracks_cache.insert(
        "album-1".into(),
        vec![cached_track("track-1", 1), cached_track("track-2", 2)],
    );
    app
}

/// The same app with the album leaf expanded and its track rows painted.
fn expanded_track_harness(app: crate::app::App) -> TickHarness {
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let album_id = album_target(&harness, "album-1");
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(album_id.clone()));
    draw_frame(&mut harness);
    harness
}

fn expanded_music_tree_track_harness(autoload: bool) -> TickHarness {
    expanded_track_harness(music_tree_track_app(autoload))
}

/// The playback-target queue's item ids in queue order.
fn playback_queue_ids(harness: &TickHarness) -> Vec<String> {
    harness
        .model()
        .app
        .playback_queue()
        .emby_items()
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

/// The two tree-track activation routes that share the grouped resolver.
#[derive(Clone, Copy, Debug)]
enum TrackActivation {
    Enter,
    DoubleClick,
}

/// Select the painted track row, then activate it by the case's route.
fn activate_track(harness: &mut TickHarness, at: (u16, u16), kind: TrackActivation) {
    harness.inject(left_click(at.0, at.1));
    dispatch_step(harness);
    draw_frame(harness);
    match kind {
        TrackActivation::Enter => inject_key(harness, Key::Enter),
        TrackActivation::DoubleClick => {
            harness.inject(left_click(at.0, at.1));
            dispatch_step(harness);
        }
    }
}
