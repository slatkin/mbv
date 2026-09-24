use super::wide::render_wide_media_list;
use super::wide_row_regression_tests_helpers::row_of;
use crate::app::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState, WideMediaList,
};
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// canonical-list-duration-kind 1.2: the painter suppresses the duration
/// slot for `Collection` rows even when one is projected, and paints a
/// `Media` row's duration right-aligned in `DURATION` deep gold.
#[test]
fn collection_row_suppresses_projected_duration_media_row_paints_it() {
    let rect = Rect::new(0, 0, 40, 4);
    let selected_bg = palette::SURFACE_RESTING;
    let dur = Some(crate::app::ui_util::fmt_duration_short(272)); // 4:32
    assert_eq!(dur.as_deref(), Some("4:32"));
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        row_of("coll", "Some Album", dur.clone(), MediaKind::Collection),
        row_of("leaf", "Some Track", dur, MediaKind::Media),
    ]);

    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, selected_bg);
        })
        .unwrap();
    let buf = terminal.backend().buffer();

    let row_text = |y: u16| {
        (0..rect.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
    };
    assert!(
        !row_text(0).contains("4:32"),
        "Collection row must not paint a projected duration: {:?}",
        row_text(0)
    );
    let media = row_text(1);
    assert!(
        media.contains("4:32"),
        "Media row paints its duration: {media:?}"
    );
    assert!(
        media.trim_end().ends_with("4:32"),
        "Media duration is right-aligned: {media:?}"
    );
    let dur_x = rect.width - 4;
    assert_eq!(
        buf[(dur_x, 1)].fg,
        palette::DURATION,
        "Media duration is painted deep gold"
    );
}

/// Split rows paint one palette: context in gold, item title in sage. On
/// a narrow slot the row truncates as one string — the context keeps its
/// width and the single ellipsis lands at the cut.
#[test]
fn episode_row_paints_the_split_row_palette() {
    use crate::app::components::media_list::{MediaListRow, MediaSemanticState};

    let rect = Rect::new(0, 0, 40, 1);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![MediaListRow::Item {
        target: "ep".into(),
        primary: "Severance".into(),
        secondary: Some("Episode Title".into()),
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, 1)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row_text: String = (0..rect.width)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect();
    assert!(
        row_text.contains("Severance Episode Title"),
        "both title parts paint: {row_text:?}"
    );
    // The series title is at the 2-column quiet indent; the episode
    // title starts after it and the one separating space.
    assert_eq!(buf[(2, 0)].fg, palette::SPLIT_ROW_CONTEXT_FG);
    assert_eq!(
        buf[(2 + "Severance ".len() as u16, 0)].fg,
        palette::SPLIT_ROW_TITLE_FG
    );

    // Narrow slot on an unselected row (the selected row marquees):
    // the row truncates as one string — the context part keeps its full
    // budget and the single ellipsis lands at the cut.
    let rect = Rect::new(0, 0, 20, 2);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Item {
            target: "first".into(),
            primary: "First".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "ep".into(),
            primary: "A Very Long Series Name".into(),
            secondary: Some("Episode Title".into()),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
    ]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row_text: String = (0..rect.width)
        .map(|x| buf[(x, 1)].symbol().to_string())
        .collect();
    assert!(
        row_text.contains("A Very Long Se\u{2026}"),
        "the row truncates as one string with one trailing ellipsis: {row_text:?}"
    );
    assert!(
        !row_text.contains("Episode"),
        "the item title is cut with the row: {row_text:?}"
    );
    assert!(row_text.matches('\u{2026}').count() == 1, "{row_text:?}");
    // The truncated context part keeps the split-row context role.
    assert_eq!(buf[(2, 1)].fg, palette::SPLIT_ROW_CONTEXT_FG);
}

/// A row with no secondary title keeps its semantic title role (soft
/// white emphasis, played muted) — the split palette never leaks.
#[test]
fn single_part_rows_keep_the_ordinary_title_role() {
    let rect = Rect::new(0, 0, 40, 2);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Item {
            target: "ordinary".into(),
            primary: "Ordinary Movie".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "played".into(),
            primary: "Played Movie".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Played,
        },
    ]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    assert_eq!(buf[(2, 0)].fg, palette::TEXT_EMPHASIS);
    assert_eq!(buf[(2, 1)].fg, palette::TEXT_MUTED);
    // Played rows deliberately read as generic dim text (TEXT_MUTED);
    // the former "watched, not generic dim" contract was retired
    // 2026-09-19 when PLAYED_ROW_FG/Fog was collapsed into TEXT_MUTED.
}

/// A played split row mutes only the item title (light grey → muted)
/// while the context keeps its soft-white role, so the container stays
/// legible on watched rows.
#[test]
fn played_split_row_mutes_the_item_title_while_context_keeps_gold() {
    let rect = Rect::new(0, 0, 40, 1);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![MediaListRow::Item {
        target: "ep".into(),
        primary: "Severance".into(),
        secondary: Some("Episode Title".into()),
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Played,
    }]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, 1)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    assert_eq!(buf[(2, 0)].fg, palette::SPLIT_ROW_CONTEXT_FG);
    assert_eq!(
        buf[(2 + "Severance ".len() as u16, 0)].fg,
        palette::TEXT_MUTED
    );
}

/// The publish-date gutter is a fixed six-column right-aligned column in
/// its own role, and a row without a date reserves none of it.
#[test]
fn published_date_paints_a_fixed_right_aligned_gutter() {
    let rect = Rect::new(0, 0, 40, 2);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Item {
            target: "dated".into(),
            primary: "Show A".into(),
            secondary: Some("Episode One".into()),
            trailing: Some(MediaListTrailing::Gutter("17 Sep".into())),
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "undated".into(),
            primary: "Show B".into(),
            secondary: Some("Episode Two".into()),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
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
    let dated = row_text(0);
    // The gutter is the last six columns of the row's content: the row is
    // 40 wide with a two-column right inset, so it ends at column 37.
    assert_eq!(&dated[32..38], "17 Sep", "{dated:?}");
    assert_eq!(buf[(32, 0)].fg, palette::STATUS_AVAILABLE);
    assert!(
        !row_text(1).contains("Sep"),
        "a row without a date paints no gutter: {:?}",
        row_text(1)
    );
}

/// A short date still occupies the full fixed gutter, so every dated row's
/// column lines up on the same edge.
#[test]
fn short_published_date_right_aligns_in_the_same_gutter() {
    let rect = Rect::new(0, 0, 40, 1);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![MediaListRow::Item {
        target: "dated".into(),
        primary: "Show A".into(),
        secondary: None,
        trailing: Some(MediaListTrailing::Gutter("3 Sep".into())),
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let text: String = (0..rect.width)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect();
    assert_eq!(&text[32..38], " 3 Sep", "{text:?}");
}

/// Release years and publish dates share the same fixed gutter placement
/// and green role once projected through the unified trailing variant.
#[test]
fn year_and_publish_date_paint_identically_in_the_gutter() {
    let rect = Rect::new(0, 0, 40, 2);
    let mut list: WideMediaList<String> = WideMediaList::new();
    let mut year_item = crate::app::tests::make_item("Year row", "Movie");
    year_item.production_year = 2001;
    let year_trailing = (!year_item.is_folder && year_item.production_year > 0)
        .then(|| MediaListTrailing::Gutter(year_item.production_year.to_string()));
    let published_trailing = Some(MediaListTrailing::Gutter(
        year_item.production_year.to_string(),
    ));
    list.set_content(vec![
        MediaListRow::Item {
            target: "year".into(),
            primary: year_item.name,
            secondary: None,
            trailing: year_trailing,
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "date".into(),
            primary: "Date row".into(),
            secondary: None,
            trailing: published_trailing,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
    ]);

    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    for x in 32..38 {
        assert_eq!(buf[(x, 0)].symbol(), buf[(x, 1)].symbol());
        assert_eq!(buf[(x, 0)].fg, palette::STATUS_AVAILABLE);
        assert_eq!(buf[(x, 0)].fg, buf[(x, 1)].fg);
    }
}

/// Workspace runtimes use the same six-column green gutter as dates and
/// years. Both sub-hour and hour-plus values remain right-aligned without
/// falling back to the Queue's gold duration slot.
#[test]
fn workspace_durations_paint_right_aligned_without_six_column_truncation() {
    let rect = Rect::new(0, 0, 40, 4);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Item {
            target: "track".into(),
            primary: "Track".into(),
            secondary: None,
            trailing: Some(MediaListTrailing::Gutter("59:59".into())),
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "episode".into(),
            primary: "Episode".into(),
            secondary: None,
            trailing: Some(MediaListTrailing::Gutter("1:01".into())),
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "chapter".into(),
            primary: "Chapter".into(),
            secondary: None,
            trailing: Some(MediaListTrailing::Gutter("100:00".into())),
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "book".into(),
            primary: "Book".into(),
            secondary: None,
            trailing: Some(MediaListTrailing::Gutter("1:00".into())),
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        },
    ]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    for (y, expected) in [(0, " 59:59"), (1, "  1:01"), (2, "100:00"), (3, "  1:00")] {
        let gutter: String = (32..38).map(|x| buf[(x, y)].symbol().to_string()).collect();
        assert_eq!(gutter, expected, "gutter row {y}");
        let first = 38 - expected.chars().count() as u16;
        for x in first..38 {
            assert_eq!(buf[(x, y)].fg, palette::STATUS_AVAILABLE);
        }
    }
}
