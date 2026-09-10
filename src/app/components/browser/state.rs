use ratatui::layout::Rect;

use super::BrowserComponent;
use crate::app::components::media_list::ViewportAnchor;
use crate::app::render::HomeImagePaint;

impl BrowserComponent {
    pub(in crate::app) fn owns_canonical_position(&self) -> bool {
        self.has_active_control()
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        if self.wide_movies {
            self.wide_list
                .selected_target()
                .copied()
                .unwrap_or(self.cursor)
        } else if self.uses_inline_control() {
            self.inline_browser
                .selected_target()
                .copied()
                .unwrap_or(self.cursor)
        } else {
            self.cursor
        }
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        if self.wide_movies {
            self.wide_list.scroll()
        } else if self.uses_inline_control() {
            self.inline_browser.scroll()
        } else {
            self.scroll
        }
    }

    /// Test-only: the persistent canonical controls' own public selection and
    /// scroll state, for asserting which control `apply_position` seeded. The
    /// `cursor()`/`scroll()` getters delegate to the ACTIVE control (or the
    /// legacy fields when none is active), so they cannot observe a control
    /// that is seeded but not currently active.
    #[cfg(test)]
    pub(crate) fn test_inline_selected_target(&self) -> Option<usize> {
        self.inline_browser.selected_target().copied()
    }

    #[cfg(test)]
    pub(crate) fn test_inline_scroll(&self) -> usize {
        self.inline_browser.scroll()
    }

    #[cfg(test)]
    pub(crate) fn test_wide_selected_target(&self) -> Option<usize> {
        self.wide_list.selected_target().copied()
    }

    #[cfg(test)]
    pub(crate) fn test_wide_scroll(&self) -> usize {
        self.wide_list.scroll()
    }

    pub(in crate::app) fn viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        self.active_viewport_anchor(viewport_height)
    }

    pub(in crate::app) fn apply_viewport_anchor(&mut self, anchor: ViewportAnchor<String>) {
        // Apply the explicit target immediately when content is already loaded;
        // the painted view still consumes the pending anchor to place the row.
        if let Some(cursor) = self
            .context
            .items
            .iter()
            .position(|item| item.id == anchor.selected_target)
        {
            if self.wide_movies {
                self.wide_list.select_index(cursor);
            } else if self.uses_inline_control() {
                self.inline_browser.select_index(cursor);
            }
        }
        self.preserved_anchor = Some(anchor.clone());
        self.pending_anchor = Some(anchor);
    }

    pub(in crate::app) fn painted_viewport_height(&self) -> usize {
        self.layout.left_area.height as usize
    }

    /// Records the wide layout's pill-row presentation from validated shell
    /// content; whether the layout is wide is derived locally in `view()`.
    pub(in crate::app) fn configure_wide_movies(&mut self, home_video: bool, letter_pills: bool) {
        self.wide_movies_home_video = home_video;
        self.wide_movies_letter_pills = letter_pills;
    }

    /// Runtime terminal-capability flag (task 5.3d.17a): mirrors
    /// `HomeComponent::set_use_nerd_fonts` so the component can paint the
    /// wide hero text.
    pub(in crate::app) fn set_use_nerd_fonts(&mut self, use_nerd_fonts: bool) {
        self.use_nerd_fonts = use_nerd_fonts;
    }

    pub(in crate::app) fn set_images_enabled(&mut self, images_enabled: bool) {
        self.images_enabled = images_enabled;
    }

    /// Takes the hero cover image (if any) `view()` computed but could not
    /// paint itself. The shell calls this right after `application.view()`
    /// returns and paints it via `App::paint_home_image` (mirrors
    /// `HomeComponent::take_image_paint`, task 5.3d.17a).
    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    pub(in crate::app) fn menu_placement_geometry(&self) -> Option<(Rect, Option<Rect>)> {
        (self.layout.left_area.width > 0 && self.layout.left_area.height > 0)
            .then_some((self.layout.left_area, self.layout.selected_item_rect))
    }
}
