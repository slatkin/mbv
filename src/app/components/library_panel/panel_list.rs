//! The `PanelList` implementation (task 5.8, design D3): one object-safe
//! implementation over the shared media-list carrier for every `Target`.
//! The panel drives the carrier through this surface — the viewport clamp
//! (`clamp_viewport`), the paint policy, the slot-rect view, and the
//! retained-geometry reads — while typed target resolution stays on the
//! carrier's own surface, so no per-destination `ListSlot` arm can grow.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::component::Component;

use crate::app::components::inline_search::InlineSearch;
use crate::app::components::media_list::{MediaListCarrier, WideMediaListPaintPolicy, ZebraStripe};
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
    fn clamp_viewport(&mut self, viewport_height: usize) {
        // The carrier's own viewport clamp (no same-named inherent pair, so
        // no recursion ambiguity to dodge).
        self.clamp_viewport(viewport_height);
    }

    fn clear_selection(&mut self) {
        self.clear_owner_selection();
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

/// The Inline Search session's PanelList surface (design.md D3, task 2.1):
/// every method forwards one line to the session's embedded carrier, so the
/// panel's `ListSlot::Search` arm drives the exact same fixed-row presentation
/// `ListSlot::Media` does — the search control keeps only its query, pool,
/// scoring, and debounce.
impl PanelList for InlineSearch {
    fn clamp_viewport(&mut self, viewport_height: usize) {
        PanelList::clamp_viewport(self.results_mut(), viewport_height);
    }

    fn clear_selection(&mut self) {
        PanelList::clear_selection(self.results_mut());
    }

    fn set_paint_policy(&mut self, policy: PanelListPaintPolicy) {
        PanelList::set_paint_policy(self.results_mut(), policy);
    }

    fn view(&mut self, frame: &mut Frame, rect: Rect) {
        PanelList::view(self.results_mut(), frame, rect);
    }

    fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        PanelList::set_geometry(self.results_mut(), claim_rect, content_rect);
    }

    fn selected_row_rect(&self) -> Option<Rect> {
        PanelList::selected_row_rect(self.results())
    }

    fn claims_point(&self, point: Position) -> bool {
        PanelList::claims_point(self.results(), point)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod panel_list_tests {
    use super::*;
    use crate::app::components::media_list::{MediaKind, MediaListRow, MediaSemanticState};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
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

    /// The selected-row bar is intentionally identical across both Wide
    /// arms; arm-specific stripe colours are owned by the Render Component
    /// regressions in `src/app/render/components/media_list.rs` and its
    /// Wide-arm tests.
    #[test]
    fn wide_selected_rows_paint_the_bar_in_both_slots() {
        let mut carrier = MediaListCarrier::new();
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
            palette::SELECTED_ROW_BG.color()
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
            palette::SELECTED_ROW_BG.color()
        );
    }

    /// The canonical-list contract, driven through the panel's object-safe
    /// surface: the same fixed-row owner remains active while its geometry
    /// changes, preserving selection and clamping on view.
    #[test]
    fn clamp_viewport_preserves_the_shared_owner_across_the_transition() {
        let mut carrier = MediaListCarrier::new();
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
        PanelList::clamp_viewport(&mut carrier, 2);
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
        PanelList::clamp_viewport(&mut carrier, 2);
        assert_eq!(carrier.selected_target().cloned(), wide_selected);
    }
}
