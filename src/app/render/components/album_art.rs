use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

const INLINE_ALBUM_ART_COLS: u16 = 30;
pub(in crate::app::render) const INLINE_ALBUM_ART_ROWS: u16 = 15;
const INLINE_ALBUM_ART_GAP: u16 = 2;
const INLINE_ALBUM_ART_RIGHT_PAD: u16 = 2;
pub(in crate::app::render) const INLINE_ALBUM_ART_RESERVED: u16 =
    INLINE_ALBUM_ART_COLS + INLINE_ALBUM_ART_GAP + INLINE_ALBUM_ART_RIGHT_PAD;

pub(in crate::app::render) fn inline_album_art_cache_key(album_id: &str) -> String {
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
    Center,
    Right,
}

#[derive(Clone, Copy)]
enum ArtAnchorY {
    Top,
}

/// Places a `w`x`h` image within `container` anchored to the given corner/edge,
/// letterboxing the leftover margin to the opposite side(s).
fn align_art(container: Rect, w: u16, h: u16, ax: ArtAnchorX, ay: ArtAnchorY) -> Rect {
    let free_w = container.width.saturating_sub(w);
    let x = match ax {
        ArtAnchorX::Center => container.x + free_w / 2,
        ArtAnchorX::Right => container.x + free_w,
    };
    let y = match ay {
        ArtAnchorY::Top => container.y,
    };
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

impl App {
    pub(in crate::app) fn paint_music_image(
        &mut self,
        f: &mut Frame,
        image_paint: Option<MusicImagePaint>,
    ) {
        let Some(MusicImagePaint::Album {
            area,
            album,
            centered,
        }) = image_paint
        else {
            return;
        };
        if centered {
            self.render_inline_album_art_centered(f, area, &album, &mut LayoutMain::default());
        } else {
            self.render_inline_album_art(f, area, &album, &mut LayoutMain::default());
        }
    }

    pub(in crate::app::render) fn render_inline_album_art(
        &mut self,
        f: &mut Frame,
        area: Rect,
        album: &mbv_core::api::EmbyItem,
        _layout: &mut LayoutMain,
    ) {
        if !self.images_enabled() || area.width < 4 || area.height < 2 {
            return;
        }

        let box_rect = inline_art_box_rect(area);
        let nav_gate_open = self.right_panel_image_renders_allowed();
        self.render_inline_art_cell(
            f,
            box_rect,
            album,
            inline_album_art_cache_key(&album.id),
            nav_gate_open,
            false,
            (ArtAnchorX::Right, ArtAnchorY::Top),
        );
    }

    pub(in crate::app::render) fn render_inline_album_art_centered(
        &mut self,
        f: &mut Frame,
        area: Rect,
        album: &mbv_core::api::EmbyItem,
        _layout: &mut LayoutMain,
    ) {
        if !self.images_enabled() || area.width < 4 || area.height < 2 {
            return;
        }

        let box_rect = Rect {
            x: area.x,
            y: area.y,
            width: INLINE_ALBUM_ART_COLS.min(area.width),
            height: INLINE_ALBUM_ART_ROWS.min(area.height),
        };
        let box_rect = Rect {
            x: area.x + area.width.saturating_sub(box_rect.width) / 2,
            ..box_rect
        };
        let nav_gate_open = self.right_panel_image_renders_allowed();
        self.render_inline_art_cell(
            f,
            box_rect,
            album,
            inline_album_art_cache_key(&album.id),
            nav_gate_open,
            false,
            (ArtAnchorX::Center, ArtAnchorY::Top),
        );
    }

    /// Fetches + renders a single album cover into `cell`, falling back to the
    /// `OVERLAY` loading placeholder while the image isn't yet decoded/gated.
    /// Returns the rect actually painted (image or placeholder).
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
        album: &mbv_core::api::EmbyItem,
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
                crate::app::render::MUSIC_ALBUM_IMAGE_TYPES,
            );
        } else {
            self.fetch_card_image(
                cache_key.clone(),
                album.id.clone(),
                album.series_id.clone(),
                crate::app::render::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }

        let mut img_rect = cell;
        let mut use_placeholder = true;

        if nav_gate_open {
            if let Some(state) = self.cached_image_protocol_mut(&cache_key) {
                if let Some(actual) = state.size_for(
                    ratatui_image::Resize::Scale(Some(crate::app::render::RENDER_FILTER)),
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
                Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                img_rect,
            );
        } else if let Some(state) = self.cached_image_protocol_mut(&cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            f.render_stateful_widget(
                SImg::default().resize(ratatui_image::Resize::Scale(Some(
                    crate::app::render::RENDER_FILTER,
                ))),
                img_rect,
                state,
            );
        }

        img_rect
    }
}

/// Album artwork that a render component computed but cannot paint because
/// image-cache authority remains in the shell during the migration.
pub(in crate::app) enum MusicImagePaint {
    Album {
        area: Rect,
        album: Box<mbv_core::api::EmbyItem>,
        centered: bool,
    },
}
