use ratatui::layout::Position;
use tuirealm::event::MouseEvent;

use super::{Msg, ShellRequest, TerminalObserverEvent, TvHit, TvWorkspaceComponent};
use crate::app::components::media_list::{Presentation, RowLocalInput};
use crate::app::components::mouse::gesture::MouseGesture;

impl TvWorkspaceComponent {
    /// Wide pane-based mouse handling (design.md D6): gesture recognition
    /// comes from the private `MouseGestureState`. Season pills resolve
    /// through `tv_chrome`; both panes' row identity comes from their
    /// embedded control. The component emits a semantic `Msg` with a
    /// resolved `TvHit` -- never raw coordinates -- except the context-menu
    /// anchor (design.md D4). A left click moves the component's local pane
    /// + pane cursor; a right click never does.
    pub(super) fn handle_mouse_wide(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                // The series rail is the only scrollable TV surface. Its
                // canonical control claims the painted region.
                if !self.carrier.claims_current_point(at) {
                    return None;
                }
                self.move_rows(delta);
                // Return a framework-visible claim after mutating local state;
                // dropping the message would let the framework's mutation be
                // discarded by the mouse fold.
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::Click(at) => {
                let hit = self.resolve_hit(at)?;
                self.apply_pane_click(hit.clone(), at);
                Some(Msg::Shell(ShellRequest::TvHitClick { hit }))
            }
            MouseGesture::DoubleClick(at) => {
                let hit = self.resolve_hit(at)?;
                self.apply_pane_click(hit.clone(), at);
                Some(Msg::Shell(ShellRequest::TvHitDoubleClick { hit }))
            }
            MouseGesture::RightClick(at) => {
                let hit = self.resolve_hit(at)?;
                Some(Msg::Shell(ShellRequest::TvHitContextMenu {
                    hit,
                    anchor: (mouse.column, mouse.row),
                }))
            }
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => None,
        }
    }

    /// Narrow flat-list mouse handling (mirrors `BrowserComponent::
    /// handle_mouse` before the merge, task 8.1): row identity and letter
    /// pills both resolve through `TvHit` and `ShellRequest::TvHit*` --
    /// `handle_mouse_single_click_tv`'s new `LetterPill` arm -- so the shell
    /// reprojects the one TV owner exactly as the Wide clicks already do.
    pub(super) fn handle_mouse_narrow(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                let claimed = self.carrier.claims_current_point(at)
                    || (self.carrier.active() == Presentation::Inline
                        && self.layout.inline_hero_area.contains(at));
                if !claimed {
                    return None;
                }
                self.move_cursor_delta_narrow(delta);
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::Click(at) => {
                let hit = self.resolve_hit_narrow(at)?;
                self.apply_narrow_click(&hit, at);
                Some(Msg::Shell(ShellRequest::TvHitClick { hit }))
            }
            MouseGesture::DoubleClick(at) => {
                let hit = self.resolve_hit_narrow(at)?;
                self.apply_narrow_click(&hit, at);
                Some(Msg::Shell(ShellRequest::TvHitDoubleClick { hit }))
            }
            MouseGesture::RightClick(at) => {
                // Letter pills never raise a context menu (mirrors
                // `BrowserComponent`, which never checked `pill_regions` on
                // `RightClick`).
                if !self.claim_narrow_point(at) {
                    return None;
                }
                let target = self.carrier.selected_target().cloned()?;
                Some(Msg::Shell(ShellRequest::TvHitContextMenu {
                    hit: TvHit::SeriesRow(target),
                    anchor: (mouse.column, mouse.row),
                }))
            }
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => None,
        }
    }

    /// Resolve a Narrow position to a letter pill or a series row, from the
    /// component's own painted geometry.
    fn resolve_hit_narrow(&self, position: Position) -> Option<TvHit> {
        if let Some(&pill) = self.pill_regions.resolve(position) {
            return Some(TvHit::LetterPill(pill));
        }
        if self.carrier.claims_current_point(position) {
            return self
                .carrier
                .resolve_current_point(position)
                .cloned()
                .map(TvHit::SeriesRow);
        }
        None
    }

    /// Move the shared owner's selection to a clicked Narrow row; a pill
    /// click never moves the row selection (the shell's letter-pill effect
    /// re-scopes the level and re-anchors the cursor on its own).
    fn apply_narrow_click(&mut self, hit: &TvHit, at: Position) {
        if let TvHit::SeriesRow(target) = hit {
            self.carrier
                .delegate(RowLocalInput::Click(at), Some(target.clone()));
        }
    }

    /// If `at` lands inside the painted list or inline-hero region, move the
    /// selection to the row under it (mirrors `BrowserComponent::
    /// claim_list_point`).
    fn claim_narrow_point(&mut self, at: Position) -> bool {
        if !(self.layout.left_area.contains(at) || self.layout.inline_hero_area.contains(at)) {
            return false;
        }
        let Some(target) = self.carrier.resolve_current_point(at).cloned() else {
            return false;
        };
        self.carrier.select_target(&target)
    }

    /// Move by one selectable item in the shared owner's row order (mirrors
    /// `BrowserComponent::move_cursor_delta`).
    fn move_cursor_delta_narrow(&mut self, delta: i64) {
        self.carrier.delegate(RowLocalInput::Move(delta), None);
        if self.carrier.selected_target().is_some() {
            self.carrier.sync_viewport(self.painted_viewport_height());
        }
    }
}
