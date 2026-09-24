use super::wide::{
    render_wide_media_list, render_wide_media_list_component, render_wide_media_list_with_zebra,
};
use super::wide_row_regression_tests_helpers::{heading, item, paint};
use crate::app::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState, WideMediaList,
    WideMediaListPaintPolicy, ZebraStripe,
};
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

fn stripe(surface: palette::Surface) -> ZebraStripe {
    ZebraStripe {
        focused: palette::surface_colors(surface, true).fill,
        unfocused: palette::surface_colors(surface, false).fill,
    }
}
#[test]
fn selected_row_spans_full_width_with_two_col_indent() {
    const PX: u16 = 10;
    const PW: u16 = 40;
    let selected_bg = palette::SURFACE_RESTING;

    for duration in [None, Some("1:05".to_string())] {
        let mut list: WideMediaList<String> = WideMediaList::new();
        list.set_content(vec![
            item("sel", "Selected Entry", duration.clone()),
            item("other", "Other Entry", None),
        ]);

        let mut terminal = Terminal::new(TestBackend::new(60, 6)).unwrap();
        terminal
            .draw(|f| {
                render_wide_media_list(
                    f,
                    Rect::new(PX, 0, PW, 4),
                    Rect::new(PX, 0, PW, 4),
                    &mut list,
                    true,
                    selected_bg,
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();

        assert_eq!(
            buf[(PX, 0)].symbol(),
            " ",
            "accent marker is gone; panel x is quiet indent (duration={duration:?})"
        );
        // Skip the 2-column quiet indent; the title is the next
        // non-blank cell.
        let first_text = (PX + 1..PX + PW)
            .find(|&x| buf[(x, 0)].symbol().trim() != "")
            .map(|x| x - PX);
        assert_eq!(
            first_text,
            Some(2),
            "title text indent must be 2 columns (duration={duration:?})"
        );
        for x in PX..PX + PW {
            assert_eq!(
                buf[(x, 0)].bg,
                selected_bg,
                "selected-row bar must fill column {x} (duration={duration:?})"
            );
        }
        assert_ne!(
            buf[(PX, 1)].bg,
            selected_bg,
            "only the selected row is filled (duration={duration:?})"
        );
    }
}

/// Group headings read as foam labels (not bold) in every grouped list,
/// and date metadata paints green in the fixed right-aligned gutter.
#[test]
fn heading_labels_are_foam_not_bold_and_year_metadata_is_green() {
    let rect = Rect::new(0, 0, 40, 3);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Heading {
            text: "Artist".into(),
        },
        MediaListRow::Item {
            target: "album".into(),
            primary: "Album".into(),
            secondary: None,
            trailing: Some(MediaListTrailing::Gutter("2001".into())),
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Item {
            target: "resume".into(),
            primary: "Resume".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::active(Some(47)),
        },
    ]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list(f, rect, rect, &mut list, true, palette::SURFACE_RESTING);
        })
        .unwrap();
    let buf = terminal.backend().buffer();

    assert_eq!(buf[(2, 0)].symbol(), "A", "heading label at the indent");
    assert_eq!(buf[(3, 0)].symbol(), "R", "heading label paints all caps");
    assert_eq!(buf[(2, 0)].fg, palette::GROUP_HEADING_FG);
    assert!(!buf[(2, 0)].modifier.contains(Modifier::BOLD));

    assert_eq!(buf[(34, 1)].symbol(), "2");
    assert_eq!(buf[(34, 1)].fg, palette::STATUS_AVAILABLE);

    let badge_x = 2 + "Resume ".len() as u16;
    assert_eq!(buf[(badge_x, 2)].symbol(), "4");
    assert_eq!(buf[(badge_x, 2)].fg, palette::PROGRESS_PERCENT);
}

/// A framed parent may claim a full-width panel while reserving a
/// vertically offset row-flow rect. The row painter keeps its established
/// full-width selection treatment, but rows and scrollbar must start at the
/// content flow's y-coordinate and use its height.
#[test]
fn zebra_stripes_are_contained_and_selected_row_still_wins() {
    let rect = Rect::new(0, 0, 32, 4);
    let selected_bg = palette::SURFACE_RESTING;
    let zebra_bg = palette::SURFACE_FOCUSED;
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        item("one", "One", None),
        item("two", "Two", None),
        item("three", "Three", None),
        item("four", "Four", None),
    ]);
    list.select_last();
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_with_zebra(
                f,
                rect,
                rect,
                &mut list,
                true,
                selected_bg,
                Some(zebra_bg),
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    // The sequence's first row is the primary fill; the second row
    // carries the stripe. The selected row (fourth, index 3) is painted
    // full-bleed with `selected_bg` regardless of its stripe parity.
    for y in [0, 2] {
        assert_eq!(buf[(2, y)].bg, Color::Reset, "row {y} is unstriped");
    }
    let y = 1;
    assert_eq!(buf[(0, y)].bg, Color::Reset);
    assert_eq!(buf[(1, y)].bg, Color::Reset);
    assert_eq!(buf[(2, y)].bg, zebra_bg);
    assert_eq!(buf[(29, y)].bg, zebra_bg);
    assert_eq!(buf[(30, y)].bg, Color::Reset);
    assert_eq!(buf[(31, y)].bg, Color::Reset);
    assert_ne!(buf[(0, 3)].bg, zebra_bg);
    for x in 0..rect.width {
        assert_eq!(
            buf[(x, 3)].bg,
            selected_bg,
            "selected row must remain full-bleed at x={x}"
        );
    }
}

#[test]
fn two_tone_zebra_stripe_stays_inside_right_inset_without_duration() {
    let rect = Rect::new(0, 0, 32, 2);
    let mut list: WideMediaList<String> = WideMediaList::new();
    // The sequence's second row carries the stripe, so the two-tone row
    // is the one under test (the plain first row is unstriped).
    list.set_content(vec![
        item("first", "First", None),
        MediaListRow::Item {
            target: "two-tone".into(),
            primary: "Series".into(),
            secondary: Some("Episode".into()),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
    ]);
    let zebra_bg = palette::SURFACE_FOCUSED;
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_with_zebra(
                f,
                rect,
                rect,
                &mut list,
                true,
                palette::SURFACE_RESTING,
                Some(zebra_bg),
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    assert_eq!(
        buf[(2, 0)].bg,
        palette::SURFACE_RESTING,
        "the selected first row keeps the selected fill"
    );
    assert_eq!(buf[(29, 1)].bg, zebra_bg, "stripe reaches the content edge");
    assert_eq!(
        buf[(30, 1)].bg,
        Color::Reset,
        "right inset remains unstriped"
    );
    assert_eq!(
        buf[(31, 1)].bg,
        Color::Reset,
        "outer edge remains unstriped"
    );
}

/// A grouped list fills its content rows with the secondary colour under its
/// surface-coloured `Heading`/`Spacer` labels and alternates within each
/// group: every second member carries the surface fill instead, whatever the
/// group's size. Every content row still confines its fill to the shared
/// text-flow range inside the two-column gutters.
#[test]
fn library_wide_grouped_lists_alternate_from_the_secondary_fill() {
    let rect = Rect::new(0, 0, 40, 11);
    let mut list = WideMediaList::new();
    list.set_content(vec![
        heading("A"),
        item("a1", "A1", None),
        item("a2", "A2", None),
        item("a3", "A3", None),
        MediaListRow::Spacer,
        heading("B"),
        item("b1", "B1", None),
        item("b2", "B2", None),
        MediaListRow::Spacer,
        heading("C"),
        item("c1", "C1", None),
    ]);
    // The production browser stripe: the library column's fill for the
    // paint's focus bit (the Grouped Music tree's alternation tone),
    // against the `LibraryPanel` box fill the panel paints underneath.
    let pair = stripe(palette::Surface::LibraryColumn);
    let other = stripe(palette::Surface::LibraryPanel);
    assert_ne!(pair.focused, other.focused);
    assert_ne!(pair.unfocused, other.unfocused);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_component(
                f,
                rect,
                &mut list,
                WideMediaListPaintPolicy::new(true).with_zebra(pair),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    // Every group alternates from its own first member: group A's second
    // member and group B's second member drop to the surface fill, and the
    // membership count never matters -- group B's two members and group C's
    // single member alternate like any other. The headers (0, 5, 9) and the
    // separators (4, 8) keep the surface fill.
    for (y, striped) in [
        (0, false),
        (1, true),
        (2, false),
        (3, true),
        (4, false),
        (5, false),
        (6, true),
        (7, false),
        (8, false),
        (9, false),
        (10, true),
    ] {
        if y == 1 {
            // The cursor's own row paints the opaque bar across the whole
            // panel, overriding both its stripe and the gutters.
            for x in 0..rect.width {
                assert_eq!(
                    buffer[(x, y)].bg,
                    palette::SELECTED_ROW_BG,
                    "cursor row {x}"
                );
            }
            continue;
        }
        assert_eq!(
            buffer[(2, y)].bg,
            if striped { pair.focused } else { Color::Reset },
            "row {y}"
        );
        assert_eq!(buffer[(0, y)].bg, Color::Reset, "row {y} left gutter");
        assert_eq!(buffer[(1, y)].bg, Color::Reset, "row {y} left gutter");
        assert_eq!(buffer[(38, y)].bg, Color::Reset, "row {y} right inset");
        assert_eq!(buffer[(39, y)].bg, Color::Reset, "row {y} right inset");
    }
}

/// An ungrouped list keeps its alternation, counted from its own first row:
/// a stripe is a property of the row, not of the screen row it lands on, so
/// scrolling never flips the stripes under the cursor.
#[test]
fn library_wide_ungrouped_stripes_follow_the_row_not_the_screen_row() {
    let rect = Rect::new(0, 0, 40, 4);
    let mut list = WideMediaList::new();
    let rows: Vec<_> = (1..=11)
        .map(|index| item(&format!("i{index}"), &format!("I{index}"), None))
        .collect();
    list.set_content(rows);
    // The window opens on source row 7 (the last row is selected, so the
    // first seven are off-screen): the first visible row carries the stripe,
    // where the window-relative sequence would have striped the row below
    // it instead.
    list.select_last();
    let pair = stripe(palette::Surface::MainContentBox);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_component(
                f,
                rect,
                &mut list,
                WideMediaListPaintPolicy::new(true).with_zebra(pair),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    // The window really opens mid-list: the first rows are scrolled off, so
    // the stripes below prove the row's own parity rather than the
    // window's.
    let painted: String = (0..12).map(|x| buffer[(x, 0)].symbol()).collect();
    assert!(
        !painted.trim_start().starts_with("I1"),
        "row 1 is off-screen"
    );
    assert!(
        painted.trim_start().starts_with("I8"),
        "the window opens on the eighth row, not the first"
    );
    assert_eq!(buffer[(2, 0)].bg, pair.focused, "source row 7");
    assert_eq!(buffer[(2, 1)].bg, Color::Reset, "source row 8");
    assert_eq!(buffer[(2, 2)].bg, pair.focused, "source row 9");
    // Source row 10 is the selected row: the bar overrides its stripe.
    assert_eq!(buffer[(2, 3)].bg, palette::SELECTED_ROW_BG, "source row 10");
}

#[test]
fn library_wide_workspace_stripes_with_the_fixed_resting_storm() {
    let rect = Rect::new(0, 0, 40, 4);
    let mut list = WideMediaList::new();
    list.set_content(vec![
        item("one", "One", None),
        item("two", "Two", None),
        item("three", "Three", None),
        item("four", "Four", None),
    ]);
    // The unified Workspace stripe: the fixed resting-content Storm, the
    // same fill in both focus states (the role itself — the stripe no
    // longer borrows a sidebar fill).
    let pair = ZebraStripe::fixed(palette::SURFACE_RESTING);
    let other = stripe(palette::Surface::MainContentBox);
    assert_ne!(pair.focused, other.focused);
    assert_ne!(pair.unfocused, other.unfocused);
    assert_eq!(pair.focused, pair.unfocused);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_component(
                f,
                rect,
                &mut list,
                WideMediaListPaintPolicy::for_library_workspace(true).with_zebra(pair),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    // The selected first row paints the focused Workspace's Iris bar;
    // the sequence then opens on the primary fill, so rows 1 and 3
    // carry the stripe.
    assert_eq!(buffer[(2, 0)].bg, palette::ACCENT_ACTIVE);
    assert_eq!(buffer[(2, 1)].bg, pair.focused);
    assert_eq!(buffer[(2, 2)].bg, Color::Reset);
    assert_eq!(buffer[(2, 3)].bg, pair.focused);
}

#[test]
fn library_wide_selected_row_paints_the_bar_over_its_stripe() {
    let rect = Rect::new(0, 0, 40, 4);
    let pair = stripe(palette::Surface::MainContentBox);
    let mut selected = WideMediaList::new();
    selected.set_content(vec![item("one", "One", None), item("two", "Two", None)]);
    // The sequence's second row carries the stripe, and the bar replaces it.
    selected.select_last();
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_component(
                f,
                rect,
                &mut selected,
                WideMediaListPaintPolicy::new(true).with_zebra(pair),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let cell = &buffer[(2, 1)];
    assert_eq!(cell.fg, palette::SELECTED_ROW_FG);
    assert!(!cell.modifier.contains(Modifier::BOLD));
    for x in 0..rect.width {
        assert_eq!(buffer[(x, 1)].bg, palette::SELECTED_ROW_BG, "bar at x={x}");
    }
    assert_eq!(buffer[(2, 0)].bg, Color::Reset);

    // An unfocused list paints no bar at all: the rows keep their own
    // ordinary and zebra fills.
    let mut unfocused = WideMediaList::new();
    unfocused.set_content(vec![item("one", "One", None), item("two", "Two", None)]);
    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_component(
                f,
                rect,
                &mut unfocused,
                WideMediaListPaintPolicy::new(false).with_zebra(pair),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(2, 1)].bg, pair.unfocused);
    assert_eq!(buffer[(2, 0)].bg, Color::Reset);
}

#[test]
fn non_adjacent_multi_selected_rows_paint_the_bar() {
    let rect = Rect::new(0, 0, 32, 4);
    let pair = stripe(palette::Surface::MainContentBox);
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        item("one", "One", None),
        item("two", "Two", None),
        item("three", "Three", None),
        item("four", "Four", None),
    ]);
    list.toggle_selection(&"two".to_string());
    list.toggle_selection(&"four".to_string());

    let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
    terminal
        .draw(|f| {
            render_wide_media_list_component(
                f,
                rect,
                &mut list,
                WideMediaListPaintPolicy::new(false).with_zebra(pair),
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    // The list is unfocused, so only the selected rows paint: the cursor's
    // own first row plus the two toggled rows each fill the whole row with
    // the canonical bar and selected-row foreground.
    for y in [0, 1, 3] {
        assert_eq!(buf[(2, y)].fg, palette::SELECTED_ROW_FG);
        assert!(!buf[(2, y)].modifier.contains(Modifier::BOLD));
        for x in 0..rect.width {
            assert_eq!(buf[(x, y)].bg, palette::SELECTED_ROW_BG, "row {y} x={x}");
        }
    }
    assert_eq!(buf[(2, 2)].bg, Color::Reset);
}

#[test]
fn distinct_claim_and_content_geometry_keeps_rows_on_the_retained_flow() {
    let claim = Rect::new(6, 0, 30, 6);
    let content = Rect::new(8, 2, 26, 2);
    let selected_bg = palette::SURFACE_RESTING;
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        item("selected", "Selected", None),
        item("other", "Other", None),
    ]);

    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    let mut selected_row_rect = None;
    terminal
        .draw(|f| {
            selected_row_rect = Some(
                render_wide_media_list(f, claim, content, &mut list, true, selected_bg)
                    .selected_row_rect,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();

    assert_eq!(
        selected_row_rect,
        Some(Some(Rect::new(content.x, content.y, content.width, 1)))
    );
    assert_eq!(buffer[(claim.x, content.y)].symbol(), " ");
    assert_eq!(buffer[(content.x, content.y)].symbol(), "S");
    assert_eq!(buffer[(claim.x, content.y)].bg, selected_bg);
    assert_eq!(buffer[(claim.right() - 1, content.y)].bg, selected_bg);
    assert_eq!(buffer[(claim.x, claim.y)].symbol(), " ");
    assert_ne!(buffer[(claim.x, content.y + 1)].bg, selected_bg);
}

/// Step 4 latent bug: the painter must persist the resolved scroll offset
/// back into `list` so it survives across frames. Home discarded the old
/// `usize` return, so its rail always re-scrolled to the top.
#[test]
fn painter_persists_resolved_scroll_offset_across_frames() {
    let rect = Rect::new(0, 0, 40, 4);
    let selected_bg = palette::SURFACE_RESTING;
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(
        (0..12)
            .map(|i| item(&format!("t{i}"), &format!("Entry {i}"), None))
            .collect(),
    );
    list.select_last();

    let first = paint(&mut list, rect, selected_bg);
    let resolved = first.row_geometry.offset();
    assert!(resolved > 0, "a bottom selection must scroll the viewport");
    assert_eq!(
        list.scroll(),
        resolved,
        "painter stores the offset it resolved"
    );

    // Re-render with no further input: the stored offset is reused, not reset.
    let second = paint(&mut list, rect, selected_bg);
    assert_eq!(second.row_geometry.offset(), resolved);
    assert_eq!(list.scroll(), resolved);
}
