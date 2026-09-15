mod plain_rows;
mod row;
mod wide;

pub(in crate::app) use plain_rows::render_plain_rows;
pub(in crate::app) use wide::{
    render_inline_media_browser_component, render_wide_media_list_component,
};

#[cfg(test)]
mod wide_row_regression_tests {
    use super::wide::{
        render_wide_media_list, render_wide_media_list_component, render_wide_media_list_with_zebra,
    };
    use super::wide_row_regression_tests_helpers::{item, paint, row_of};
    use crate::app::components::media_list::{
        MediaKind, MediaListRow, MediaSemanticState, WideMediaList, WideMediaListPaintPolicy,
        ZebraStripe,
    };
    use crate::app::palette;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;
    use std::time::{Duration, Instant};

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

    /// Regression: a two-tone (secondary-title) row must key its marquee
    /// state on the same `primary` text the painter looks the state up with.
    /// When the key drifted to the joined title parts, the start time reset
    /// every frame and the Home marquee held at the start forever.
    #[test]
    fn two_tone_selected_row_marquees_on_the_primary_key() {
        use crate::app::components::media_list::{MediaListRow, MediaSemanticState};

        let mut list = WideMediaList::new();
        list.set_content(vec![MediaListRow::Item {
            target: "ep".into(),
            primary: "A very long series name that overflows".into(),
            secondary: Some("Episode Title".into()),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        }]);
        let at_rest = title_row(&mut list, true);
        list.set_marquee_started_at(
            "A very long series name that overflows",
            Instant::now() - Duration::from_millis(1_401),
        );
        let advanced = title_row(&mut list, true);
        assert_ne!(
            at_rest, advanced,
            "two-tone row must advance its marquee, not restart it"
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

    /// A framed parent may claim a full-width panel while reserving a
    /// vertically offset row-flow rect. The row painter keeps its established
    /// full-width selection treatment, but rows and scrollbar must start at the
    /// content flow's y-coordinate and use its height.
    #[test]
    fn zebra_stripes_are_contained_and_selected_row_still_wins() {
        let rect = Rect::new(0, 0, 32, 4);
        let selected_bg = palette::SURFACE_RESTING;
        let zebra_bg = Color::Rgb(60, 72, 65);
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
                    false,
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        for y in [0, 2] {
            assert_eq!(buf[(0, y)].bg, Color::Reset);
            assert_eq!(buf[(1, y)].bg, Color::Reset);
            assert_eq!(buf[(2, y)].bg, zebra_bg);
            assert_eq!(buf[(29, y)].bg, zebra_bg);
            assert_eq!(buf[(30, y)].bg, Color::Reset);
            assert_eq!(buf[(31, y)].bg, Color::Reset);
        }
        assert_ne!(buf[(0, 1)].bg, zebra_bg);
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
        list.set_content(vec![
            MediaListRow::Item {
                target: "two-tone".into(),
                primary: "Series".into(),
                secondary: Some("Episode".into()),
                trailing: None,
                duration: None,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
            },
            item("selected", "Selected", None),
        ]);
        list.select_last();
        let zebra_bg = Color::Rgb(60, 72, 65);
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
                    false,
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        assert_eq!(buf[(29, 0)].bg, zebra_bg, "stripe reaches the content edge");
        assert_eq!(
            buf[(30, 0)].bg,
            Color::Reset,
            "right inset remains unstriped"
        );
        assert_eq!(
            buf[(31, 0)].bg,
            Color::Reset,
            "outer edge remains unstriped"
        );
    }

    #[test]
    fn gutter_policy_selected_title_is_bold_focus_accent() {
        let rect = Rect::new(0, 0, 32, 2);
        let mut list: WideMediaList<String> = WideMediaList::new();
        list.set_content(vec![
            item("other", "Other", None),
            item("selected", "Selected", Some("1:05".into())),
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
                    palette::SURFACE_RESTING,
                    Some(Color::Rgb(60, 72, 65)),
                    true,
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        // The selected title paints in the focus accent, bold; no icon and
        // no selected background; the duration keeps the default colour.
        assert_eq!(buf[(0, 1)].symbol(), " ");
        assert_eq!(buf[(2, 1)].fg, palette::TEXT_FOCUS_ACCENT);
        assert!(buf[(2, 1)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(26, 1)].fg, palette::STATUS_AVAILABLE);
        assert_ne!(buf[(10, 1)].bg, palette::SURFACE_RESTING);
        // The unselected even item keeps its zebra stripe, and the selected
        // odd item is unstriped.
        assert_eq!(buf[(2, 0)].bg, Color::Rgb(60, 72, 65));
        assert_eq!(buf[(2, 1)].bg, Color::Reset);
    }

    #[test]
    fn library_wide_browser_stripes_items_not_structural_rows() {
        let rect = Rect::new(0, 0, 40, 6);
        let mut list = WideMediaList::new();
        list.set_content(vec![
            item("one", "One", None),
            MediaListRow::Heading {
                text: "Group".into(),
            },
            item("two", "Two", None),
            MediaListRow::Spacer,
            item("three", "Three", None),
            item("four", "Four", None),
        ]);
        let pair = ZebraStripe {
            focused: palette::surface_colors(palette::Surface::MainContentBox, true).fill,
            unfocused: palette::surface_colors(palette::Surface::MainContentBox, false).fill,
        };
        assert_ne!(
            pair.focused,
            palette::surface_colors(palette::Surface::LibraryPanel, true).fill
        );
        assert_ne!(
            pair.unfocused,
            palette::surface_colors(palette::Surface::LibraryPanel, false).fill
        );
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
        for (y, striped) in [
            (0, true),
            (1, false),
            (2, false),
            (3, false),
            (4, true),
            (5, false),
        ] {
            assert_eq!(
                buffer[(2, y)].bg,
                if striped { pair.focused } else { Color::Reset },
                "row {y}"
            );
        }
    }

    #[test]
    fn library_wide_workspace_stripes_with_library_panel_pair() {
        let rect = Rect::new(0, 0, 40, 4);
        let mut list = WideMediaList::new();
        list.set_content(vec![
            item("one", "One", None),
            item("two", "Two", None),
            item("three", "Three", None),
            item("four", "Four", None),
        ]);
        let pair = ZebraStripe {
            focused: palette::surface_colors(palette::Surface::LibraryPanel, true).fill,
            unfocused: palette::surface_colors(palette::Surface::LibraryPanel, false).fill,
        };
        assert_ne!(
            pair.focused,
            palette::surface_colors(palette::Surface::MainContentBox, true).fill
        );
        assert_ne!(
            pair.unfocused,
            palette::surface_colors(palette::Surface::MainContentBox, false).fill
        );
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
        assert_eq!(buffer[(2, 0)].bg, pair.focused);
        assert_eq!(buffer[(2, 1)].bg, Color::Reset);
        assert_eq!(buffer[(2, 2)].bg, pair.focused);
        assert_eq!(buffer[(2, 3)].bg, Color::Reset);
    }

    #[test]
    fn library_wide_accent_keeps_selected_stripe_and_unfocused_rows_plain() {
        let rect = Rect::new(0, 0, 40, 4);
        let pair = ZebraStripe {
            focused: palette::surface_colors(palette::Surface::MainContentBox, true).fill,
            unfocused: palette::surface_colors(palette::Surface::MainContentBox, false).fill,
        };
        let mut selected = WideMediaList::new();
        selected.set_content(vec![item("one", "One", None), item("two", "Two", None)]);
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
        let cell = &terminal.backend().buffer()[(2, 0)];
        assert_eq!(cell.fg, palette::TEXT_FOCUS_ACCENT);
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert_eq!(cell.bg, pair.focused);
        assert_eq!(terminal.backend().buffer()[(0, 0)].bg, Color::Reset);

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
        for y in 0..2 {
            let cell = &terminal.backend().buffer()[(2, y)];
            assert_ne!(cell.fg, palette::TEXT_FOCUS_ACCENT);
        }
        assert_eq!(terminal.backend().buffer()[(2, 0)].bg, pair.unfocused);
    }

    #[test]
    fn non_adjacent_multi_selected_rows_and_unfocused_cursor_paint_selected_surface() {
        let rect = Rect::new(0, 0, 32, 4);
        let pair = ZebraStripe {
            focused: palette::surface_colors(palette::Surface::MainContentBox, true).fill,
            unfocused: palette::surface_colors(palette::Surface::MainContentBox, false).fill,
        };
        let mut list: WideMediaList<String> = WideMediaList::new();
        list.set_content(vec![
            item("one", "One", None),
            item("two", "Two", None),
            item("three", "Three", None),
            item("four", "Four", None),
        ]);
        list.toggle_selection(&"one".to_string());
        list.toggle_selection(&"three".to_string());

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
        for y in [0, 2] {
            assert_eq!(buf[(2, y)].fg, palette::TEXT_FOCUS_ACCENT);
            assert!(buf[(2, y)].modifier.contains(Modifier::BOLD));
            assert_eq!(buf[(2, y)].bg, pair.unfocused);
            assert_eq!(buf[(0, y)].bg, Color::Reset);
        }
        for y in [1, 3] {
            assert_ne!(buf[(2, y)].fg, palette::TEXT_FOCUS_ACCENT);
            assert_eq!(buf[(2, y)].bg, Color::Reset);
        }
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
    /// `Media` row's duration right-aligned in `STATUS_AVAILABLE` green.
    #[test]
    fn collection_row_suppresses_projected_duration_media_row_paints_it() {
        let rect = Rect::new(0, 0, 40, 4);
        let selected_bg = palette::SURFACE_RESTING;
        let dur = crate::app::ui_util::list_duration_secs(272); // 4:32
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
            palette::STATUS_AVAILABLE,
            "Media duration is painted green"
        );
    }

    /// Episode rows split into a two-tone title: the series title paints in
    /// the ordinary emphasis role and the episode title after it in the
    /// yellow focus-accent role, so the two are visually delineated. On a
    /// slot too narrow for both, the episode title keeps its budget and the
    /// series name ellipsises first.
    #[test]
    fn episode_row_paints_secondary_title_in_the_focus_accent_role() {
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
        assert_eq!(buf[(2, 0)].fg, palette::TEXT_EMPHASIS);
        assert_eq!(
            buf[(2 + "Severance ".len() as u16, 0)].fg,
            palette::TEXT_FOCUS_ACCENT
        );

        // Narrow slot on an unselected row (the selected row marquees):
        // the episode title survives, the series name ellipsises first.
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
            row_text.contains("Episode Title"),
            "episode title survives a narrow slot: {row_text:?}"
        );
        assert!(
            row_text.contains('\u{2026}'),
            "long series name ellipsises first: {row_text:?}"
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
                trailing: Some("FOAM".into()),
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
                trailing: None,
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
            playing.contains("FOAM 47%"),
            "live progress rides the trailing text like other rows: {playing:?}"
        );
        assert!(
            playing.contains("2:00"),
            "total time shows like other rows: {playing:?}"
        );
        let duration_x = rect.width - 2 - 4;
        assert_eq!(buf[(duration_x, 0)].symbol(), "2");
        assert_eq!(buf[(duration_x, 0)].fg, palette::STATUS_AVAILABLE);

        let resume = row_text(1);
        assert!(resume.contains("Resume title 12%"));
        assert!(resume.contains("2:00"));
        let duration_x = rect.width - 2 - 4;
        assert_eq!(buf[(duration_x, 1)].symbol(), "2");
        assert_eq!(buf[(duration_x, 1)].fg, palette::STATUS_AVAILABLE);
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
