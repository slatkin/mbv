#[cfg(test)]
mod wide_row_title_tests {
    use super::wide::render_wide_media_list;
    use super::wide_row_regression_tests_helpers::item;
    use crate::app::components::media_list::{
        MediaKind, MediaListRow, MediaListTitleReveal, MediaSemanticState, WideMediaList,
    };
    use crate::app::palette;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
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
}
