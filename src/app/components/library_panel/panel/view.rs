use super::{
    render_narrow_skeleton, render_wide_skeleton, wide_hero_fits, LeafKeyResult, LibraryKey,
    LibraryPanel, SkeletonHits, SkeletonPillWindows,
};

use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use crate::app::components::library_panel::LibraryKind;
use crate::app::components::msg::Msg;
use crate::app::components::UserEvent;

impl Default for LibraryPanel {
    fn default() -> Self {
        Self::new()
    }
}

struct SkeletonPaintState<'a> {
    focused: bool,
    list_pane_width: Option<u16>,
    hovered_selector: Option<usize>,
    hovered_link: Option<usize>,
    terminal_height: u16,
    split: &'a mut Option<super::SplitGeometry>,
    wide_geometry: &'a mut Option<super::WideSkeletonGeometry>,
    narrow_geometry: &'a mut Option<super::WideSkeletonGeometry>,
    hit_regions: &'a mut SkeletonHits,
    windows: &'a mut SkeletonPillWindows,
}

struct HeroOverlayPaintState<'a> {
    hovered_link: Option<usize>,
    terminal_height: u16,
    open: bool,
    geometry: &'a mut Option<super::OverlayGeometry>,
    hit_regions: &'a mut SkeletonHits,
    windows: &'a mut SkeletonPillWindows,
}

fn paint_skeleton(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    content: &mut super::super::content::LibraryPanelContent<'_>,
    overview_scroll: usize,
    show_hero_pane: bool,
    state: &mut SkeletonPaintState<'_>,
) -> ratatui::layout::Rect {
    if wide_hero_fits(area) {
        if let Some(geometry) = render_wide_skeleton(
            frame,
            area,
            content,
            state.focused,
            state.list_pane_width,
            overview_scroll,
            state.hovered_selector,
            state.hovered_link,
            state.hit_regions,
            state.windows,
            state.terminal_height,
            show_hero_pane,
        ) {
            let gap = ratatui::layout::Rect {
                x: geometry.hero.right(),
                y: geometry.hero.y,
                width: geometry.browser.x.saturating_sub(geometry.hero.right()),
                height: geometry.hero.height,
            };
            *state.split = (gap.width > 0 && gap.height > 0).then_some(super::SplitGeometry {
                gap,
                pane_origin_x: area.right(),
                content_width: area.width,
                width: geometry.browser.width,
            });
            *state.wide_geometry = Some(geometry);
        }
        area
    } else {
        let geometry = render_narrow_skeleton(
            frame,
            area,
            content,
            state.focused,
            state.hovered_selector,
            state.hit_regions,
            state.windows,
        );
        let overlay_area = geometry.list_panel;
        *state.narrow_geometry = Some(geometry);
        overlay_area
    }
}

fn paint_library_hero_overlay(
    frame: &mut Frame,
    overlay_area: ratatui::layout::Rect,
    hints: &[&str],
    overview_scroll: usize,
    content: &mut super::super::content::LibraryPanelContent<'_>,
    state: &mut HeroOverlayPaintState<'_>,
) {
    if !state.open {
        return;
    }
    let Some(overlay_rect) =
        crate::app::render::arrangements::library::library_hero_overlay(overlay_area)
    else {
        return;
    };
    let inner = crate::app::render::components::library_hero_overlay::paint_library_hero_overlay(
        frame,
        overlay_area,
        overlay_rect,
        hints,
    );
    let hero = content.hero.as_mut().map(|hero| {
        super::super::hero_composition::paint_library_hero_content(
            frame,
            inner,
            hero,
            overview_scroll,
            state.hovered_link,
            &mut state.hit_regions.links,
            &mut state.hit_regions.workspace_selector,
            &mut state.windows.workspace_selector,
            crate::app::render::components::library_hero_overlay::OVERLAY_SHEET_SURFACE,
            state.terminal_height,
        )
    });
    let composition = hero.unwrap_or(super::super::hero_composition::HeroCompositionGeometry {
        workspace: None,
        hero_image: None,
        overview_box: None,
        overview_content_length: 0,
        overview_viewport: 0,
    });
    *state.geometry = Some(super::OverlayGeometry {
        #[cfg(test)]
        pane: overlay_area,
        frame: overlay_rect,
        hero: composition,
    });
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
        let flat_tv_list = matches!(
            self.owners.active_key(),
            Some(LibraryKey::Service {
                kind: LibraryKind::TvShows,
                ..
            })
        );
        let Some(owner) = self.owners.active_mut() else {
            self.reset_split_gesture();
            return;
        };
        let overview_scroll = owner.hero_scroll_offset();
        // Flat Latest/Upcoming TV rows are leaves, not a hero-bearing browser.
        // The Library panel owns this Wide skeleton policy; other destinations
        // and TV series/workspace modes retain the shared split.
        let show_hero_pane = !flat_tv_list || owner.browser_rows_are_hero_bearing();
        let mut content = owner.content();
        #[cfg(test)]
        {
            self.projected_selector_markers = content
                .selector
                .as_ref()
                .map(|selector| selector.markers.clone())
                .unwrap_or_default();
        };
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
        let mut hit_regions = std::mem::take(&mut self.hits);
        let mut windows = self.pill_windows;
        let overlay_area = paint_skeleton(
            frame,
            area,
            &mut content,
            overview_scroll,
            show_hero_pane,
            &mut SkeletonPaintState {
                focused: self.focused,
                list_pane_width: self.list_pane_width,
                hovered_selector: self.hovered_selector,
                hovered_link: self.hovered_link,
                terminal_height: self.terminal_height,
                split: &mut self.split,
                wide_geometry: &mut self.wide_geometry,
                narrow_geometry: &mut self.narrow_geometry,
                hit_regions: &mut hit_regions,
                windows: &mut windows,
            },
        );
        // Paint the Library-local overlay after the ordinary skeleton. The
        // shared Hero path therefore remains the sole content painter.
        paint_library_hero_overlay(
            frame,
            overlay_area,
            hints,
            overview_scroll,
            &mut content,
            &mut HeroOverlayPaintState {
                hovered_link: self.hovered_link,
                terminal_height: self.terminal_height,
                open: self.hero_overlay_open,
                geometry: &mut self.overlay_geometry,
                hit_regions: &mut hit_regions,
                windows: &mut windows,
            },
        );
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
        // A typed request the owner resolved from the frame it just painted
        // (task 6.5: the Grouped Music tree's neighbour artwork window) rides
        // the panel's deferred-message seam and is dispatched after this
        // paint. A pending deferred message is never clobbered; the owner
        // re-resolves the same window on the next painted frame.
        if self.deferred_msg.is_none() {
            if let Some(message) = self
                .owners
                .active_mut()
                .and_then(|owner| owner.post_paint_message())
            {
                self.deferred_msg = Some(message);
            }
        }
        self.hits = hit_regions;
        self.pill_windows = windows;
        self.painted_area = Some(area);
        if self.split.is_none() {
            self.reset_split_gesture();
        }
    }

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
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
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
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
                    && self
                        .owners
                        .active_mut()
                        .is_some_and(|owner| owner.hero_overlay_enter_available())
                {
                    if let Some(message) = self.open_hero_from_browser(None) {
                        return Some(message);
                    }
                }
                let result = self
                    .owners
                    .active_mut()
                    .map_or(LeafKeyResult::Unhandled, |owner| owner.on_key_result(key));
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
