use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

const INLINE_ALBUM_ART_COLS: u16 = 24;
pub(super) const INLINE_ALBUM_ART_ROWS: u16 = 12;
const INLINE_ALBUM_ART_GAP: u16 = 2;
const INLINE_ALBUM_ART_RIGHT_PAD: u16 = 2;
pub(super) const INLINE_ALBUM_ART_RESERVED: u16 =
    INLINE_ALBUM_ART_COLS + INLINE_ALBUM_ART_GAP + INLINE_ALBUM_ART_RIGHT_PAD;

fn inline_album_art_cache_key(album_id: &str) -> String {
    format!("{album_id}:P")
}

/// Computes the reserved-column art box: right-aligned within `area`
/// (leaving `INLINE_ALBUM_ART_RIGHT_PAD`), sized up to
/// `INLINE_ALBUM_ART_COLS`x`INLINE_ALBUM_ART_ROWS` (clamped to `area`).
/// Shared by the single-album inline-art path and the artist-header collage
/// so their outer geometry can't drift apart.
fn inline_art_box_rect(area: Rect) -> Rect {
    let box_w = INLINE_ALBUM_ART_COLS.min(area.width);
    let box_h = INLINE_ALBUM_ART_ROWS.min(area.height);
    Rect {
        x: area.x
            + area
                .width
                .saturating_sub(box_w + INLINE_ALBUM_ART_RIGHT_PAD),
        y: area.y,
        width: box_w,
        height: box_h,
    }
}

#[derive(Clone, Copy)]
enum ArtAnchorX {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy)]
enum ArtAnchorY {
    Top,
    Center,
    Bottom,
}

/// Places a `w`x`h` image within `container` anchored to the given corner/edge,
/// letterboxing the leftover margin to the opposite side(s). The single-album
/// art uses `(Right, Top)`; collage tiles anchor toward the box center so any
/// margin falls on the outer edges and the tiles abut with no internal seam.
fn align_art(container: Rect, w: u16, h: u16, ax: ArtAnchorX, ay: ArtAnchorY) -> Rect {
    let free_w = container.width.saturating_sub(w);
    let free_h = container.height.saturating_sub(h);
    let x = match ax {
        ArtAnchorX::Left => container.x,
        ArtAnchorX::Center => container.x + free_w / 2,
        ArtAnchorX::Right => container.x + free_w,
    };
    let y = match ay {
        ArtAnchorY::Top => container.y,
        ArtAnchorY::Center => container.y + free_h / 2,
        ArtAnchorY::Bottom => container.y + free_h,
    };
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

impl App {
    pub(super) fn render_inline_album_art(
        &mut self,
        f: &mut Frame,
        area: Rect,
        album: &mbv_core::api::MediaItem,
        layout: &mut LayoutMain,
    ) {
        if !self.images_enabled() || area.width < 4 || area.height < 2 {
            return;
        }

        let box_rect = inline_art_box_rect(area);
        let nav_gate_open = self.power_right_panel_image_renders_allowed();
        let img_rect = self.render_inline_art_cell(
            f,
            box_rect,
            album,
            inline_album_art_cache_key(&album.id),
            nav_gate_open,
            false,
            (ArtAnchorX::Right, ArtAnchorY::Top),
        );
        layout.inline_image_rect = Some(img_rect);
    }

    /// Renders a 2x2 (or fewer) collage of an artist's album covers in
    /// `area`, for the selected artist header's block. Each tile is fetched
    /// center-cropped to a square (a `:sq`-suffixed cache key, distinct from
    /// the standalone album image) so the covers form an even grid.
    ///
    /// Fill behavior: 1 album fills the whole box; 2 split into left/right
    /// halves; 3+ use a 2x2 grid (top-left, top-right, bottom-left,
    /// bottom-right) with only the first 4 albums shown. When 3 albums are
    /// given, the 4th (bottom-right) cell is simply left unpainted, showing
    /// the selected-block background through.
    ///
    /// Each tile anchors toward the box center (e.g. the top-left tile pins its
    /// bottom-right corner) so the squares abut with no internal seam; any
    /// letterbox margin falls on the box's outer edges instead.
    pub(super) fn render_inline_artist_collage(
        &mut self,
        f: &mut Frame,
        area: Rect,
        albums: &[mbv_core::api::MediaItem],
        layout: &mut LayoutMain,
    ) {
        if !self.images_enabled() || area.width < 4 || area.height < 2 || albums.is_empty() {
            return;
        }

        let box_rect = inline_art_box_rect(area);
        layout.inline_image_rect = Some(box_rect);

        // Each entry is `(cell, (anchor_x, anchor_y))`; anchors point toward the
        // box center so adjacent tiles meet at the seam.
        let cells: Vec<(Rect, (ArtAnchorX, ArtAnchorY))> = if albums.len() == 1 {
            vec![(box_rect, (ArtAnchorX::Center, ArtAnchorY::Center))]
        } else if albums.len() == 2 {
            let left_w = box_rect.width / 2;
            vec![
                (
                    Rect {
                        x: box_rect.x,
                        y: box_rect.y,
                        width: left_w,
                        height: box_rect.height,
                    },
                    (ArtAnchorX::Right, ArtAnchorY::Center),
                ),
                (
                    Rect {
                        x: box_rect.x + left_w,
                        y: box_rect.y,
                        width: box_rect.width - left_w,
                        height: box_rect.height,
                    },
                    (ArtAnchorX::Left, ArtAnchorY::Center),
                ),
            ]
        } else {
            let half_w = box_rect.width / 2;
            let half_h = box_rect.height / 2;
            vec![
                (
                    Rect {
                        x: box_rect.x,
                        y: box_rect.y,
                        width: half_w,
                        height: half_h,
                    },
                    (ArtAnchorX::Right, ArtAnchorY::Bottom),
                ),
                (
                    Rect {
                        x: box_rect.x + half_w,
                        y: box_rect.y,
                        width: box_rect.width - half_w,
                        height: half_h,
                    },
                    (ArtAnchorX::Left, ArtAnchorY::Bottom),
                ),
                (
                    Rect {
                        x: box_rect.x,
                        y: box_rect.y + half_h,
                        width: half_w,
                        height: box_rect.height - half_h,
                    },
                    (ArtAnchorX::Right, ArtAnchorY::Top),
                ),
                (
                    Rect {
                        x: box_rect.x + half_w,
                        y: box_rect.y + half_h,
                        width: box_rect.width - half_w,
                        height: box_rect.height - half_h,
                    },
                    (ArtAnchorX::Left, ArtAnchorY::Top),
                ),
            ]
        };

        let nav_gate_open = self.power_right_panel_image_renders_allowed();
        for ((cell, anchor), album) in cells.iter().zip(albums.iter().take(4)) {
            self.render_inline_art_cell(
                f,
                *cell,
                album,
                format!("{}:sq", album.id),
                nav_gate_open,
                true,
                *anchor,
            );
        }
    }

    /// Fetches + renders a single album cover into `cell`, falling back to the
    /// `OVERLAY` loading placeholder while the image isn't yet decoded/gated.
    /// Returns the rect actually painted (image or placeholder). Shared by the
    /// single-album art path and each quadrant of the collage.
    ///
    /// When `square` is set, the cover is fetched center-cropped to a square
    /// (via `fetch_card_image_square`) — the collage mode, giving uniform grid
    /// tiles; otherwise the natural-aspect cover is fetched. Placement within
    /// `cell` follows `anchor` (the standalone path uses `(Right, Top)`;
    /// collage tiles anchor toward the box center so they abut).
    fn render_inline_art_cell(
        &mut self,
        f: &mut Frame,
        cell: Rect,
        album: &mbv_core::api::MediaItem,
        cache_key: String,
        nav_gate_open: bool,
        square: bool,
        anchor: (ArtAnchorX, ArtAnchorY),
    ) -> Rect {
        if cell.width == 0 || cell.height == 0 {
            return cell;
        }

        if square {
            self.fetch_card_image_square(
                cache_key.clone(),
                album.id.clone(),
                album.series_id.clone(),
                super::MUSIC_ALBUM_IMAGE_TYPES,
            );
        } else {
            self.fetch_card_image(
                cache_key.clone(),
                album.id.clone(),
                album.series_id.clone(),
                super::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }

        let mut img_rect = cell;
        let mut use_placeholder = true;

        if nav_gate_open {
            if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                if let Some(actual) = state.size_for(
                    ratatui_image::Resize::Scale(Some(super::POWER_RENDER_FILTER)),
                    ratatui::layout::Size {
                        width: cell.width,
                        height: cell.height,
                    },
                ) {
                    img_rect = align_art(cell, actual.width, actual.height, anchor.0, anchor.1);
                    use_placeholder = false;
                }
            }
        }

        if use_placeholder {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::OVERLAY)),
                img_rect,
            );
        } else if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            f.render_stateful_widget(
                SImg::default().resize(ratatui_image::Resize::Scale(Some(
                    super::POWER_RENDER_FILTER,
                ))),
                img_rect,
                state,
            );
        }

        img_rect
    }
}
