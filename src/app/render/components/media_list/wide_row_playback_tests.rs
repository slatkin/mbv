use super::wide::render_wide_media_list;
use super::wide_row_regression_tests_helpers::item;
use crate::app::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState, WideMediaList,
};
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// A row on the canonical Iris bar paints the bar's own foregrounds: Ink
/// for the text it carries, and the bar's darker progress role for a
/// resume percentage — the ordinary orange does not read on the light
/// bar. Both resume-carrying states ride the same path.
#[test]
fn selected_row_paints_its_own_progress_role_for_every_progress_state() {
    use crate::app::components::media_list::ActiveProgress;

    for state in [
        MediaSemanticState::NowPlaying {
            progress: Some(ActiveProgress::new(47)),
        },
        MediaSemanticState::Active {
            progress: Some(ActiveProgress::new(12)),
        },
    ] {
        let now_playing = matches!(state, MediaSemanticState::NowPlaying { .. });
        let rect = Rect::new(0, 0, 40, 1);
        let mut list: WideMediaList<String> = WideMediaList::new();
        list.set_content(vec![MediaListRow::Item {
            target: "playing".into(),
            primary: "Playing title".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: state,
        }]);

        let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
        terminal
            .draw(|f| {
                render_wide_media_list(f, rect, rect, &mut list, true, palette::SELECTED_ROW_BG);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        match (0..rect.width).find(|&x| buf[(x, 0)].symbol() == "▶") {
            Some(marker_x) => {
                assert!(now_playing, "only a now-playing row carries the marker");
                assert_eq!(buf[(marker_x, 0)].fg, palette::SELECTED_ROW_FG);
            }
            None => assert!(!now_playing, "the now-playing marker must paint"),
        }
        let progress_x = (0..rect.width)
            .find(|&x| matches!(buf[(x, 0)].symbol(), "4" | "1"))
            .expect("selected progress percentage");
        for x in progress_x..progress_x + 2 {
            assert_eq!(
                buf[(x, 0)].fg,
                palette::SELECTED_ROW_PROGRESS_FG,
                "the bar carries its own progress role"
            );
        }
    }
}

#[test]
fn play_marker_only_paints_for_now_playing_rows() {
    use crate::app::components::media_list::ActiveProgress;

    let rect = Rect::new(0, 0, 40, 2);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Item {
            target: "resume".into(),
            primary: "Resume title".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Active {
                progress: Some(ActiveProgress::new(12)),
            },
        },
        MediaListRow::Item {
            target: "playing".into(),
            primary: "Playing title".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::NowPlaying {
                progress: Some(ActiveProgress::new(47)),
            },
        },
    ]);

    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row_text = |y: u16| {
        (0..rect.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
    };

    let resume = row_text(0);
    assert!(
        !resume.contains("▶ "),
        "resume rows have no play marker: {resume:?}"
    );
    assert!(resume.contains("Resume title"));

    let playing = row_text(1);
    assert!(
        playing.contains("▶ Playing title"),
        "now-playing marker: {playing:?}"
    );
}

/// Queue now-playing rows paint their total duration like every other
/// row — no throbber slot — while resume rows retain their inline badge
/// and duration. Keep both cases in one buffer regression so a painter
/// match cannot silently change the browser resume presentation while
/// touching the queue presentation.
#[test]
fn now_playing_row_paints_duration_like_other_rows() {
    use crate::app::components::media_list::{ActiveProgress, MediaListRow, MediaSemanticState};

    let rect = Rect::new(0, 0, 52, 3);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Item {
            target: "playing".into(),
            primary: "Playing title".into(),
            secondary: None,
            trailing: Some(MediaListTrailing::Gutter("2001".into())),
            duration: Some("2:00".into()),
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::NowPlaying {
                progress: Some(ActiveProgress::new(47)),
            },
        },
        MediaListRow::Item {
            target: "resume".into(),
            primary: "Resume title".into(),
            secondary: None,
            trailing: Some(MediaListTrailing::Gutter("2001".into())),
            duration: Some("2:00".into()),
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Active {
                progress: Some(ActiveProgress::new(12)),
            },
        },
    ]);

    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row_text = |y: u16| {
        (0..rect.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
    };

    let playing = row_text(0);
    assert!(playing.contains("Playing title"));
    assert!(
        !playing.contains('▌'),
        "no throbber glyph on the now-playing row: {playing:?}"
    );
    assert!(
        playing.contains("Playing title 47%"),
        "live progress rides the trailing text like other rows: {playing:?}"
    );
    assert!(
        playing.contains("2001"),
        "the production-year gutter follows inline progress: {playing:?}"
    );
    assert!(
        playing.contains("2:00"),
        "total time shows like other rows: {playing:?}"
    );
    let progress_x = 18;
    assert_eq!(buf[(progress_x, 0)].symbol(), "4");
    assert_eq!(buf[(progress_x, 0)].fg, palette::PROGRESS_PERCENT);
    let gutter_x = 38;
    assert_eq!(buf[(gutter_x, 0)].symbol(), " ");
    assert_eq!(buf[(gutter_x + 1, 0)].symbol(), " ");
    assert_eq!(buf[(gutter_x + 2, 0)].symbol(), "2");
    assert_eq!(buf[(gutter_x + 2, 0)].fg, palette::STATUS_AVAILABLE);
    let duration_x = rect.width - 2 - 4;
    assert_eq!(buf[(duration_x, 0)].symbol(), "2");
    assert_eq!(buf[(duration_x, 0)].fg, palette::DURATION);

    let resume = row_text(1);
    assert!(resume.contains("Resume title 12%"));
    assert!(resume.contains("2001"));
    assert!(resume.contains("2:00"));
    let progress_x = 15;
    assert_eq!(buf[(progress_x, 1)].symbol(), "1");
    assert_eq!(buf[(progress_x, 1)].fg, palette::PROGRESS_PERCENT);
    assert_eq!(buf[(gutter_x + 2, 1)].symbol(), "2");
    assert_eq!(buf[(gutter_x + 2, 1)].fg, palette::STATUS_AVAILABLE);
    let duration_x = rect.width - 2 - 4;
    assert_eq!(buf[(duration_x, 1)].symbol(), "2");
    assert_eq!(buf[(duration_x, 1)].fg, palette::DURATION);
}

#[test]
fn now_playing_duration_and_narrow_reserves_are_safe() {
    use crate::app::components::media_list::{ActiveProgress, MediaListRow, MediaSemanticState};

    // A now-playing row keeps its duration even when the title must give
    // way: duration and trailing progress share the row exactly like an
    // ordinary active row.
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![MediaListRow::Item {
        target: "playing".into(),
        primary: "A very long title that must be truncated".into(),
        secondary: None,
        trailing: None,
        duration: Some("2:00".into()),
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::NowPlaying {
            progress: Some(ActiveProgress::new(100)),
        },
    }]);
    let mut terminal = Terminal::new(TestBackend::new(20, 1)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(
                f,
                Rect::new(0, 0, 20, 1),
                Rect::new(0, 0, 20, 1),
                &mut list,
                true,
                palette::SURFACE_RESTING,
            );
        })
        .unwrap();
    let text = (0..20)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect::<String>();
    assert!(
        text.contains("2:00"),
        "duration survives truncation: {text:?}"
    );
    assert!(
        text.contains("100%"),
        "trailing progress shares the row: {text:?}"
    );

    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![MediaListRow::Item {
        target: "playing".into(),
        primary: "Focused".into(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::NowPlaying { progress: None },
    }]);
    let mut terminal = Terminal::new(TestBackend::new(1, 1)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(
                f,
                Rect::new(0, 0, 1, 1),
                Rect::new(0, 0, 1, 1),
                &mut list,
                true,
                palette::SURFACE_RESTING,
            );
        })
        .unwrap();
}

/// The duration right-aligns to 2 columns from the panel edge whether or
/// not the focused list overflows and reserves a scrollbar column — the
/// scrollbar must not shift it another column inwards.
#[test]
fn duration_right_inset_is_two_columns_with_and_without_scrollbar() {
    let selected_bg = palette::SURFACE_RESTING;
    for (rows_count, focused) in [(3usize, false), (3, true), (12, true)] {
        let rect = Rect::new(0, 0, 40, 4);
        let mut list: WideMediaList<String> = WideMediaList::new();
        list.set_content(
            (0..rows_count)
                .map(|i| item(&format!("t{i}"), &format!("Entry {i}"), Some("4:32".into())))
                .collect(),
        );

        let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
        terminal
            .draw(|f| {
                render_wide_media_list(f, rect, rect, &mut list, focused, selected_bg);
            })
            .unwrap();
        let buf = terminal.backend().buffer();

        // "4:32" ends 2 columns before the panel's right edge; the two
        // cells before it are the quiet gap.
        let dur_start = rect.width - 2 - 4;
        for (i, ch) in "4:32".chars().enumerate() {
            assert_eq!(
                buf[(dur_start + i as u16, 0)].symbol(),
                ch.to_string(),
                "duration at fixed inset (rows={rows_count}, focused={focused}, i={i})"
            );
        }
        let gap_x = rect.width - 2 - 4 - 1;
        assert_eq!(
            buf[(gap_x, 0)].symbol(),
            " ",
            "quiet gap before duration (rows={rows_count}, focused={focused})"
        );
    }
}
