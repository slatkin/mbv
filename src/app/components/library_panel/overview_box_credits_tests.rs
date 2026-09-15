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
    fn set_presentation(
        &mut self,
        _presentation: crate::app::components::media_list::Presentation,
        _viewport_height: usize,
    ) {
    }

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
        links: Vec::new(),
        artwork: HeroArtwork {
            shape,
            source: None,
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
            paint_overview_box(f, Rect::new(0, 0, 32, 12), 0, &content, 0);
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
fn long_overview_is_clipped_to_inner_box_when_pane_is_short() {
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
        .draw(|f| metrics = paint_overview_box(f, Rect::new(0, 0, 32, 6), 0, &content, 0))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let inner = Rect::new(2, 2, 28, 3);
    assert_eq!(metrics.unwrap().viewport, 0);
    assert!(
        (inner.bottom()..6).all(|y| {
            (inner.left()..inner.right()).all(|x| buffer[(x, y)].symbol() != "▁")
                && (inner.left()..inner.right()).all(|x| buffer[(x, y)].symbol() != "Person")
        }),
        "separator and table never paint below inner bottom"
    );
}

#[test]
fn pinned_overview_and_separator_stay_fixed_while_credits_scroll() {
    let credits = (0..12)
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
    let draw = |offset| {
        let mut terminal = Terminal::new(TestBackend::new(32, 14)).unwrap();
        let mut paint = None;
        terminal
            .draw(|f| paint = paint_overview_box(f, Rect::new(0, 0, 32, 14), 0, &content, offset))
            .unwrap();
        (terminal.backend().buffer().clone(), paint.unwrap())
    };
    let (at_start, metrics) = draw(0);
    let (at_end, _) = draw(metrics.content_length.saturating_sub(metrics.viewport));
    let separator_y = 3;
    assert_eq!(metrics.viewport, 8);
    for y in 2..=separator_y {
        for x in 0..31 {
            assert_eq!(at_start[(x, y)].symbol(), at_end[(x, y)].symbol());
        }
    }
    let row = |buffer: &ratatui::buffer::Buffer, y| {
        (0..32).map(|x| buffer[(x, y)].symbol()).collect::<String>()
    };
    assert_ne!(row(&at_start, 5), row(&at_end, 5));
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
                paint_overview_box(f, Rect::new(0, 0, 32, 14), 0, &content, offset);
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
            paint_credits(
                f,
                Rect::new(0, 0, 24, 1),
                &[HeroCredit {
                    name: "A very long credit name".into(),
                    role: "Actor".into(),
                }],
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
            paint_credits(
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
            paint_credits(
                f,
                Rect::new(0, 0, 24, 1),
                &[HeroCredit {
                    name: "A very long credit name".into(),
                    role: "Director of Photography".into(),
                }],
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
