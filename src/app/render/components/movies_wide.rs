//! Dedicated Movies library's wide (hero-on-left) rendering: the same
//! hero-on-left arrangement Home and Music use, composed with a read-only
//! shared selected-Emby hero card on the left and the letter pills +
//! one-column Movies list in the right rail (right-panel-arrangements spec).
//!
//! The left hero is a projection of the right-rail list cursor, never a
//! focus/activation surface: `LayoutMain.hero_area` is left unset here so
//! mouse handling treats the left pane as outside interactive geometry, and
//! the shared card is painted by `render_home_hero_data` (the exact Home
//! wide selected-media card, per library-list-hero spec).

use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::home_hero::{prepare_wide_emby_hero_card, HeroData};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::{palette, App};
use ratatui::layout::Rect;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

impl App {
    /// Whether `lib_idx` is the dedicated Movies library (a `collection_type
    /// == "movies"` library that is not routed through the feed/home-video
    /// group view). Only this library gets the wide hero-on-left
    /// arrangement; home videos, podcasts, TV, and music keep their own.
    pub(in crate::app::render) fn is_wide_movies_library(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            lib.library.collection_type == "movies" && !self.is_feed_home_video_group_view(lib_idx)
        })
    }

    /// The selected Movie from the active list source: the inline-search
    /// result cursor while search is open (the hero must not read the stale
    /// navigation-level cursor then), else the navigation cursor item.
    pub(in crate::app::render) fn selected_wide_movie(
        &self,
        lib_idx: usize,
        ctx: &LibraryListRenderCtx,
    ) -> Option<mbv_core::api::EmbyItem> {
        if ctx.is_search_active() {
            return ctx.items.get(ctx.cursor).cloned();
        }
        self.selected_movie_item(lib_idx)
    }

    pub(in crate::app::render) fn render_wide_movies_with_ctx(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        ctx: &LibraryListRenderCtx,
        selected_movie: Option<&mbv_core::api::EmbyItem>,
        layout: &mut LayoutMain,
    ) {
        let left_content_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };

        let Some(panes) = library_arrangement::wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y)
        else {
            return;
        };
        let mut left_panel = panes.left_panel;
        let right_panel = panes.right_panel;
        left_panel.height = left_content_area.height;

        // Library-side separator row below the left pane, matching Music and
        // Home's wide layout.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            Rect {
                x: left_panel.x,
                y: left_panel.bottom(),
                width: left_panel.width,
                height: 1,
            },
        );

        let left_area = panes.left_area;
        let right_area = panes.right_area;
        layout.movies_wide_right_area = right_area;

        // ── Left pane: read-only shared hero card ────────────────────────
        // The card is not published as `layout.hero_area`, so it is never an
        // interactive hero: no hero-pane focus state or activation path.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_RESTING)),
            left_panel,
        );
        let hero_content = padded_rect(left_area, PANE_PAD_X, 0);
        let hero_data = selected_movie.and_then(|item| {
            prepare_wide_emby_hero_card(item, hero_content).map(
                |(meta_layout, meta_area, img_area)| {
                    HeroData::Emby(
                        Box::new(item.clone()),
                        meta_area,
                        meta_area, // wide_area same as meta_area (image above text)
                        img_area,
                        meta_layout,
                    )
                },
            )
        });

        // ── Right rail: pills + one-column list ──────────────────────────
        // Pills sit at the top of the rail via the shared hero-on-left right
        // pane; an active inline search replaces that slot with the existing
        // search control.
        let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        let pills_area = right_pane.pills_area;
        let list_panel = right_pane.list_panel;

        if ctx.is_search_active() {
            crate::app::render::components::hero::render_search_box(
                f,
                pills_area,
                ctx.search_query.as_deref().unwrap_or_default(),
                ctx.search_loading,
            );
        } else if self.is_home_video_view(lib_idx) {
            crate::app::render::render_count_label(f, pills_area, ctx.total_count);
        } else if self.should_show_letter_pills(lib_idx) {
            self.render_letter_pills_row(f, pills_area, lib_idx, layout);
        }

        if list_panel.height > 0 {
            let list_bg = palette::resolve_surface_focus(focused);
            f.render_widget(
                Block::default().style(Style::default().bg(list_bg)),
                list_panel,
            );
        }
        let list_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);

        let final_scroll = super::list::render_generic_movies_home_video_rows_with_ctx(
            f, list_area, ctx, focused, layout,
        );
        if ctx.is_search_active() {
            if let Some(search) = &mut self.libs[lib_idx].search {
                search.scroll = final_scroll;
            }
        } else if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            level.scroll = final_scroll;
        }

        // Paint the shared hero card last (after the list, mirroring
        // `render_list`'s hero-after-list order). The exact Home wide
        // selected-media card: `id:pwr_kw` cache key, Backdrop/Primary/Logo
        // artwork preference, watch-state glyph, release date, duration,
        // overview, and graceful empty-field handling all come from
        // `render_home_hero_data`/`keep_watching_hero_*`.
        if let Some(hero_data) = &hero_data {
            self.render_home_hero_data(f, hero_data, true, focused);
        }

        hero_left::hero_on_left_list_panel_border(f, list_panel, focused);
    }
}

#[cfg(test)]
#[path = "movies_wide_tests.rs"]
mod tests;
