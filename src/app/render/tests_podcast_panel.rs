//! Podcast panel-output tests (reorganize-podcast-pill-navigation 4.3). The
//! Audiobookshelf Podcasts destination paints through the shared Wide/Narrow
//! Library panel skeleton over `PodcastContent::content()`, so these
//! assertions target the mounted `LibraryPanel`'s own output — one Selector
//! pill bar, the grouped split-row episode list, and the Workspace-free Wide
//! hero — with one painter per surface in both geometries. They replace the
//! deleted `wide_podcast_*` show-row snapshot tests
//! (`tests_narrow_browse_migration.rs`); the show browser and surname
//! buckets they pinned no longer exist.

use super::test_helpers::buffer_to_string;
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::podcast_content::PodcastContent;
use crate::app::components::LibraryKind;
use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_core::audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfShow,
};
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute};

const DAY: u64 = 24 * 60 * 60;
const NOW: u64 = 30 * DAY;

pub(super) fn podcast_key() -> LibraryKey {
    LibraryKey::Service {
        service: ServiceKind::Audiobookshelf,
        library_id: "abs-podcasts".into(),
        kind: LibraryKind::AudiobookshelfPodcast,
    }
}

fn show(id: &str, title: &str) -> AudiobookshelfShow {
    AudiobookshelfShow {
        library_item_id: id.into(),
        title: title.into(),
        author: None,
        description: None,
        cover_path: None,
    }
}

fn episode(
    show: &str,
    id: &str,
    published_at: Option<u64>,
    duration_seconds: Option<f64>,
) -> AudiobookshelfDownloadedEpisode {
    AudiobookshelfDownloadedEpisode {
        library_item_id: show.into(),
        episode_id: id.into(),
        title: id.into(),
        description: None,
        published_at,
        duration_seconds,
    }
}

/// Two subscribed shows; Alpha's cache holds a dated episode and an undated
/// one, Beta's a single undated one. The All view groups `New` (the dated
/// episode), then `Unknown date` (the rest).
pub(super) fn podcast_owner() -> PodcastContent {
    let mut state = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    });
    state.append_page(
        0,
        20,
        2,
        vec![show("alpha", "Alpha Show"), show("beta", "Beta Show")],
    );
    state.cache_detail(
        "alpha".into(),
        vec![
            episode("alpha", "Dated Episode", Some(NOW - DAY), Some(3600.0)),
            episode("alpha", "Undated Episode", None, Some(1800.0)),
        ],
    );
    state.cache_detail("beta".into(), vec![episode("beta", "Beta One", None, None)]);

    let mut owner = PodcastContent::new();
    owner.set_now_secs(NOW);
    owner.set_content(&state, false);
    owner
}

fn panel_with(owner: PodcastContent, focused: bool) -> LibraryPanel {
    let key = podcast_key();
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(focused));
    panel
}

fn terminal_for(panel: &mut LibraryPanel, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| Component::view(panel, frame, Rect::new(0, 0, width, height)))
        .unwrap();
    terminal
}

/// Whether `needle` appears in any single buffer row inside `area`.
fn area_contains(buf: &ratatui::buffer::Buffer, area: Rect, needle: &str) -> bool {
    (area.top()..area.bottom()).any(|y| {
        let line: String = (area.left()..area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect();
        line.contains(needle)
    })
}

/// How many times `needle` occurs in the whole painted buffer.
fn count_in_buffer(terminal: &Terminal<TestBackend>, needle: &str) -> usize {
    buffer_to_string(terminal).matches(needle).count()
}

/// Wide placement completeness (row 4.3): the Selector row paints the state
/// and show pills, the single-column list paints the grouped split rows, and
/// the Hero pane paints the selected episode — with no Workspace — each with
/// exactly one painter.
#[test]
fn podcast_wide_paints_pill_row_grouped_list_and_workspace_free_hero() {
    let mut panel = panel_with(podcast_owner(), true);
    let terminal = terminal_for(&mut panel, 240, 30);
    let wide = panel
        .test_wide_geometry()
        .expect("the panel painted a Wide skeleton");
    let output = buffer_to_string(&terminal);

    // One Selector pill bar: state pills then one pill per show, all
    // sharing one row (same y — a two-row pill bar fails this).
    let hits = panel.test_selector_hits().regions();
    let first_y = hits.first().map(|(rect, _)| rect.y);
    assert!(!hits.is_empty(), "the Selector row is one pill bar");
    assert!(
        hits.iter()
            .all(|(rect, _)| rect.y == first_y.unwrap() && rect.height == 1),
        "the Selector row is one pill bar"
    );
    assert!(
        wide.controls.is_none(),
        "podcasts have no List controls row"
    );
    assert!(
        output.contains("Played") && output.contains("Unplayed") && output.contains("All"),
        "{output:?}"
    );
    // One painter: each pill label paints once.
    assert_eq!(count_in_buffer(&terminal, "Unplayed"), 1);
    assert_eq!(
        count_in_buffer(&terminal, "Beta Show"),
        2,
        "one pill + one row context: {output:?}"
    );

    // The grouped list paints the age-group headings and the split rows
    // (podcast name in the context role, episode title), each painted exactly
    // once by the one list painter. Library list rows carry no time column.
    assert!(area_contains(
        terminal.backend().buffer(),
        wide.list_area,
        "New"
    ));
    assert!(area_contains(
        terminal.backend().buffer(),
        wide.list_area,
        "Unknown date"
    ));
    assert_eq!(
        count_in_buffer(&terminal, "Dated Episode"),
        2,
        "list row + hero title: {output:?}"
    );
    assert!(
        !area_contains(terminal.backend().buffer(), wide.list_area, "01:00:00"),
        "library list rows paint no duration time"
    );

    // The Wide hero paints the selected episode in the Hero pane — no
    // Workspace box was placed (`wide.workspace` is `None`), so the hero
    // facts' podcast name is the pane's context line.
    assert!(wide.workspace.is_none(), "the hero has no Workspace");
    assert!(area_contains(
        terminal.backend().buffer(),
        wide.hero_area,
        "Dated Episode"
    ));
    assert!(area_contains(
        terminal.backend().buffer(),
        wide.hero_area,
        "Alpha Show"
    ));
    assert!(area_contains(
        terminal.backend().buffer(),
        wide.hero_area,
        "01:00:00"
    ));
}

/// Ordinary non-Wide rows (row 4.3): the Narrow panel paints the same
/// Selector bar and the grouped split rows in one column, and no hero —
/// the episode title paints exactly once because no hero pane repeats it.
#[test]
fn podcast_narrow_paints_ordinary_grouped_rows_and_no_hero() {
    let mut panel = panel_with(podcast_owner(), true);
    let terminal = terminal_for(&mut panel, 80, 30);
    let narrow = panel
        .test_narrow_geometry()
        .expect("the panel painted a Narrow skeleton");
    let output = buffer_to_string(&terminal);

    let hits = panel.test_selector_hits().regions();
    let first_y = hits.first().map(|(rect, _)| rect.y);
    assert!(!hits.is_empty(), "the Selector row is one pill bar");
    assert!(
        hits.iter()
            .all(|(rect, _)| rect.y == first_y.unwrap() && rect.height == 1),
        "the Selector row is one pill bar in Narrow too"
    );
    assert!(
        output.contains("Played") && output.contains("All"),
        "{output:?}"
    );

    // The grouped episode rows are the column's ordinary rows.
    assert!(area_contains(
        terminal.backend().buffer(),
        narrow.list_area,
        "New"
    ));
    assert!(area_contains(
        terminal.backend().buffer(),
        narrow.list_area,
        "Dated Episode"
    ));
    assert_eq!(
        count_in_buffer(&terminal, "Dated Episode"),
        1,
        "no hero repeats the episode title outside Wide: {output:?}"
    );
    // The Workspace-free hero is a Wide-only slot; the Narrow skeleton
    // places no hero pane and no Workspace box.
    assert!(narrow.workspace.is_none());
}
