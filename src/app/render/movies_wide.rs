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

use super::hero_left;
use super::home_hero::{prepare_wide_emby_hero_card, HeroData};
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Padding inside the wide Movies hero and list panels, matching Home's
/// wide hero column (`home.rs::HOME_HERO_PAD_X/Y`).
const PANE_PAD_X: u16 = 2;
const PANE_PAD_Y: u16 = 1;

impl App {
    /// Whether `lib_idx` is the dedicated Movies library (a `collection_type
    /// == "movies"` library that is not routed through the feed/home-video
    /// group view). Only this library gets the wide hero-on-left
    /// arrangement; home videos, podcasts, TV, and music keep their own.
    pub(super) fn is_wide_movies_library(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            lib.library.collection_type == "movies" && !self.is_feed_home_video_group_view(lib_idx)
        })
    }

    /// The selected Movie from the active list source: the inline-search
    /// result cursor while search is open (the hero must not read the stale
    /// navigation-level cursor then), else the navigation cursor item.
    fn selected_wide_movie(&self, lib_idx: usize) -> Option<mbv_core::api::EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        if let Some(s) = &lib.search {
            let idx = *s.results.get(s.cursor)?;
            return s.items.get(idx).cloned();
        }
        self.selected_movie_item(lib_idx)
    }

    /// Renders the wide Movies library: read-only shared selected-Emby hero
    /// card on the left, letter pills (or the active search box) plus the
    /// one-column Movies list in the right rail. Below the shared
    /// breakpoint the caller keeps the existing inline presentation.
    pub(super) fn render_wide_movies(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        let left_content_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };

        let Some((mut left_panel, right_panel)) = hero_left::shared_hero_presentation(area) else {
            return;
        };
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

        let left_area = Rect {
            y: left_panel.y.saturating_add(PANE_PAD_Y),
            height: left_panel.height.saturating_sub(PANE_PAD_Y * 2),
            ..left_panel
        };
        let right_area = Rect {
            y: right_panel.y.saturating_add(PANE_PAD_Y),
            height: right_panel.height.saturating_sub(PANE_PAD_Y * 2),
            ..right_panel
        };
        layout.movies_wide_right_area = right_area;

        // ── Left pane: read-only shared hero card ────────────────────────
        // The card is not published as `layout.hero_area`, so it is never an
        // interactive hero: no hero-pane focus state or activation path.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_RESTING)),
            left_panel,
        );
        let hero_content = Rect {
            x: left_area.x.saturating_add(PANE_PAD_X),
            y: left_area.y,
            width: left_area.width.saturating_sub(PANE_PAD_X * 2),
            height: left_area.height,
        };
        let hero_data = self.selected_wide_movie(lib_idx).and_then(|item| {
            prepare_wide_emby_hero_card(&item, hero_content).map(
                |(meta_layout, meta_area, img_area)| {
                    HeroData::Emby(
                        Box::new(item),
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

        if let Some(s) = self.libs[lib_idx].search.as_ref() {
            super::hero::render_search_box(f, pills_area, &s.query, s.loading);
        } else if self.is_home_video_view(lib_idx) {
            let total = self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|level| level.total_count)
                .unwrap_or(0);
            super::render_count_label(f, pills_area, total);
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
        let list_area = Rect {
            x: list_panel.x.saturating_add(PANE_PAD_X),
            y: list_panel.y.saturating_add(PANE_PAD_Y),
            width: list_panel.width.saturating_sub(PANE_PAD_X * 2),
            height: list_panel.height.saturating_sub(PANE_PAD_Y * 2),
        };

        self.render_wide_library_rows(f, list_area, lib_idx, focused, layout);

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
