use super::*;
use crate::app::components::library_panel::content::{
    ArtworkShape, HeroArtwork, HeroImageState, PanelList, Workspace,
};
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// A stub `PanelList` so a Workspace can be present for the overview box.
struct NoopList;
impl PanelList for NoopList {
    fn clamp_viewport(&mut self, _viewport_height: usize) {}

    fn set_paint_policy(
        &mut self,
        _policy: crate::app::components::library_panel::content::PanelListPaintPolicy,
    ) {
    }

    fn view(&mut self, _f: &mut Frame, _rect: Rect) {}
}

fn facts(shape: ArtworkShape) -> HeroFacts {
    HeroFacts {
        title: "Dune".into(),
        meta_rows: vec!["2021".into()],
        duration_row: None,
        progress_row: None,
        links: Vec::new(),
        artwork: HeroArtwork {
            shape,
            source: None,
            decoration: None,
            image: HeroImageState::None,
        },
    }
}

#[test]
fn overview_credits_have_blank_row_separator_and_zebra_stripes() {
    let content = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("Overview".into()),
        credits: Some(vec![
            HeroCredit {
                name: "One".into(),
                role: "Actor".into(),
            },
            HeroCredit {
                name: "Two".into(),
                role: "Writer".into(),
            },
            HeroCredit {
                name: "Three".into(),
                role: "Composer".into(),
            },
        ]),
        workspace: None,
    };
    let mut terminal = Terminal::new(TestBackend::new(32, 12)).unwrap();
    terminal
        .draw(|f| {
            paint_overview_box(
                f,
                Rect::new(0, 0, 32, 12),
                0,
                &content,
                0,
                palette::Surface::HeroPane,
                12,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let separator = (0..12).find(|y| buffer[(2, *y)].symbol() == "▁").unwrap();
    assert_eq!(buffer[(2, separator)].fg, palette::HERO_OVERVIEW_SEPARATOR);
    assert_eq!(buffer[(2, separator + 1)].symbol(), " ");
    let stripe = palette::HERO_CREDITS_STRIPE;
    assert_ne!(buffer[(2, separator + 2)].bg, stripe);
    assert_eq!(buffer[(2, separator + 3)].bg, stripe);
    assert_ne!(buffer[(2, separator + 4)].bg, stripe);
    assert_eq!(buffer[(2, separator + 2)].fg, palette::HERO_CREDITS_NAME);
    assert_eq!(buffer[(2, separator + 3)].fg, palette::HERO_CREDITS_NAME);
}

#[test]
fn long_overview_scrolls_inside_a_short_box() {
    let content = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("one two three four five six seven eight nine ten eleven twelve".into()),
        credits: Some(vec![HeroCredit {
            name: "Person".into(),
            role: "Actor".into(),
        }]),
        workspace: Some(Workspace {
            header: None,
            selector: None,
            list: &mut NoopList,
            focused: false,
        }),
    };
    let mut terminal = Terminal::new(TestBackend::new(32, 6)).unwrap();
    let mut metrics = None;
    terminal
        .draw(|f| {
            metrics = paint_overview_box(
                f,
                Rect::new(0, 0, 32, 6),
                0,
                &content,
                0,
                palette::Surface::HeroPane,
                6,
            )
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let inner = Rect::new(2, 2, 28, 3);
    let metrics = metrics.unwrap();
    // The short box shows its own inner height and scrolls the rest of the
    // flow (overview text, separator, table) rather than pinning anything.
    assert_eq!(metrics.viewport, 3);
    assert!(metrics.content_length > metrics.viewport);
    assert!(
        (inner.bottom()..6).all(|y| {
            (inner.left()..inner.right()).all(|x| buffer[(x, y)].symbol() != "▁")
                && (inner.left()..inner.right()).all(|x| buffer[(x, y)].symbol() != "Person")
        }),
        "separator and table never paint below inner bottom"
    );

    // At the end of the range the overview has scrolled out of the box and
    // the flow's later rows hold the inner area instead.
    terminal
        .draw(|f| {
            paint_overview_box(
                f,
                Rect::new(0, 0, 32, 6),
                0,
                &content,
                metrics.content_length - metrics.viewport,
                palette::Surface::HeroPane,
                6,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let inner_text = (inner.top()..inner.bottom())
        .map(|y| {
            (inner.left()..inner.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<String>();
    assert!(
        !inner_text.contains("one two three"),
        "the overview text scrolled out: {inner_text:?}"
    );
}

#[test]
fn overview_separator_and_credits_scroll_as_one_flow() {
    let credits = (0..60)
        .map(|i| HeroCredit {
            name: format!("Person {i}"),
            role: format!("Role {i}"),
        })
        .collect::<Vec<_>>();
    let content = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("Overview text".into()),
        credits: Some(credits),
        workspace: Some(Workspace {
            header: None,
            selector: None,
            list: &mut NoopList,
            focused: false,
        }),
    };
    // A tall pane (above the short-pane threshold) whose natural content
    // exceeds its room: the short-pane 5-row overview cap must not
    // interfere, and the Workspace sizing must force an overflow.
    let draw = |offset| {
        let mut terminal = Terminal::new(TestBackend::new(32, 60)).unwrap();
        let mut paint = None;
        terminal
            .draw(|f| {
                paint = paint_overview_box(
                    f,
                    Rect::new(0, 0, 32, 60),
                    0,
                    &content,
                    offset,
                    palette::Surface::HeroPane,
                    60,
                )
            })
            .unwrap();
        (terminal.backend().buffer().clone(), paint.unwrap())
    };
    let (at_start, metrics) = draw(0);
    let max_offset = metrics.content_length.saturating_sub(metrics.viewport);
    let (scrolled_by_one, _) = draw(1);
    let (at_end, _) = draw(max_offset);
    let row = |buffer: &ratatui::buffer::Buffer, y| {
        (0..32).map(|x| buffer[(x, y)].symbol()).collect::<String>()
    };
    let separator_row =
        |buffer: &ratatui::buffer::Buffer| (2..60).find(|y| buffer[(2, *y)].symbol() == "▁");

    // The flow is the overview line, the separator, the blank row and every
    // credit row — the scroll range covers all of it.
    assert_eq!(
        metrics.content_length,
        1 + OVERVIEW_CREDITS_GAP_ROWS + 60,
        "the overview is part of the scrollable content"
    );
    assert_eq!(metrics.viewport, 57);

    // At the top the overview paints above its separator; one row of scroll
    // moves both up, so neither is pinned.
    assert_eq!(separator_row(&at_start), Some(3));
    assert!(row(&at_start, 2).contains("Overview text"));
    assert_eq!(separator_row(&scrolled_by_one), Some(2));
    assert!(!row(&scrolled_by_one, 3).contains("Overview text"));

    // At the end of the range the overview and separator have scrolled out
    // and the table fills the box.
    assert_eq!(separator_row(&at_end), None);
    assert!(row(&at_end, 2).contains("Person 3"));
    assert!(row(&at_end, 58).contains("Person 59"));
}

#[test]
fn credits_scrollbar_thumb_tracks_table_offset() {
    let credits = (0..12)
        .map(|i| HeroCredit {
            name: format!("Person {i}"),
            role: "Actor".into(),
        })
        .collect::<Vec<_>>();
    let content = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("Overview".into()),
        credits: Some(credits),
        workspace: Some(Workspace {
            header: None,
            selector: None,
            list: &mut NoopList,
            focused: false,
        }),
    };
    let draw = |offset| {
        let mut terminal = Terminal::new(TestBackend::new(32, 14)).unwrap();
        terminal
            .draw(|f| {
                paint_overview_box(
                    f,
                    Rect::new(0, 0, 32, 14),
                    0,
                    &content,
                    offset,
                    palette::Surface::HeroPane,
                    14,
                );
            })
            .unwrap();
        (2..14)
            .filter(|y| terminal.backend().buffer()[(31, *y)].symbol() != " ")
            .collect::<Vec<_>>()
    };
    let at_start = draw(0);
    let at_end = draw(12);
    assert_ne!(at_start, at_end);
    assert!(at_start.iter().copied().max() < at_end.iter().copied().max());
}

#[test]
fn credits_zebra_stripe_follows_table_offset() {
    let credits = [
        HeroCredit {
            name: "One".into(),
            role: "Actor".into(),
        },
        HeroCredit {
            name: "Two".into(),
            role: "Writer".into(),
        },
    ];
    let mut terminal = Terminal::new(TestBackend::new(24, 1)).unwrap();
    terminal
        .draw(|f| paint_credits_from(f, Rect::new(0, 0, 24, 1), &credits, 1))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].bg,
        palette::HERO_CREDITS_STRIPE
    );
}

#[test]
fn credits_column_geometry_uses_rows_before_scroll_offset() {
    let credits = [
        HeroCredit {
            name: "The Longest Name".into(),
            role: "Director".into(),
        },
        HeroCredit {
            name: "X".into(),
            role: "Actor".into(),
        },
    ];
    let draw = |offset| {
        let mut terminal = Terminal::new(TestBackend::new(32, 1)).unwrap();
        terminal
            .draw(|f| paint_credits_from(f, Rect::new(0, 0, 32, 1), &credits, offset))
            .unwrap();
        (0..32)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>()
    };
    let at_start = draw(0);
    let after_scroll = draw(1);
    assert_eq!(&at_start[24..32], "Director");
    assert_eq!(&after_scroll[27..32], "Actor");
    assert_eq!(&after_scroll[16..18], "  ");
}

#[test]
fn credits_clip_and_role_column_survives_long_name() {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(24, 3)).unwrap();
    terminal
        .draw(|f| {
            paint_credits_from(
                f,
                Rect::new(0, 0, 24, 1),
                &[HeroCredit {
                    name: "A very long credit name".into(),
                    role: "Actor".into(),
                }],
                0,
            );
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 1)].symbol(), " ");
    let row = (0..24)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect::<String>();
    assert_eq!(&row[19..24], "Actor", "role ends at the box edge: {row:?}");
    assert_eq!(&row[16..18], "  ", "name/role gap is retained: {row:?}");
}

#[test]
fn credits_roles_use_the_full_remaining_width_and_right_align() {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(36, 2)).unwrap();
    terminal
        .draw(|f| {
            paint_credits_from(
                f,
                Rect::new(2, 0, 30, 2),
                &[
                    HeroCredit {
                        name: "Ann".into(),
                        role: "Lead".into(),
                    },
                    HeroCredit {
                        name: "Alexandra".into(),
                        role: "Cinematographer".into(),
                    },
                ],
                0,
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let first = (2..32).map(|x| buf[(x, 0)].symbol()).collect::<String>();
    let second = (2..32).map(|x| buf[(x, 1)].symbol()).collect::<String>();
    assert_eq!(
        &first[26..30],
        "Lead",
        "short role is right-aligned: {first:?}"
    );
    assert_eq!(
        &second[15..30],
        "Cinematographer",
        "long role uses the remaining width: {second:?}"
    );
    assert_eq!(
        &first[11..26],
        "               ",
        "gap spans to the edge: {first:?}"
    );
}

#[test]
fn credits_truncate_roles_at_the_right_edge() {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(24, 1)).unwrap();
    terminal
        .draw(|f| {
            paint_credits_from(
                f,
                Rect::new(0, 0, 24, 1),
                &[HeroCredit {
                    name: "A very long credit name".into(),
                    role: "Director of Photography".into(),
                }],
                0,
            );
        })
        .unwrap();
    let row = (0..24)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect::<String>();
    let role = row.chars().skip(18).collect::<String>();
    assert_eq!(
        role, "Direc…",
        "truncated role ends at the box edge: {row:?}"
    );
}
