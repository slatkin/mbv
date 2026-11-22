//! The terminal font size to neutral cell size conversion (design D7).
//!
//! [`mbv_ui_model::geom::CellSize`] is the neutral value that pixel
//! composition (`mbv_images::title_overlay`) sizes its bitmaps with. The
//! terminal toolkit measures the cell, and this crate is the owner of that
//! toolkit. So the conversion lives here, outside the type, as the
//! [`ToCellSize`] extension trait. A Client with no terminal toolkit (the GUI)
//! builds no `CellSize` from a picker and never depends on this crate.

use mbv_ui_model::geom::CellSize;

/// The neutral cell size of a terminal font measurement.
///
/// Implemented for `ratatui_image::FontSize`, the only font measurement the
/// protocol store reads. The conversion is total: it copies the measured pixel
/// width and height and clamps nothing, so a reader that treats a zero side as
/// unusable sees the measurement as taken. `Picker::font_size` supplies it to
/// the callers that hold a picker.
pub trait ToCellSize {
    /// The neutral cell size for this font measurement.
    fn to_cell_size(&self) -> CellSize;
}

impl ToCellSize for ratatui_image::FontSize {
    fn to_cell_size(&self) -> CellSize {
        CellSize::new(self.width, self.height)
    }
}
