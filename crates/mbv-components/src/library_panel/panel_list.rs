//! The `PanelList` implementation (task 5.8, design D3): one object-safe
//! implementation over the shared media-list carrier for every `Target`.
//! The panel drives the carrier through this surface — the viewport clamp
//! (`clamp_viewport`), the paint policy, the slot-rect view, and the
//! retained-geometry reads — while typed target resolution stays on the
//! carrier's own surface, so no per-destination `ListSlot` arm can grow.

use ratatui::Frame;
use ratatui::layout::Rect;
use std::hash::Hash;
use tuirealm::component::Component;

use crate::inline_search::InlineSearch;
use crate::media_list::MediaListCarrier;
use mbv_render::components::media_list::WideMediaListPaintPolicy;
use mbv_theme::Surface;

use super::content::{PanelList, PanelListPaintPolicy};

impl<Target: Clone + Eq> PanelList for MediaListCarrier<Target> {
    fn clamp_viewport(&mut self, viewport_height: usize) {
        // The carrier's own viewport clamp (no same-named inherent pair, so
        // no recursion ambiguity to dodge).
        self.clamp_viewport(viewport_height);
    }

    fn set_paint_policy(&mut self, policy: PanelListPaintPolicy) {
        match policy {
            PanelListPaintPolicy::Wide {
                focused,
                palette_focused,
            } => {
                // Every library browser list stripes again (7038e430 dropped
                // it; restored 2026-09-20), in the Grouped Music tree's style:
                // the stripe is the library column's own fill for the
                // palette bit — focused `SURFACE_FOCUSED`, the same tone the
                // music tree's rows alternate with, resting the app backdrop.
                // The policy keys the stripe tone on the palette bit, so a
                // mini view panel keeps its selected-row bar on the resting
                // stripe.
                self.wide_mut().set_paint_policy(
                    WideMediaListPaintPolicy::new(focused)
                        .with_palette_focus(palette_focused)
                        .with_zebra(Surface::LibraryColumn),
                );
            }
            PanelListPaintPolicy::WideWorkspace { focused } => {
                // Focused, the rows stripe against the focused box fill (the
                // `WorkspaceStripe` surface on the `MainContentBox` fill,
                // painted by the workspace box); unfocused the policy's
                // palette bit rests and resolves the same stripe's resting
                // fill. The focused selection paints the Iris bar.
                self.wide_mut().set_paint_policy(
                    WideMediaListPaintPolicy::for_library_workspace(focused)
                        .with_zebra(Surface::WorkspaceStripe),
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
}

/// The complete shared tree owner uses the same erased `PanelList` surface as
/// the flat carrier. The panel supplies only focus and claim/content geometry;
/// all tree painting and retained target geometry stay behind
/// `TreeBrowser::Component::view`.
impl<Target: Clone + Eq + Hash> PanelList for crate::list::tree_browser::TreeBrowser<Target> {
    fn clamp_viewport(&mut self, viewport_height: usize) {
        self.clamp_viewport_to(viewport_height);
    }

    fn set_paint_policy(&mut self, policy: PanelListPaintPolicy) {
        let (focused, palette_focused) = match policy {
            PanelListPaintPolicy::Wide {
                focused,
                palette_focused,
            } => (focused, palette_focused),
            PanelListPaintPolicy::WideWorkspace { focused } => (focused, focused),
        };
        self.set_paint_focus(focused, palette_focused);
        self.invalidate_paint();
    }

    fn view(&mut self, frame: &mut Frame, rect: Rect) {
        Component::view(self, frame, rect);
    }

    fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        self.set_geometry(claim_rect, content_rect);
    }

    fn selected_row_rect(&self) -> Option<Rect> {
        self.selected_row_rect()
    }

    fn search_bar(&self) -> Option<(String, bool)> {
        self.search_bar()
    }
}

/// The Inline Search session's `PanelList` surface (design.md D3, task 2.1):
/// every method forwards one line to the session's embedded carrier, so the
/// panel's `ListSlot::Search` arm drives the exact same fixed-row presentation
/// `ListSlot::Media` does — the search control keeps only its query, pool,
/// scoring, and debounce.
impl PanelList for InlineSearch {
    fn clamp_viewport(&mut self, viewport_height: usize) {
        PanelList::clamp_viewport(self.results_mut(), viewport_height);
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
}

#[cfg(test)]
mod panel_list_tests {
    use super::*;
    use mbv_render::components::media_list::{MediaKind, MediaListRow, MediaSemanticState};
    use mbv_theme as palette;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;

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

    #[derive(Clone, Debug, Eq, Hash, PartialEq)]
    enum TreeTarget {
        Root,
        Child,
    }

    #[test]
    fn shared_tree_uses_the_media_slot_panel_surface_for_paint_and_hits() {
        use crate::list::tree_browser::{TreeBrowser, TreeMarkPolicy, TreeNode};

        let mut tree = TreeBrowser::new();
        tree.reconcile([
            TreeNode::new(
                TreeTarget::Root,
                None,
                "Root",
                "root",
                MediaSemanticState::Ordinary,
                TreeMarkPolicy::Direct,
            ),
            TreeNode::new(
                TreeTarget::Child,
                Some(TreeTarget::Root),
                "Child",
                "child",
                MediaSemanticState::Ordinary,
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();
        let claim = Rect::new(1, 0, 18, 2);
        let content = Rect::new(3, 0, 14, 2);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();
        terminal
            .draw(|frame| {
                PanelList::set_paint_policy(
                    &mut tree,
                    PanelListPaintPolicy::Wide {
                        focused: true,
                        palette_focused: true,
                    },
                );
                PanelList::set_geometry(&mut tree, claim, content);
                PanelList::view(&mut tree, frame, claim);
            })
            .unwrap();

        assert!(tree.claims_current_point(Position::new(2, 0)));
        assert_eq!(
            tree.resolve_current_point(Position::new(2, 0)),
            Some(&TreeTarget::Root)
        );
    }

    /// The selected-row bar is intentionally identical across the browser
    /// arms; arm-specific stripe colours are owned by the Render Component
    /// regressions in `crates/mbv-render/src/components/media_list.rs` and its
    /// Wide-arm tests. Every arm uses the canonical Iris selected-row role.
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
                    PanelListPaintPolicy::Wide {
                        focused: true,
                        palette_focused: true,
                    },
                );
                PanelList::view(&mut carrier, f, area);
            })
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(area.x, area.y)].bg,
            palette::surface_colors(palette::Surface::SelectedRow, false).fill
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
            palette::surface_colors(palette::Surface::SelectedRow, false).fill,
            "the focused Workspace uses the canonical Iris selected-row bar"
        );
    }

    /// The Workspace's zebra stripe follows the box fill: unfocused rests at
    /// the fixed resting content Storm; focused the rows stripe the focused
    /// `WorkspaceStripe` tone against the focused box fill. The stripe
    /// painter itself is owned by the media-list regressions.
    #[test]
    fn workspace_stripes_zebra_rows_with_the_resting_storm() {
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![item("a"), item("b")]);
        let area = Rect::new(0, 0, 20, 2);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();

        for (focused, expected) in [
            (
                true,
                palette::surface_colors(palette::Surface::WorkspaceStripe, true).fill,
            ),
            (
                false,
                palette::surface_colors(palette::Surface::WorkspaceStripe, false).fill,
            ),
        ] {
            terminal
                .draw(|f| {
                    PanelList::set_paint_policy(
                        &mut carrier,
                        PanelListPaintPolicy::WideWorkspace { focused },
                    );
                    PanelList::view(&mut carrier, f, area);
                })
                .unwrap();
            let buf = terminal.backend().buffer();
            // The ungrouped alternation opens on the box fill, so row 1
            // carries the stripe; the 2-column quiet indent keeps the parent
            // background, so the stripe is read from the title column.
            assert_eq!(
                buf[(area.x + 2, area.y + 1)].bg,
                expected,
                "striped row, focused={focused}"
            );
        }
    }

    /// The browser-list stripe pair, pinned to the Grouped Music tree's own
    /// alternation: the second row carries the library column's fill for the
    /// paint's focus bit, so a focused browser list alternates `SURFACE_FOCUSED`
    /// against the `LibraryPanel` box fill exactly as the music rows do.
    #[test]
    fn browser_stripes_zebra_rows_like_the_music_tree() {
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![item("a"), item("b")]);
        let area = Rect::new(0, 0, 20, 2);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();

        for focused in [true, false] {
            terminal
                .draw(|f| {
                    PanelList::set_paint_policy(
                        &mut carrier,
                        PanelListPaintPolicy::Wide {
                            focused,
                            palette_focused: focused,
                        },
                    );
                    PanelList::view(&mut carrier, f, area);
                })
                .unwrap();
            let buf = terminal.backend().buffer();
            // The ungrouped alternation opens on the box fill, so row 1
            // carries the stripe; the 2-column quiet indent keeps the parent
            // background, so the stripe is read from the title column.
            assert_eq!(
                buf[(area.x + 2, area.y + 1)].bg,
                palette::surface_colors(palette::Surface::LibraryColumn, focused).fill,
                "striped row, focused={focused}"
            );
            assert_ne!(
                buf[(area.x + 2, area.y + 1)].bg,
                palette::surface_colors(palette::Surface::LibraryPanel, focused).fill,
                "the stripe must be visible against the box fill, focused={focused}"
            );
        }
    }

    /// The mini-view split: a focused browser list with a resting palette
    /// keeps the canonical Iris bar on its selected row while the zebra
    /// stripe rests at the unfocused tone. The selected row is the exception
    /// to the palette suppression, so the single displayed panel keeps a
    /// visible cursor.
    #[test]
    fn mini_view_keeps_the_cursor_bar_on_the_resting_stripe() {
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![item("a"), item("b")]);
        let area = Rect::new(0, 0, 20, 2);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();

        terminal
            .draw(|f| {
                PanelList::set_paint_policy(
                    &mut carrier,
                    PanelListPaintPolicy::Wide {
                        focused: true,
                        palette_focused: false,
                    },
                );
                PanelList::view(&mut carrier, f, area);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let resting = palette::surface_colors(palette::Surface::LibraryColumn, false).fill;
        assert_ne!(
            resting,
            palette::surface_colors(palette::Surface::LibraryColumn, true).fill,
            "the LibraryColumn focus pair must differ for this proof to bind"
        );
        assert_eq!(
            buf[(area.x, area.y)].bg,
            palette::surface_colors(palette::Surface::SelectedRow, false).fill,
            "the focused row keeps its bar on the resting palette"
        );
        assert_eq!(
            buf[(area.x + 2, area.y + 1)].bg,
            resting,
            "the stripe rests while the cursor stays"
        );
    }

    /// The shared tree stripes with the browser pane's library pair — the
    /// same `LibraryColumn` stripe over the `LibraryPanel` box fill the flat
    /// carrier resolves — in both focus states: group headings and the spacer
    /// between groups keep the box fill, and each group's first member opens
    /// the alternation on the stripe. The mini-view split (focused cursor on
    /// a resting palette) keeps the selected row's bar on the resting stripe.
    #[test]
    fn shared_tree_stripes_with_the_library_browser_pair() {
        use crate::list::tree_browser::{
            TreeBrowser, TreeEntry, TreeMarkPolicy, TreeNode, TreeOperation,
        };

        let node = |target: TreeTarget, title: &str| {
            TreeNode::new(
                target,
                None,
                title,
                title,
                MediaSemanticState::Ordinary,
                TreeMarkPolicy::Direct,
            )
        };
        let mut tree = TreeBrowser::new();
        tree.reconcile(vec![
            TreeEntry::Heading("A".into()),
            TreeEntry::Node(node(TreeTarget::Root, "first")),
            TreeEntry::Spacer,
            TreeEntry::Heading("B".into()),
            TreeEntry::Node(node(TreeTarget::Child, "second")),
        ])
        .unwrap();
        tree.apply(TreeOperation::Select(TreeTarget::Child));
        let claim = Rect::new(1, 0, 18, 5);
        let content = Rect::new(3, 0, 14, 5);
        let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
        // The title column: content's x plus the 2-column quiet indent the
        // stripe is read from (the indent keeps the parent background).
        let x = content.x + 2;
        for (focused, palette_focused) in [(true, true), (false, false), (true, false)] {
            terminal
                .draw(|frame| {
                    PanelList::set_paint_policy(
                        &mut tree,
                        PanelListPaintPolicy::Wide {
                            focused,
                            palette_focused,
                        },
                    );
                    PanelList::set_geometry(&mut tree, claim, content);
                    PanelList::view(&mut tree, frame, claim);
                })
                .unwrap();
            let buf = terminal.backend().buffer();
            let box_fill =
                palette::surface_colors(palette::Surface::LibraryPanel, palette_focused).fill;
            let stripe =
                palette::surface_colors(palette::Surface::LibraryColumn, palette_focused).fill;
            let case = format!("focused={focused}, palette_focused={palette_focused}");
            assert_eq!(
                buf[(x, 0)].bg,
                box_fill,
                "the group heading keeps the box fill, {case}"
            );
            assert_eq!(
                buf[(x, 2)].bg,
                box_fill,
                "the spacer between groups keeps the box fill, {case}"
            );
            assert_eq!(
                buf[(x, 3)].bg,
                box_fill,
                "the next group's heading keeps the box fill, {case}"
            );
            assert_eq!(
                buf[(x, 1)].bg,
                stripe,
                "the group's first member opens on the stripe, {case}"
            );
            assert_ne!(
                buf[(x, 1)].bg,
                box_fill,
                "the stripe must be visible against the box fill, {case}"
            );
            if focused {
                // The selected Child row is the fourth content row; its bar
                // reaches the claim rect's border in every focused state,
                // including on the resting mini-view palette.
                assert_eq!(
                    buf[(claim.x, 4)].bg,
                    palette::surface_colors(palette::Surface::SelectedRow, false).fill,
                    "the selected row keeps its bar, {case}"
                );
            }
        }
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
                    PanelListPaintPolicy::Wide {
                        focused: true,
                        palette_focused: true,
                    },
                );
                PanelList::view(&mut carrier, f, list_rect);
            })
            .unwrap();
        let wide_offset = carrier.scroll();
        let wide_selected = carrier.selected_target().cloned();

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
                    PanelListPaintPolicy::Wide {
                        focused: true,
                        palette_focused: true,
                    },
                );
                PanelList::view(&mut carrier, f, list_rect);
            })
            .unwrap();
        assert_eq!(
            Some(carrier.scroll()),
            Some(wide_offset),
            "the re-anchored offset survives the panel-driven transition"
        );
        assert_eq!(
            carrier.selected_target().cloned(),
            wide_selected,
            "the shared owner's selection is preserved across the transition"
        );
        // Back to Wide: the owner is reconfigured again, not copied.
        PanelList::clamp_viewport(&mut carrier, 2);
        assert_eq!(carrier.selected_target().cloned(), wide_selected);
    }
}
