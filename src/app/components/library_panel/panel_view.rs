use super::*;

use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use crate::app::components::msg::Msg;
use crate::app::components::UserEvent;

impl Default for LibraryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LibraryPanel {
    fn view(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // Each frame's retained geometry is exactly what that frame painted
        // (ADR 0024): the reset also drops the other breakpoint's geometry, so
        // a Wide→Narrow resize leaves neither the vanished split's gap armed
        // nor the stale Wide list rect claiming clicks.
        self.reset_frame();
        // The overlay hint bar's chips are derived centrally from the active
        // destination key (a content change, not a painter arm); per-screen
        // hints land here screen-by-screen (Home first). Read before the
        // mutable content borrow below so the two never overlap.
        let hints: &[&str] = if matches!(self.owners.active_key(), Some(LibraryKey::Home)) {
            &["ENTER:Play", "ESC:Back"]
        } else {
            &["ESC:Back"]
        };
        let Some(owner) = self.owners.active_mut() else {
            return;
        };
        let overview_scroll = owner.hero_scroll_offset();
        let mut content = owner.content();
        self.painted_link_urls = content
            .hero
            .as_ref()
            .map(|hero| {
                hero.facts
                    .links
                    .iter()
                    .map(|link| link.url.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut hits = std::mem::take(&mut self.hits);
        let mut windows = self.pill_windows;
        // The overlay's own area. In non-Wide geometry it is the browser's
        // inset list box below the reserved pill rows, so the pill bar and
        // its spacer band stay outside both the overlay frame and its dim
        // backdrop; the Wide geometry keeps the whole panel as before.
        let mut overlay_area = area;
        // One breakpoint predicate (design D4): `wide_hero_fits` stays the
        // single Wide/Narrow choice; the panel clamps the list's viewport
        // through the list's `clamp_viewport` inside each skeleton.
        if wide_hero_fits(area) {
            if let Some(geometry) = render_wide_skeleton(
                frame,
                area,
                &mut content,
                self.focused,
                self.list_pane_width,
                overview_scroll,
                self.hovered_selector,
                self.hovered_link,
                &mut hits,
                &mut windows,
                self.terminal_height,
            ) {
                // The split gesture owns the gap columns it painted: the
                // gutter between the hero and browser panes, resolved
                // against the panel's own content area.
                let gap = ratatui::layout::Rect {
                    x: geometry.hero.right(),
                    y: geometry.hero.y,
                    width: geometry.browser.x.saturating_sub(geometry.hero.right()),
                    height: geometry.hero.height,
                };
                self.split = (gap.width > 0 && gap.height > 0).then_some(SplitGeometry {
                    gap,
                    pane_origin_x: area.right(),
                    content_width: area.width,
                    width: geometry.browser.width,
                });
                self.wide_geometry = Some(geometry);
            }
        } else {
            let geometry = render_narrow_skeleton(
                frame,
                area,
                &mut content,
                self.focused,
                self.hovered_selector,
                &mut hits,
                &mut windows,
            );
            overlay_area = geometry.list_panel;
            self.narrow_geometry = Some(geometry);
        }
        // Paint the Library-local overlay after the ordinary skeleton. The
        // shared Hero path therefore remains the sole content painter.
        if self.hero_overlay_open {
            if let Some(overlay_rect) =
                crate::app::render::arrangements::library::library_hero_overlay(overlay_area)
            {
                let inner = crate::app::render::components::library_hero_overlay::paint_library_hero_overlay(frame, overlay_area, overlay_rect, hints);
                if let Some(hero) = content.hero.as_mut() {
                    let composition = super::super::hero_composition::paint_library_hero_content(
                        frame,
                        inner,
                        hero,
                        overview_scroll,
                        self.hovered_link,
                        &mut hits.links,
                        &mut hits.workspace_selector,
                        &mut windows.workspace_selector,
                        // The overlay's sheet is the pane fill shared content
                        // repaints behind the Hero header and overview.
                        crate::app::render::components::library_hero_overlay::OVERLAY_SHEET_SURFACE,
                        self.terminal_height,
                    );
                    self.overlay_geometry = Some(super::OverlayGeometry {
                        pane: overlay_area,
                        frame: overlay_rect,
                        hero: composition,
                    });
                } else {
                    // A target may arrive one projection before its Hero
                    // snapshot while provider detail is loading. Keep the
                    // visible overlay/frame and its hit boundary alive rather
                    // than silently falling back to the covered browser.
                    self.overlay_geometry = Some(super::OverlayGeometry {
                        pane: overlay_area,
                        frame: overlay_rect,
                        hero: super::super::hero_composition::HeroCompositionGeometry {
                            workspace: None,
                            hero_image: None,
                            overview_box: None,
                            overview_content_length: 0,
                            overview_viewport: 0,
                        },
                    });
                }
            }
        }
        // The projected hero image's reserved box (task 5.10, design D9): the
        // shell paints the protocol into it right after view returns.
        self.image_paint = if self.hero_overlay_open {
            // The overlay owns the covered Hero surface while open; never
            // project the underlying browser image into its dimmed frame.
            self.overlay_geometry
                .as_ref()
                .and_then(|geometry| geometry.hero.hero_image.clone())
        } else {
            self.wide_geometry
                .as_ref()
                .and_then(|geometry| geometry.hero_image.clone())
        };
        self.hits = hits;
        self.pill_windows = windows;
        self.painted_area = Some(area);
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for LibraryPanel {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            // The panel's minimal keyboard forwarding (task 5.11, design D3):
            // the focused panel hands the already-routed chord to the active
            // owner, which keeps its local key interpretation exactly as a
            // mounted destination did. The router keeps precedence — this is
            // not a second resolution site, only delivery.
            Event::Keyboard(key) if self.focused => {
                if key.modifiers.is_empty()
                    && key.code == tuirealm::event::Key::Esc
                    && self.hero_overlay_open
                {
                    self.dismiss_hero_overlay();
                    return LeafKeyResult::Consumed(None).into_option();
                }
                if key.modifiers.is_empty()
                    && key.code == tuirealm::event::Key::Enter
                    && !self.hero_overlay_open
                    && self.narrow_geometry.is_some()
                {
                    if let Some(message) = self.open_hero_from_browser(None) {
                        return Some(message);
                    }
                }
                let result = self
                    .owners
                    .active_mut()
                    .map(|owner| owner.on_key_result(key))
                    .unwrap_or(LeafKeyResult::Unhandled);
                let hero_overlay_resolvable = self.hero_overlay_open
                    && self
                        .owners
                        .active_mut()
                        .is_some_and(|owner| owner.hero_overlay_available());
                if hero_overlay_resolvable && matches!(result, LeafKeyResult::Unhandled) {
                    return Some(Msg::TerminalEvent(
                        crate::app::components::msg::TerminalObserverEvent::KeyClaimed,
                    ));
                }
                result.into_option()
            }
            _ => None,
        }
    }
}
