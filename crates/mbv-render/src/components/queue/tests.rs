use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Autosave sits left of the playlist pill with one space delimiter;
/// dirty then the always-shown owner indicator sit right-anchored; the
/// retained hit covers the playlist only.
#[test]
fn autosave_sits_left_of_playlist_and_dirty_and_owner_sit_right() {
    let area = Rect::new(0, 0, 30, 1);
    let owner = vec![Span::raw("\u{1F5A7} host"), Span::raw(" peer")];
    let playlist = vec![Span::raw("P"), Span::raw("name")];
    let autosave = Some(vec![Span::raw("A")]);
    let dirty = Some(vec![Span::raw("D")]);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|f| {
            let regions = render_queue_status(
                f,
                area,
                owner.clone(),
                playlist.clone(),
                autosave.clone(),
                dirty.clone(),
            );
            assert_eq!(
                regions.playlist,
                Rect::new(3, 0, 5, 1),
                "hit covers the playlist icon plus name only"
            );
        })
        .unwrap();
    let row: String = (0..30)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol().to_string())
        .collect();
    assert!(
        row.starts_with(" A Pname "),
        "autosave, one space, then playlist: {row:?}"
    );
    assert!(
        row.ends_with(" D 🖧 host peer"),
        "dirty, then the owner pill right-anchored: {row:?}"
    );
}

/// The owner indicator paints even without a playlist, autosave, or dirty
/// marker: it always names the shown Player owner (design D7).
#[test]
fn the_owner_pill_paints_when_every_other_pill_is_absent() {
    let area = Rect::new(0, 0, 30, 1);
    let owner = vec![Span::raw("\u{1F5A7}"), Span::raw(" local")];
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|f| {
            let _ = render_queue_status(f, area, owner, Vec::new(), None, None);
        })
        .unwrap();
    let row: String = (0..30)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol().to_string())
        .collect();
    assert!(
        row.trim_end().ends_with("🖧 local"),
        "the owner pill is the footer's only content: {row:?}"
    );
}
