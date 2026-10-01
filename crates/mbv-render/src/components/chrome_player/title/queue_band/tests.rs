use super::*;

use crate::components::chrome_player::{
    PlaybackControls, PlaybackStripAreas, TransportAvailability,
};
use crate::render_player_panel;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

/// Emby ticks per second, for readable fixtures.
const TPS: i64 = mbv_emby_model::TICKS_PER_SECOND;

/// The Queue bar's full block, half block, and one-eighth block.
const FULL: &str = "\u{2588}";
const HALF: &str = "\u{258c}";
const ONE_EIGHTH: &str = "\u{258f}";
/// The Library strip's thin upper-line character.
const UPPER_LINE: &str = "\u{2594}";

/// Paint one Queue seek row into the terminal's current buffer. Painting via
/// `get_frame` (no buffer swap) lets repeated calls repaint the same cells,
/// which is how the stale-fill guard observes a backward seek.
fn paint_queue_seek_row(
    terminal: &mut Terminal<TestBackend>,
    row: Rect,
    position_ticks: i64,
    runtime_ticks: i64,
) -> Rect {
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    let mut ctx = PlaybackRenderContext {
        area: row,
        playback: &mut playback,
        player_h: row.height,
        controls: PlaybackControls {
            show: true,
            use_nerd_fonts: false,
            availability: TransportAvailability::default(),
            panel_focused: false,
            progress: (position_ticks, runtime_ticks, false),
            idle_feed_title: None,
        },
        now_playing_title: None,
        panel: palette::Surface::QueueOnlyPlaybackPanel,
        status_indicators: None,
        title_parts: None,
        marquee_text: &mut marquee_text,
        marquee_started_at: &mut marquee_started_at,
    };
    let panel_bg = palette::surface_colors(ctx.panel, ctx.controls.panel_focused).fill;
    let mut frame = terminal.get_frame();
    render_queue_seek_row(&mut frame, row, &mut ctx, panel_bg);
    playback.seekbar
}

/// Paint the Library strip (not the Queue band) and return its seekbar row.
fn paint_library_seek_row(position_ticks: i64, runtime_ticks: i64) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    terminal
        .draw(|frame| {
            render_player_panel(
                frame,
                PlaybackRenderContext {
                    area: Rect::new(0, 0, 40, 1),
                    playback: &mut playback,
                    player_h: 1,
                    controls: PlaybackControls {
                        show: true,
                        use_nerd_fonts: false,
                        availability: TransportAvailability::default(),
                        panel_focused: false,
                        progress: (position_ticks, runtime_ticks, false),
                        idle_feed_title: None,
                    },
                    now_playing_title: None,
                    panel: palette::Surface::PlaybackPanel,
                    status_indicators: None,
                    title_parts: None,
                    marquee_text: &mut marquee_text,
                    marquee_started_at: &mut marquee_started_at,
                },
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

/// One painted row: each cell's symbol, foreground, and background.
fn row_cells(buf: &Buffer, width: u16) -> Vec<(String, Color, Color)> {
    (0..width)
        .map(|x| {
            let cell = &buf[(x, 0)];
            (cell.symbol().to_string(), cell.fg, cell.bg)
        })
        .collect()
}

/// The row's concatenated symbols.
fn row_text(cells: &[(String, Color, Color)]) -> String {
    cells.iter().map(|(symbol, _, _)| symbol.as_str()).collect()
}

/// Whether a cell carries solid accent progress: a non-space glyph painted in
/// the accent foreground, or Gauge's centre label slot (a space over the
/// accent background).
fn is_accent_filled(cell: &(String, Color, Color)) -> bool {
    let (symbol, fg, bg) = cell;
    (*fg == palette::ACCENT && symbol != " ") || *bg == palette::ACCENT
}

/// The columns carrying the Queue bar's muted track background.
fn track_columns(cells: &[(String, Color, Color)]) -> Vec<usize> {
    cells
        .iter()
        .enumerate()
        .filter(|(_, (_, _, bg))| *bg == palette::PROGRESS_TRACK)
        .map(|(x, _)| x)
        .collect()
}

/// The columns carrying solid accent progress.
fn filled_columns(cells: &[(String, Color, Color)]) -> Vec<usize> {
    cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| is_accent_filled(cell))
        .map(|(x, _)| x)
        .collect()
}

// Migrated from `queue_playback_panel/tests.rs` (7fdb7fee8): the seekbar sits
// between its elapsed and total labels, one space each side.
#[test]
fn seek_row_flanks_the_bar_with_elapsed_and_total() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 75 * TPS, 300 * TPS);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    let text = row_text(&cells);
    assert!(
        text.starts_with(" 1:15 "),
        "elapsed left of the bar: {text:?}"
    );
    assert!(text.ends_with(" 5:00 "), "total right of the bar: {text:?}");
    assert_eq!(seekbar, Rect::new(6, 0, 28, 1));
}

// Migrated from `queue_playback_panel/tests.rs` (7fdb7fee8): a quarter of the
// bar is accent fill over the muted track, and no percentage label appears.
#[test]
fn seek_row_fills_a_quarter_of_the_bar_in_accent_over_the_track() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 75 * TPS, 300 * TPS);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    let full = cells
        .iter()
        .filter(|(symbol, fg, _)| symbol == FULL && *fg == palette::ACCENT)
        .count();
    assert_eq!(full, 7, "a quarter of the 28-cell bar is filled");
    assert_eq!(track_columns(&cells).len(), 28, "the whole bar is track");
    assert!(
        !row_text(&cells).contains('%'),
        "no Gauge percentage label: {:?}",
        row_text(&cells)
    );
}

// Migrated from `queue_playback_panel/tests.rs` (7fdb7fee8): the retained seek
// rect covers exactly the painted bar span, never the flanking labels.
#[test]
fn seek_hit_rect_covers_only_the_bar_not_the_flanking_times() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 75 * TPS, 300 * TPS);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    let bar = track_columns(&cells);
    assert_eq!(
        (usize::from(seekbar.x), usize::from(seekbar.right())),
        (*bar.first().unwrap(), bar.last().unwrap() + 1),
        "the bar span seeks, the time labels never do"
    );
}

// Regression for this change (`queue-seekbar-fractional-progress`): 10 seconds
// of a 45-minute episode in a 24-cell bar shows one eighth of a cell. The old
// whole-cell rounding left it empty.
#[test]
fn early_progress_paints_a_one_eighth_edge() {
    let mut terminal = Terminal::new(TestBackend::new(37, 1)).unwrap();
    let seekbar = paint_queue_seek_row(
        &mut terminal,
        Rect::new(0, 0, 37, 1),
        10 * TPS,
        45 * 60 * TPS,
    );
    assert_eq!(seekbar.width, 24, "the bar keeps its available width");
    let cells = row_cells(terminal.current_buffer_mut(), 37);
    let start = usize::from(seekbar.x);
    assert_eq!(cells[start].0, ONE_EIGHTH, "the leading edge is one eighth");
    assert_eq!(cells[start].1, palette::ACCENT);
    assert_eq!(
        track_columns(&cells).len(),
        24,
        "the bar is one muted track"
    );
    let full = cells.iter().filter(|(symbol, _, _)| symbol == FULL).count();
    assert_eq!(full, 0, "no whole cell fills at one eighth");
    let text = row_text(&cells);
    assert!(!text.contains('%'), "no Gauge percentage label: {text:?}");
    assert!(!text.contains('\u{2502}'), "no border on the bar: {text:?}");
}

// Spec scenario: completed cells precede the fractional leading cell.
#[test]
fn completed_cells_precede_the_fractional_leading_cell() {
    let mut terminal = Terminal::new(TestBackend::new(20, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 20, 1), 7 * TPS, 16 * TPS);
    assert_eq!(seekbar.width, 8);
    let cells = row_cells(terminal.current_buffer_mut(), 20);
    let start = usize::from(seekbar.x);
    assert_eq!(cells[start].0, FULL);
    assert_eq!(cells[start + 1].0, FULL);
    assert_eq!(cells[start + 2].0, FULL);
    assert_eq!(cells[start + 3].0, HALF, "the next cell is half filled");
    assert_eq!(cells[start + 3].1, palette::ACCENT);
    assert_eq!(track_columns(&cells).len(), 8, "the rest stays track");
    assert!(!row_text(&cells).contains('%'), "no percentage label");
}

// Spec scenario: a non-positive runtime leaves the whole bar as muted track.
#[test]
fn a_nonpositive_runtime_paints_an_empty_bounded_bar() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 30 * TPS, 0);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    assert_eq!(track_columns(&cells).len(), 28, "the whole bar is track");
    assert!(filled_columns(&cells).is_empty(), "no accent fill");
    assert!(seekbar.width > 0 && seekbar.right() <= 40);
}

// Spec scenario: a non-positive position leaves the whole bar as muted track.
#[test]
fn a_nonpositive_position_paints_an_empty_bounded_bar() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 0, 300 * TPS);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    assert_eq!(track_columns(&cells).len(), 28, "the whole bar is track");
    assert!(filled_columns(&cells).is_empty(), "no accent fill");
}

// Spec scenario: a position at or beyond runtime fills the bar without an
// extra cell beyond its bounds.
#[test]
fn a_position_beyond_runtime_fills_the_bar_without_spilling() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 400 * TPS, 300 * TPS);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    let filled = filled_columns(&cells);
    assert_eq!(filled.len(), usize::from(seekbar.width), "the bar is full");
    assert_eq!(filled.first().copied(), Some(usize::from(seekbar.x)));
    assert_eq!(
        filled.last().copied(),
        Some(usize::from(seekbar.right()) - 1),
        "no accent spills past the bar"
    );
}

// Regression for this change (`queue-seekbar-fractional-progress`): repainting
// back to an earlier position clears the obsolete accent fill.
#[test]
fn backward_repaint_clears_obsolete_fill() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let row = Rect::new(0, 0, 40, 1);
    let late = paint_queue_seek_row(&mut terminal, row, 200 * TPS, 300 * TPS);
    assert!(
        filled_columns(&row_cells(terminal.current_buffer_mut(), 40)).len() > 1,
        "the later position fills several cells"
    );
    let early = paint_queue_seek_row(&mut terminal, row, TPS, 300 * TPS);
    assert_eq!(early, late, "the bar rectangle is unchanged");
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    let start = usize::from(early.x);
    assert_eq!(cells[start].0, ONE_EIGHTH, "the early edge is one eighth");
    assert_eq!(
        filled_columns(&cells),
        vec![start],
        "only the leading cell stays filled"
    );
    assert_eq!(track_columns(&cells).len(), usize::from(early.width));
}

// Scope guard: the Library strip keeps its thin upper-line bar and whole-cell
// rounding; the Queue fractional blocks never reach it.
#[test]
fn library_strip_keeps_its_thin_whole_cell_bar() {
    let cells = row_cells(&paint_library_seek_row(12 * TPS, 50 * TPS), 40);
    let text = row_text(&cells);
    assert_eq!(
        text.matches(UPPER_LINE).count(),
        40,
        "the thin upper-line bar spans the row: {text:?}"
    );
    let accent = cells
        .iter()
        .filter(|(symbol, fg, _)| symbol == UPPER_LINE && *fg == palette::ACCENT)
        .count();
    assert_eq!(accent, 10, "0.24 of 40 whole cells rounds up to 10");
    assert!(
        !text.contains(FULL) && !text.contains(ONE_EIGHTH),
        "no Queue fractional blocks on the Library bar: {text:?}"
    );
}
