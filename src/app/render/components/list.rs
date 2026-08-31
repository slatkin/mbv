use crate::app::layout::LayoutMain;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use ratatui::layout::Rect;
use ratatui::Frame;

pub(in crate::app) fn render_generic_movies_home_video_rows_with_ctx(
    f: &mut Frame,
    list_area: Rect,
    ctx: &LibraryListRenderCtx,
    focused: bool,
    columns: usize,
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
        0
    } else {
        let row_ctx = ctx.rows(list_area, columns, focused, 0);
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
