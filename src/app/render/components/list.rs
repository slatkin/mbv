use crate::app::render::components::list_rows::LibraryListRenderCtx;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Legacy painter (design.md D7): the last live caller went to the canonical
/// carrier; unit U2 deletes this file.
#[allow(dead_code)]
pub(in crate::app) fn render_generic_movies_home_video_rows_with_ctx(
    f: &mut Frame,
    list_area: Rect,
    ctx: &LibraryListRenderCtx,
    focused: bool,
    columns: usize,
    layout: &mut Rect,
) -> usize {
    *layout = list_area;
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
        let row_ctx = ctx.rows(list_area, columns, focused);
        if !ctx.is_search_active() && (ctx.true_total() >= 50 || ctx.letter_filter.is_some()) {
            super::list_letter_groups::render_letter_grouped_rows(
                f,
                row_ctx,
                ctx.letter_filter.clone(),
                ctx.true_total(),
            )
        } else {
            super::media_list::render_plain_rows(f, row_ctx)
        }
    }
}
