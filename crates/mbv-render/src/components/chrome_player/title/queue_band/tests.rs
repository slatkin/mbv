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

/// The shared render context for both seek-row painters.
fn seek_context<'a>(
    area: Rect,
    player_h: u16,
    panel: palette::Surface,
    progress_ticks: (i64, i64),
    playback: &'a mut PlaybackStripAreas,
    marquee_text: &'a mut String,
    marquee_started_at: &'a mut std::time::Instant,
) -> PlaybackRenderContext<'a> {
    PlaybackRenderContext {
        area,
        playback,
        player_h,
        controls: PlaybackControls {
            show: true,
            use_nerd_fonts: false,
            availability: TransportAvailability::default(),
            panel_focused: false,
            progress: (progress_ticks.0, progress_ticks.1, false),
            idle_feed_title: None,
        },
        now_playing_title: None,
        panel,
        status_indicators: None,
        title_parts: None,
        marquee_text,
        marquee_started_at,
    }
}

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
    let mut ctx = seek_context(
        row,
        row.height,
        palette::Surface::QueueOnlyPlaybackPanel,
        (position_ticks, runtime_ticks),
        &mut playback,
        &mut marquee_text,
        &mut marquee_started_at,
    );
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
            let ctx = seek_context(
                Rect::new(0, 0, 40, 1),
                1,
                palette::Surface::PlaybackPanel,
                (position_ticks, runtime_ticks),
                &mut playback,
                &mut marquee_text,
                &mut marquee_started_at,
            );
            render_player_panel(frame, ctx);
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

/// The columns carrying the Queue bar's unplayed track background: the
/// backdrop slate (#272e33), not the shared `PROGRESS_TRACK` grey the
/// Library strip keeps.
fn track_columns(cells: &[(String, Color, Color)]) -> Vec<usize> {
    cells
        .iter()
        .enumerate()
        .filter(|(_, (_, _, bg))| *bg == palette::SURFACE_BACKDROP)
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
    paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 75 * TPS, 300 * TPS);
    let text = row_text(&row_cells(terminal.current_buffer_mut(), 40));
    assert!(
        text.starts_with(" 1:15 "),
        "elapsed left of the bar: {text:?}"
    );
    assert!(text.ends_with(" 5:00 "), "total right of the bar: {text:?}");
}

// Migrated from `queue_playback_panel/tests.rs` (7fdb7fee8): a quarter of the
// bar is accent fill.
#[test]
fn seek_row_fills_a_quarter_of_the_bar_in_accent() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 75 * TPS, 300 * TPS);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    let full = cells
        .iter()
        .filter(|(symbol, fg, _)| symbol == FULL && *fg == palette::ACCENT)
        .count();
    assert_eq!(full, usize::from(seekbar.width) / 4);
}

// Migrated from `queue_playback_panel/tests.rs` (7fdb7fee8): the whole bar sits
// over the muted track.
#[test]
fn seek_row_paints_the_whole_bar_over_the_track() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 75 * TPS, 300 * TPS);
    let cells = row_cells(terminal.current_buffer_mut(), 40);
    assert_eq!(track_columns(&cells).len(), usize::from(seekbar.width));
}

// Migrated from `queue_playback_panel/tests.rs` (7fdb7fee8): no Gauge
// percentage label or border on the bar.
#[test]
fn seek_row_has_no_percentage_label_or_border() {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 40, 1), 75 * TPS, 300 * TPS);
    let text = row_text(&row_cells(terminal.current_buffer_mut(), 40));
    assert!(!text.contains('%'), "no Gauge percentage label: {text:?}");
    assert!(!text.contains('\u{2502}'), "no border on the bar: {text:?}");
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

/// Paint 10 seconds of a 45-minute episode and return the bar and its cells.
fn paint_early_progress() -> (Rect, Vec<(String, Color, Color)>) {
    let mut terminal = Terminal::new(TestBackend::new(37, 1)).unwrap();
    let seekbar = paint_queue_seek_row(
        &mut terminal,
        Rect::new(0, 0, 37, 1),
        10 * TPS,
        45 * 60 * TPS,
    );
    (seekbar, row_cells(terminal.current_buffer_mut(), 37))
}

// Regression for this change (`queue-seekbar-fractional-progress`): 10 seconds
// of a 45-minute episode shows one eighth of a cell. The old whole-cell
// rounding left it empty.
#[test]
fn early_progress_paints_a_one_eighth_edge() {
    let (seekbar, cells) = paint_early_progress();
    let leading = &cells[usize::from(seekbar.x)];
    assert_eq!(
        (leading.0.as_str(), leading.1),
        (ONE_EIGHTH, palette::ACCENT)
    );
}

// Regression for this change: the one-eighth edge is the only fill.
#[test]
fn early_progress_fills_no_whole_cell() {
    let (_, cells) = paint_early_progress();
    let full = cells.iter().filter(|(symbol, _, _)| symbol == FULL).count();
    assert_eq!(full, 0);
}

// Regression for this change: the bar is one muted track at its full width.
#[test]
fn early_progress_keeps_the_bar_one_muted_track() {
    let (seekbar, cells) = paint_early_progress();
    assert_eq!(track_columns(&cells).len(), usize::from(seekbar.width));
}

/// Paint 7s of 16s (3.5 cells of the bar) and return the bar and its cells.
fn paint_three_and_a_half_cells() -> (Rect, Vec<(String, Color, Color)>) {
    let mut terminal = Terminal::new(TestBackend::new(20, 1)).unwrap();
    let seekbar = paint_queue_seek_row(&mut terminal, Rect::new(0, 0, 20, 1), 7 * TPS, 16 * TPS);
    (seekbar, row_cells(terminal.current_buffer_mut(), 20))
}

// Spec scenario: completed cells precede the fractional leading cell.
#[test]
fn completed_cells_precede_the_fractional_leading_cell() {
    let (seekbar, cells) = paint_three_and_a_half_cells();
    let start = usize::from(seekbar.x);
    let symbols: Vec<&str> = cells[start..start + 3]
        .iter()
        .map(|c| c.0.as_str())
        .collect();
    assert_eq!(symbols, [FULL; 3]);
}

// Spec scenario: the cell after the completed ones is half filled.
#[test]
fn the_cell_after_completed_cells_is_half_filled() {
    let (seekbar, cells) = paint_three_and_a_half_cells();
    let next = &cells[usize::from(seekbar.x) + 3];
    assert_eq!((next.0.as_str(), next.1), (HALF, palette::ACCENT));
}

// Spec scenario: the remainder of the bar stays track.
#[test]
fn the_rest_of_the_bar_stays_track_after_the_fractional_cell() {
    let (seekbar, cells) = paint_three_and_a_half_cells();
    assert_eq!(track_columns(&cells).len(), usize::from(seekbar.width));
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

/// Paint a late position, then repaint an early one over the same cells.
fn repaint_backward() -> (Rect, Rect, Vec<(String, Color, Color)>) {
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    let row = Rect::new(0, 0, 40, 1);
    let late = paint_queue_seek_row(&mut terminal, row, 200 * TPS, 300 * TPS);
    let early = paint_queue_seek_row(&mut terminal, row, TPS, 300 * TPS);
    (late, early, row_cells(terminal.current_buffer_mut(), 40))
}

// Regression for this change (`queue-seekbar-fractional-progress`): repainting
// back to an earlier position keeps the bar rectangle.
#[test]
fn backward_repaint_keeps_the_bar_rect() {
    let (late, early, _) = repaint_backward();
    assert_eq!(early, late);
}

// Regression for this change (`queue-seekbar-fractional-progress`): repainting
// back to an earlier position clears the obsolete accent fill.
#[test]
fn backward_repaint_clears_obsolete_fill() {
    let (_, early, cells) = repaint_backward();
    assert_eq!(filled_columns(&cells), vec![usize::from(early.x)]);
}

// Regression for this change: the cleared cells return to the muted track.
#[test]
fn backward_repaint_restores_the_track() {
    let (_, early, cells) = repaint_backward();
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
    let unfilled = cells
        .iter()
        .filter(|(symbol, fg, _)| symbol == UPPER_LINE && *fg == palette::PROGRESS_TRACK)
        .count();
    assert_eq!(
        unfilled, 30,
        "the Library bar keeps the shared progress-track grey"
    );
    assert!(
        !text.contains(FULL) && !text.contains(ONE_EIGHTH),
        "no Queue fractional blocks on the Library bar: {text:?}"
    );
}
