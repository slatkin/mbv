use ratatui::layout::Rect;

use super::BrowserComponent;
use super::Presentation;
use crate::app::components::media_list::ViewportAnchor;
use crate::app::render::HomeImagePaint;

impl BrowserComponent {
    /// The selected stable target of the presentation carrying the shared
    /// owner.
    pub(in crate::app) fn carrier_selected_target(&self) -> Option<String> {
        match self.carrier {
            Presentation::Wide => self.wide_list.selected_target().cloned(),
            Presentation::Inline => self.inline_browser.selected_target().cloned(),
            Presentation::Grid => self.grid.selected_target().cloned(),
        }
    }

    /// Move the shared owner's selection to `target` when present.
    pub(in crate::app) fn carrier_select_target(&mut self, target: &str) -> bool {
        let target = target.to_string();
        match self.carrier {
            Presentation::Wide => self.wide_list.select_target(&target),
            Presentation::Inline => self.inline_browser.select_target(&target),
            Presentation::Grid => self.grid.select_target(&target),
        }
    }

    pub(in crate::app) fn carrier_select_first(&mut self) {
        match self.carrier {
            Presentation::Wide => self.wide_list.select_first(),
            Presentation::Inline => self.inline_browser.select_first(),
            Presentation::Grid => self.grid.select_first(),
        }
    }

    pub(in crate::app) fn carrier_select_last(&mut self) {
        match self.carrier {
            Presentation::Wide => self.wide_list.select_last(),
            Presentation::Inline => self.inline_browser.select_last(),
            Presentation::Grid => self.grid.select_last(),
        }
    }

    /// Move the shared owner's selection by `delta` selectable rows.
    pub(in crate::app) fn carrier_move_selection(&mut self, delta: i64) {
        match self.carrier {
            Presentation::Wide => self.wide_list.move_selection(delta),
            Presentation::Inline => self.inline_browser.move_selection(delta),
            Presentation::Grid => self.grid.move_selection(delta),
        }
    }

    pub(in crate::app) fn carrier_scroll(&self) -> usize {
        match self.carrier {
            Presentation::Wide => self.wide_list.scroll(),
            Presentation::Inline => self.inline_browser.scroll(),
            Presentation::Grid => self.grid.scroll(),
        }
    }

    pub(in crate::app) fn carrier_set_scroll(&mut self, offset: usize) {
        match self.carrier {
            Presentation::Wide => self.wide_list.set_scroll(offset),
            Presentation::Inline => self.inline_browser.set_scroll(offset),
            Presentation::Grid => self.grid.set_scroll(offset),
        }
    }

    /// Resolve the carrying presentation's viewport and retain the resulting
    /// resting offset.
    pub(in crate::app) fn carrier_sync_viewport(&mut self) {
        let viewport_height = self.painted_viewport_height().max(1);
        let offset = match self.carrier {
            Presentation::Wide => self.wide_list.resolve_viewport(viewport_height).offset,
            Presentation::Inline => self.inline_browser.resolve_viewport(viewport_height).offset,
            Presentation::Grid => self.grid.resolve_viewport(viewport_height).offset,
        };
        self.carrier_set_scroll(offset);
    }

    /// The component-resolved selected target of the active presentation (the
    /// legacy `wide`/`inline` branch vocabulary, kept for the painted-geometry
    /// dispatch the mouse path uses).
    pub(in crate::app) fn active_selected_target(&self) -> Option<String> {
        if self.wide_movies {
            self.wide_list.selected_target().cloned()
        } else if self.uses_inline_control() {
            self.inline_browser.selected_target().cloned()
        } else {
            self.grid.selected_target().cloned()
        }
    }

    /// The authoritative selection of the shared owner, as a `context.items`
    /// index. The owner is the only cursor store; no shell mirror exists.
    pub(in crate::app) fn owns_canonical_position(&self) -> bool {
        true
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier_selected_target()
            .and_then(|target| self.context.items.iter().position(|item| item.id == target))
            .unwrap_or(0)
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier_scroll()
    }

    pub(in crate::app) fn viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        self.carrier_viewport_anchor(viewport_height)
    }

    pub(in crate::app) fn apply_viewport_anchor(&mut self, anchor: ViewportAnchor<String>) {
        self.ensure_carrier();
        self.apply_anchor_to_carrier(&anchor, self.painted_viewport_height());
        self.preserved_anchor = Some(anchor);
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
