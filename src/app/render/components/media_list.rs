mod row;
mod wide;

pub(in crate::app) use wide::render_wide_media_list_component;

#[cfg(test)]
mod wide_row_regression_tests {
    use super::wide::{
        render_wide_media_list, render_wide_media_list_component, render_wide_media_list_with_zebra,
    };
    use super::wide_row_regression_tests_helpers::{heading, item, paint, row_of};
    use crate::app::components::media_list::{
        MediaKind, MediaListRow, MediaListTitleReveal, MediaListTrailing, MediaSemanticState,
        WideMediaList, WideMediaListPaintPolicy, ZebraStripe,
    };
    use crate::app::palette;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;
    use std::time::{Duration, Instant};

    /// The zebra pair a surface resolves to for its focused and unfocused fills.
    fn stripe(surface: palette::Surface) -> ZebraStripe {
        ZebraStripe {
            focused: palette::surface_colors(surface, true).fill,
            unfocused: palette::surface_colors(surface, false).fill,
        }
    }

    fn title_row_at(list: &mut WideMediaList<String>, focused: bool, y: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 4)).unwrap();
        terminal
            .draw(|f| {
                render_wide_media_list(
                    f,
                    Rect::new(0, 0, 40, 2),
                    Rect::new(0, 0, 40, 2),
                    list,
                    focused,
                    palette::SURFACE_RESTING,
                );
            })
            .unwrap();
        (2..38)
            .map(|x| {
                terminal.backend().buffer()[(x, y)]
                    .symbol()
                    .chars()
                    .next()
                    .unwrap_or(' ')
            })
            .collect()
    }

    fn title_row(list: &mut WideMediaList<String>, focused: bool) -> String {
        title_row_at(list, focused, 0)
    }

    /// A split row: the context text plus the item's own title, one space
    /// apart (the shape the podcast episode browser projects).
    fn split_item(target: &str, primary: &str, secondary: &str) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: primary.into(),
            secondary: Some(secondary.into()),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    #[test]
    fn selected_focused_title_marquees_and_restarts_on_content_change() {
        let mut list = WideMediaList::new();
        list.set_content(vec![item(
            "one",
            "A very long selected title that overflows",
            None,
        )]);
        let at_rest = title_row(&mut list, true);
        list.set_marquee_started_at(
            "A very long selected title that overflows",
            Instant::now() - Duration::from_millis(1_401),
        );
        let advanced = title_row(&mut list, true);
        assert_ne!(at_rest, advanced);
        assert!(!at_rest.contains('…'));

        list.set_content(vec![item(
            "two",
            "A newly changed overflowing title with more text",
            None,
        )]);
        let restarted = title_row(&mut list, true);
        assert!(restarted.trim_start().starts_with("A newly changed"));
    }

    /// A two-tone (secondary-title) row must key its marquee state on the same
    /// text the painter marquees — the joined context-and-title string. The
    /// presenter and the painter share one formula (`row_marquee_key`); when
    /// the key drifted, the start time reset every frame and the marquee held
    /// at the start forever.
    #[test]
    fn two_tone_selected_row_marquees_on_the_joined_title_key() {
        let mut list = WideMediaList::new();
        list.set_content(vec![split_item(
            "ep",
            "A very long series name that overflows",
            "Episode Title",
        )]);
        let at_rest = title_row(&mut list, true);
        list.set_marquee_started_at(
            "A very long series name that overflows Episode Title",
            Instant::now() - Duration::from_millis(1_401),
        );
        let advanced = title_row(&mut list, true);
        assert_ne!(
            at_rest, advanced,
            "two-tone row must advance its marquee, not restart it"
        );
    }

    /// The reveal-on-selection policy: the item's own title is the reward for
    /// looking at a row, so every other row paints its context text alone.
    #[test]
    fn reveal_on_selection_hides_the_item_title_until_the_row_is_selected() {
        let mut list = WideMediaList::new();
        list.set_title_reveal(MediaListTitleReveal::OnSelection);
        list.set_content(vec![
            split_item("one", "Show A", "Episode One"),
            split_item("two", "Show A", "Episode Two"),
        ]);
        let selected = title_row_at(&mut list, true, 0);
        let other = title_row_at(&mut list, true, 1);
        assert_eq!(
            selected.trim(),
            "Show A Episode One",
            "the selected row reveals the item title"
        );
        assert_eq!(
            other.trim(),
            "Show A",
            "an unselected row paints its context text alone"
        );
        assert!(
            !other.contains('…'),
            "no ellipsis stands in for the hidden title: {other:?}"
        );

        // Narrow title slot: a resting row truncates its context text, and the
        // selected row marquees the title instead of ellipsis-truncating it.
        let mut narrow = WideMediaList::new();
        narrow.set_title_reveal(MediaListTitleReveal::OnSelection);
        narrow.set_content(vec![
            split_item("one", "A podcast with a long name", "Episode One"),
            split_item("two", "A podcast with a long name", "Episode Two"),
        ]);
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        terminal
            .draw(|f| {
                render_wide_media_list(
                    f,
                    Rect::new(0, 0, 20, 2),
                    Rect::new(0, 0, 20, 2),
                    &mut narrow,
                    true,
                    palette::SURFACE_RESTING,
                );
            })
            .unwrap();
        let row_text = |y: u16| {
            (2..18)
                .map(|x| {
                    terminal.backend().buffer()[(x, y)]
                        .symbol()
                        .chars()
                        .next()
                        .unwrap_or(' ')
                })
                .collect::<String>()
        };
        assert!(
            row_text(1).contains('…'),
            "a resting row truncates its context text: {:?}",
            row_text(1)
        );
        assert!(
            !row_text(0).contains('…'),
            "the selected row marquees rather than truncating: {:?}",
            row_text(0)
        );
    }

    /// The selected row of a reveal-on-selection list marquees even when its
    /// title fits; a list that reveals titles on every row keeps a fitting
    /// title static (the accepted `Fitting title never marquees` rule).
    #[test]
    fn reveal_on_selection_marquees_a_fitting_selected_title() {
        let key = "Show A Episode One";
        let mut revealed = WideMediaList::new();
        revealed.set_title_reveal(MediaListTitleReveal::OnSelection);
        revealed.set_content(vec![split_item("one", "Show A", "Episode One")]);
        let at_rest = title_row(&mut revealed, true);
        revealed.set_marquee_started_at(key, Instant::now() - Duration::from_millis(1_401));
        let advanced = title_row(&mut revealed, true);
        assert_ne!(
            at_rest, advanced,
            "a fitting title still scrolls in a reveal-on-selection list"
        );

        let mut always = WideMediaList::new();
        always.set_content(vec![split_item("one", "Show A", "Episode One")]);
        let resting = title_row(&mut always, true);
        always.set_marquee_started_at(key, Instant::now() - Duration::from_millis(1_401));
        assert_eq!(
            resting,
            title_row(&mut always, true),
            "a fitting title stays static in a list that reveals titles on every row"
        );
    }

    /// The clock keys on the full marqueed text, so two rows sharing a context
    /// text (two episodes of one podcast) do not share a clock position.
    #[test]
    fn marquee_clock_restarts_between_rows_sharing_a_context_text() {
        let mut list = WideMediaList::new();
        list.set_title_reveal(MediaListTitleReveal::OnSelection);
        list.set_content(vec![
            split_item("one", "Show A", "Episode One"),
            split_item("two", "Show A", "Episode Two"),
        ]);
        list.set_marquee_started_at(
            "Show A Episode One",
            Instant::now() - Duration::from_millis(1_401),
        );
        let _ = title_row_at(&mut list, true, 0);
        list.move_selection(1);
        let second = title_row_at(&mut list, true, 1);
        assert!(
            second.trim().starts_with("Show A Episode Two"),
            "the newly selected row starts at its held beginning: {second:?}"
        );
    }

    #[test]
    fn non_marquee_rows_still_ellipsis_truncate() {
        let mut list = WideMediaList::new();
        list.set_content(vec![item(
            "one",
            "A very long selected title that overflows",
            None,
        )]);
        assert!(title_row(&mut list, false).contains('…'));

        let mut list = WideMediaList::new();
        list.set_content(vec![
            item(
                "selected",
                "A very long selected title that overflows",
                None,
            ),
            item(
                "other",
                "A very long non-selected title that overflows",
                None,
            ),
        ]);
        let selected_at_rest = title_row_at(&mut list, true, 0);
        list.set_marquee_started_at(
            "A very long selected title that overflows",
            Instant::now() - Duration::from_millis(1_401),
        );
        let selected_advanced = title_row_at(&mut list, true, 0);
        let other = title_row_at(&mut list, true, 1);
        assert!(
            !selected_at_rest.contains('…'),
            "selected row should marquee at rest: {selected_at_rest:?}"
        );
        assert_ne!(
            selected_at_rest, selected_advanced,
            "selected row should advance while another overflowing row is painted"
        );
        assert!(
            other.contains('…'),
            "non-selected row should use ellipsis truncation: {other:?}"
        );
    }

    /// migrate-home-feeds 4.6: the selected row's highlight bar must span the
    /// whole panel width (never just the row text, with or without a duration
    /// string), the edge marker must sit flush at the panel's `x`, and the
    /// title must land at column 2. These broke together when the painter was
    /// handed an already-inset content rect.
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

    /// Group headings read as foam labels in every grouped list, and date
    /// metadata paints green in the fixed right-aligned gutter.
    #[test]
    fn heading_labels_are_foam_and_year_metadata_is_green() {
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
        assert_eq!(buf[(2, 0)].fg, palette::TEXT_METADATA);
        assert!(buf[(2, 0)].modifier.contains(Modifier::BOLD));

        assert_eq!(buf[(34, 1)].symbol(), "2");
        assert_eq!(buf[(34, 1)].fg, palette::STATUS_AVAILABLE);

        let badge_x = 2 + "Resume ".len() as u16;
        assert_eq!(buf[(badge_x, 2)].symbol(), "4");
        assert_eq!(buf[(badge_x, 2)].fg, palette::TEXT_METADATA);
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
        // same fill in both focus states (SidebarBody's fixed row).
        let pair = stripe(palette::Surface::SidebarBody);
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
        let published_trailing =
            Some(MediaListTrailing::Gutter(year_item.production_year.to_string()));
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

    #[test]
    fn selected_now_playing_marker_uses_ink_but_progress_keeps_metadata_role() {
        use crate::app::components::media_list::ActiveProgress;

        let rect = Rect::new(0, 0, 40, 1);
        let mut list: WideMediaList<String> = WideMediaList::new();
        list.set_content(vec![MediaListRow::Item {
            target: "playing".into(),
            primary: "Playing title".into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::NowPlaying {
                progress: Some(ActiveProgress::new(47)),
            },
        }]);

        let mut terminal = Terminal::new(TestBackend::new(rect.width, rect.height)).unwrap();
        terminal
            .draw(|f| {
                render_wide_media_list(f, rect, rect, &mut list, true, palette::SELECTED_ROW_BG);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let marker_x = (0..rect.width)
            .find(|&x| buf[(x, 0)].symbol() == "▶")
            .expect("selected now-playing marker");
        assert_eq!(buf[(marker_x, 0)].fg, palette::SELECTED_ROW_FG);
        let progress_x = (0..rect.width)
            .find(|&x| buf[(x, 0)].symbol() == "4")
            .expect("selected progress percentage");
        assert_eq!(buf[(progress_x, 0)].fg, palette::TEXT_METADATA);
        assert_eq!(buf[(progress_x + 1, 0)].fg, palette::TEXT_METADATA);
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
        use crate::app::components::media_list::{
            ActiveProgress, MediaListRow, MediaSemanticState,
        };

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
        assert_eq!(buf[(progress_x, 0)].fg, palette::TEXT_METADATA);
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
        assert_eq!(buf[(progress_x, 1)].fg, palette::TEXT_METADATA);
        assert_eq!(buf[(gutter_x + 2, 1)].symbol(), "2");
        assert_eq!(buf[(gutter_x + 2, 1)].fg, palette::STATUS_AVAILABLE);
        let duration_x = rect.width - 2 - 4;
        assert_eq!(buf[(duration_x, 1)].symbol(), "2");
        assert_eq!(buf[(duration_x, 1)].fg, palette::DURATION);
    }

    #[test]
    fn now_playing_duration_and_narrow_reserves_are_safe() {
        use crate::app::components::media_list::{
            ActiveProgress, MediaListRow, MediaSemanticState,
        };

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
}

#[cfg(test)]
mod wide_row_regression_tests_helpers {
    use super::wide::MediaListPaint;
    use crate::app::components::media_list::{
        MediaKind, MediaListRow, MediaSemanticState, WideMediaList,
    };
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::Terminal;

    pub(super) fn item(
        target: &str,
        primary: &str,
        duration: Option<String>,
    ) -> MediaListRow<String> {
        row_of(target, primary, duration, MediaKind::Media)
    }

    pub(super) fn heading(text: &str) -> MediaListRow<String> {
        MediaListRow::Heading { text: text.into() }
    }

    pub(super) fn row_of(
        target: &str,
        primary: &str,
        duration: Option<String>,
        kind: MediaKind,
    ) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: primary.into(),
            secondary: None,
            trailing: None,
            duration,
            kind,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    pub(super) fn paint(
        list: &mut WideMediaList<String>,
        rect: Rect,
        selected_bg: Color,
    ) -> MediaListPaint<String> {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let mut captured = None;
        terminal
            .draw(|f| {
                captured = Some(render_wide_media_list(
                    f,
                    rect,
                    rect,
                    list,
                    true,
                    selected_bg,
                ));
            })
            .unwrap();
        captured.unwrap()
    }

    use super::wide::render_wide_media_list;
}
