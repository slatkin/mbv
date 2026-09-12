use ratatui::layout::Rect;

use super::BrowserComponent;
use crate::app::components::media_list::ViewportAnchor;
use crate::app::render::HomeImagePaint;

impl BrowserComponent {
    /// The authoritative selection of the shared owner, as a `context.items`
    /// index. The owner is the only cursor store; no shell mirror exists.
    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier
            .selected_target()
            .and_then(|target| {
                self.context
                    .items
                    .iter()
                    .position(|item| item.id == *target)
            })
            .unwrap_or(0)
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier.scroll()
    }

    pub(in crate::app) fn viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        self.carrier.viewport_anchor(viewport_height)
    }

    pub(in crate::app) fn apply_viewport_anchor(&mut self, anchor: ViewportAnchor<String>) {
        self.ensure_carrier();
        self.carrier
            .apply_viewport_anchor(&anchor, self.painted_viewport_height());
        self.preserved_anchor = Some(anchor);
    }

    pub(in crate::app) fn painted_viewport_height(&self) -> usize {
        self.layout.height as usize
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
        (self.layout.width > 0 && self.layout.height > 0)
            .then_some((self.layout, self.selected_item_rect))
    }
}
