//! The `PanelList` implementation (task 5.8, design D3): one object-safe
//! implementation over the shared media-list carrier for every `Target`.
//! The panel drives the carrier through this surface — breakpoint choice
//! (`set_presentation`), the paint policy, the slot-rect view, and the
//! retained-geometry reads — while typed target resolution stays on the
//! carrier's own surface, so no per-destination `ListSlot` arm can grow.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::component::Component;

use crate::app::components::media_list::{
    MediaListCarrier, Presentation, WideMediaListPaintPolicy, ZebraStripe,
};
use crate::app::palette::{self, Surface};

use super::content::{PanelList, PanelListPaintPolicy};

/// The zebra pair a surface resolves to for its focused and unfocused fills.
fn zebra_stripe(surface: Surface) -> ZebraStripe {
    ZebraStripe {
        focused: palette::surface_colors(surface, true).fill,
        unfocused: palette::surface_colors(surface, false).fill,
    }
}

impl<Target: Clone + PartialEq> PanelList for MediaListCarrier<Target> {
    fn set_presentation(&mut self, presentation: Presentation, viewport_height: usize) {
        // Path syntax prefers the inherent method, so this forwards to the
        // carrier's own re-anchoring implementation rather than recursing
        // into the trait.
        MediaListCarrier::set_presentation(self, presentation, viewport_height);
    }

    fn clear_selection(&mut self) {
        MediaListCarrier::clear_selection(self);
    }

    fn set_paint_policy(&mut self, policy: PanelListPaintPolicy) {
        match policy {
            PanelListPaintPolicy::Wide { focused } => {
                self.wide_mut().set_paint_policy(
                    WideMediaListPaintPolicy::new(focused)
                        .with_zebra(zebra_stripe(Surface::MainContentBox)),
                );
            }
            PanelListPaintPolicy::WideWorkspace { focused } => {
                self.wide_mut().set_paint_policy(
                    WideMediaListPaintPolicy::for_library_workspace(focused)
                        .with_zebra(zebra_stripe(Surface::LibraryPanel)),
                );
            }
        }
    }

    fn view(&mut self, frame: &mut Frame, rect: Rect) {
        let _ = Presentation::Wide;
        self.wide_mut().view(frame, rect);
    }

    fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        self.wide_mut().set_geometry(claim_rect, content_rect);
    }

    fn selected_row_rect(&self) -> Option<Rect> {
        self.current_selected_row_rect()
    }

    fn claims_point(&self, point: Position) -> bool {
        self.claims_current_point(point)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod panel_list_tests {
    use super::*;
    use crate::app::components::media_list::{
        MediaKind, MediaListRow, MediaSemanticState, Presentation,
    };
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::Terminal;

    fn item(target: &str) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: target.into(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    /// The selected-row accent is intentionally identical across both Wide
    /// arms; arm-specific stripe colours are owned by the Render Component
    /// regressions planned in tasks 4.1 and 4.2, at
    /// `src/app/render/components/media_list.rs` and its Wide-arm tests.
    #[test]
    fn wide_selected_rows_use_the_gutter_accent_for_both_slots() {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![item("selected")]);
        let area = Rect::new(0, 0, 20, 1);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();

        terminal
            .draw(|f| {
                PanelList::set_paint_policy(
                    &mut carrier,
                    PanelListPaintPolicy::Wide { focused: true },
                );
                PanelList::view(&mut carrier, f, area);
            })
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(area.x, area.y)].bg,
            Color::Reset
        );

        terminal
            .draw(|f| {
                PanelList::set_paint_policy(
                    &mut carrier,
                    PanelListPaintPolicy::WideWorkspace { focused: true },
                );
                PanelList::view(&mut carrier, f, area);
            })
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(area.x, area.y)].bg,
            Color::Reset
        );
    }

    /// The canonical-list contract, driven through the panel's object-safe
    /// surface: the same fixed-row owner remains active while its geometry
    /// changes, preserving selection and clamping on view.
    #[test]
    fn set_presentation_reanchors_the_shared_owner_across_the_transition() {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![item("a"), item("b"), item("c")]);
        carrier.select_target(&"b".to_string());
        carrier.set_scroll(0);

        // Wide paints with the selected row at the bottom of a 2-row
        // viewport, so the resolved offset re-anchors it there.
        let list_rect = Rect::new(0, 0, 20, 2);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();
        terminal
            .draw(|f| {
                PanelList::set_paint_policy(
                    &mut carrier,
                    PanelListPaintPolicy::Wide { focused: true },
                );
                PanelList::view(&mut carrier, f, list_rect);
            })
            .unwrap();
        let wide_offset = carrier.wide().current_flow_offset().expect("wide painted");
        let wide_selected = carrier.wide().current_selected_target().cloned();

        // The panel keeps the same fixed-row owner while geometry changes.
        PanelList::set_presentation(&mut carrier, Presentation::Wide, 2);
        assert_eq!(carrier.active(), Presentation::Wide);
        assert_eq!(
            carrier.selected_target().cloned(),
            wide_selected,
            "the shared owner's selection survives the transition"
        );

        let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();
        terminal
            .draw(|f| {
                PanelList::set_paint_policy(
                    &mut carrier,
                    PanelListPaintPolicy::Wide { focused: true },
                );
                PanelList::view(&mut carrier, f, list_rect);
            })
            .unwrap();
        assert_eq!(
            carrier.wide().current_flow_offset(),
            Some(wide_offset),
            "the re-anchored offset survives the panel-driven transition"
        );
        assert_eq!(
            carrier.wide().current_selected_target().cloned(),
            wide_selected,
            "the shared owner's selection is preserved across the transition"
        );
        // Back to Wide: the owner is reconfigured again, not copied.
        PanelList::set_presentation(&mut carrier, Presentation::Wide, 2);
        assert_eq!(carrier.active(), Presentation::Wide);
        assert_eq!(carrier.selected_target().cloned(), wide_selected);
    }
}
