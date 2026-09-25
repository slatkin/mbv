use super::*;
use crate::app::components::library_panel::{
    LibraryKey, LibraryKind, LibraryPanel, LibraryPanelContent, ListSlot,
};
use crate::app::components::tv_content::TvContent;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use ratatui::layout::Rect;

use crate::app::components::inline_search::{InlineSearch, SearchPool};
use crate::app::components::library_panel::content::{
    ArtworkShape, HeroArtwork, HeroContent, HeroFacts, PanelList, SelectorRow, Workspace,
    WorkspaceHeader,
};
use crate::app::render::arrangements::library::{
    wide_library_panes, wide_library_panes_with_selector,
};
use ratatui::backend::TestBackend;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

// Tall enough for a Landscape header (16:9 artwork box) to leave the
// title and meta rows visible below it.
const AREA: Rect = Rect::new(0, 0, 100, 30);

/// A test double over [`PanelList`]: paints its rows and retains the rect
/// it was viewed into.
struct StubList {
    rows: Vec<&'static str>,
    painted: Option<Rect>,
}

impl StubList {
    fn with_rows(rows: Vec<&'static str>) -> Self {
        Self {
            rows,
            painted: None,
        }
    }
}

impl PanelList for StubList {
    fn clamp_viewport(&mut self, _viewport_height: usize) {}

    fn set_paint_policy(
        &mut self,
        _policy: crate::app::components::library_panel::content::PanelListPaintPolicy,
    ) {
    }

    fn view(&mut self, f: &mut Frame, rect: Rect) {
        self.painted = Some(rect);
        for (index, row) in self.rows.iter().enumerate() {
            if index as u16 >= rect.height {
                break;
            }
            f.render_widget(
                Paragraph::new(*row),
                Rect {
                    y: rect.y + index as u16,
                    height: 1,
                    ..rect
                },
            );
        }
    }
}

fn hero_facts(title: &str) -> HeroFacts {
    HeroFacts {
        title: title.into(),
        meta_rows: vec!["2020".into()],
        duration_row: None,
        progress_row: None,
        links: Vec::new(),
        artwork: HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: None,
            decoration: None,
            image: crate::app::components::library_panel::content::HeroImageState::None,
        },
    }
}

fn draw_skeleton(
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
) -> (ratatui::buffer::Buffer, WideSkeletonGeometry, SkeletonHits) {
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    let mut hits = SkeletonHits::default();
    let mut windows = SkeletonPillWindows::default();
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = render_wide_skeleton(
                f,
                AREA,
                content,
                browser_focused,
                None,
                0,
                None,
                None,
                &mut hits,
                &mut windows,
                80, // tall terminal: the short-pane caps must not skew the skeleton tests
                true,
            );
        })
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        geometry.expect("wide skeleton painted"),
        hits,
    )
}

mod search;
mod skeleton;
mod workspace;

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    outer.left() <= inner.left()
        && outer.right() >= inner.right()
        && outer.top() <= inner.top()
        && outer.bottom() >= inner.bottom()
}
