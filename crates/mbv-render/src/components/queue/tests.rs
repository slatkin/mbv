use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Autosave sits left of the playlist pill with one space delimiter;
/// dirty sits right-anchored; the retained hit covers the playlist only.
#[test]
fn autosave_sits_left_of_playlist_and_dirty_sits_right() {
    let area = Rect::new(0, 0, 30, 1);
    let playlist = vec![Span::raw("P"), Span::raw("name")];
    let autosave = Some(vec![Span::raw("A")]);
    let dirty = Some(vec![Span::raw("D")]);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|f| {
            let regions =
                render_queue_status(f, area, playlist.clone(), autosave.clone(), dirty.clone());
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
    assert!(row.ends_with(" D "), "dirty paints right-anchored: {row:?}");
}
