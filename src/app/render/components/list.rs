use crate::app::layout::LayoutMain;
use crate::app::library_column_width::library_column_count;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::Frame;

pub(in crate::app) fn render_generic_movies_home_video_rows_with_ctx(
    f: &mut Frame,
    list_area: Rect,
    ctx: &LibraryListRenderCtx,
    focused: bool,
    layout: &mut LayoutMain,
) -> usize {
    layout.left_area = list_area;
    if ctx.items.is_empty() {
        crate::app::render::render_placeholder(
            f,
            list_area,
            if ctx.loading {
                " Loading…"
            } else {
                " (empty)"
            },
        );
        return 0;
    } else {
        let row_ctx = ctx.rows(list_area, library_column_count(list_area.width), focused, 0);
        if !ctx.is_search_active() && (ctx.true_total() >= 50 || ctx.letter_filter.is_some()) {
            super::list_letter_groups::render_letter_grouped_rows(
                f,
                row_ctx,
                ctx.letter_filter.clone(),
                ctx.true_total(),
                layout,
            )
        } else {
            super::list_plain::render_plain_rows(f, row_ctx, layout)
        }
    }
}

impl App {
    /// Whether `lib_idx` is the dedicated Movies library (a
    /// `collection_type == "movies"` library that is not routed through the
    /// feed/home-video group view). Only this library gets the wide
    /// hero-on-left arrangement; home videos, podcasts, TV, and music keep
    /// their own.
    pub(in crate::app::render) fn is_wide_movies_library(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            lib.library.collection_type == "movies" && !self.is_feed_home_video_group_view(lib_idx)
        })
    }
}
