//! Pointer-event interpretation against the last painted frame's retained slot geometry, resolving pill, list, hero, and split-drag gestures into typed Msgs.

use super::*;

impl LibraryPanel {
    /// The list slot's row-flow rect from the last painted frame, when one
    /// painted.
    pub(super) fn list_rect(&self) -> Option<ratatui::layout::Rect> {
        self.wide_geometry
            .as_ref()
            .map(|geometry| geometry.list_area)
            .or(self
                .narrow_geometry
                .as_ref()
                .map(|geometry| geometry.list_area))
    }

    /// The hero pane's rect from the last painted Wide frame, when one
    /// painted.
    fn hero_pane_rect(&self) -> Option<ratatui::layout::Rect> {
        self.wide_geometry.as_ref().map(|geometry| geometry.hero)
    }

    pub(super) fn open_hero_from_browser(&mut self, at: Option<Position>) -> Option<Msg> {
        let click_message = at.and_then(|at| {
            self.slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(at)))
        });
        // Resolve the gate after the pointer click has moved the canonical
        // browser selection to its post-click target. If the gate loses, the
        // click message must survive: the component already mutated before
        // the parent made this decision.
        if !self.open_hero_overlay_for_active() {
            return click_message;
        }
        let claim = if at.is_some() {
            TerminalObserverEvent::MouseClaimed
        } else {
            TerminalObserverEvent::KeyClaimed
        };
        Some(Msg::TerminalEvent(claim))
    }

    fn slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        if let LibrarySlotEvent::HeroPane(MediaListSurfaceInput::Wheel { at, delta }) = event {
            if let Some(geometry) = self.wide_geometry.as_ref() {
                if let Some(rect) = geometry.overview_box {
                    if rect.contains(at)
                        && geometry.overview_content_length > geometry.overview_viewport
                    {
                        let max = geometry.overview_content_length - geometry.overview_viewport;
                        return self.owners.active_mut().map(|owner| {
                            owner.hero_scroll(delta as i16, max);
                            Msg::TerminalEvent(
                                crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
                            )
                        });
                    }
                }
            }
            // Otherwise preserve the owner's existing HeroPane behavior.
        }
        let is_wheel = matches!(
            event,
            LibrarySlotEvent::List(MediaListSurfaceInput::Wheel { .. })
        );
        let result = self
            .owners
            .active_mut()
            .and_then(|owner| owner.on_slot_event(event));
        if is_wheel {
            if let (Some(key @ LibraryKey::Service { .. }), Some((index, scroll))) = (
                self.owners.active_key().cloned(),
                self.owners
                    .active_mut()
                    .and_then(|owner| owner.scroll_position()),
            ) {
                self.deferred_msg = Some(Msg::Shell(Box::new(ShellRequest::LibraryScroll {
                    key,
                    index,
                    scroll,
                })));
            }
        }
        result
    }

    /// The split drag's resolved message: live width changes during the
    /// gesture, followed by one persistence request at drag end when a width
    /// actually changed.
    pub(super) fn reset_split_gesture(&mut self) {
        self.split_gestures = MouseGestureState::new();
        self.split_changed = false;
    }

    /// Reset the split gesture only if the event is a press/release
    /// boundary, so an abandoned drag can't outlive its owning gesture.
    fn reset_boundary_gesture(&mut self, gesture_boundary: bool) {
        if gesture_boundary {
            self.reset_split_gesture();
        }
    }

    fn split_gesture_msg(
        &mut self,
        gesture: MouseGesture,
        at: ratatui::layout::Position,
    ) -> Option<Msg> {
        let Some(split) = self.split.as_ref() else {
            self.reset_split_gesture();
            return None;
        };
        match gesture {
            // Press-and-release without motion changes nothing.
            MouseGesture::Click { .. } if split.gap.contains(at) => {
                self.split_changed = false;
                None
            }
            // A second press in the gap is a double click, not a changed
            // drag. Clear the changed bit so its later release cannot emit
            // an end request for an earlier drag.
            MouseGesture::DoubleClick(_) => {
                self.split_changed = false;
                None
            }
            // A recognized `Drag` implies an armed press inside the gap, so
            // every drag resolves -- tracking necessarily continues outside
            // the gap once the pointer leaves it.
            MouseGesture::Drag { to, .. } => {
                let width = normalize_list_pane_width(
                    Some(split.pane_origin_x.saturating_sub(to.x)),
                    split.content_width,
                )
                .unwrap_or(split.width);
                if width == split.width {
                    return None;
                }
                self.split.as_mut()?.width = width;
                self.split_changed = true;
                Some(Msg::Shell(Box::new(ShellRequest::ResizeListPaneLive(
                    width,
                ))))
            }
            MouseGesture::DragEnd => std::mem::take(&mut self.split_changed).then_some(Msg::Shell(
                Box::new(ShellRequest::ResizeListPaneEnd(split.width)),
            )),
            _ => None,
        }
    }

    /// Interpret one non-gap mouse event against the panel's own painted
    /// slot geometry: pill rows resolve their slot events first, then the
    /// list slot delegates the normalized row-local input.
    fn surface_gesture(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let gesture = self.gestures.recognize(mouse)?;
        let at = match gesture {
            MouseGesture::Click { at, .. }
            | MouseGesture::DoubleClick(at)
            | MouseGesture::RightClick(at)
            | MouseGesture::Scroll { at, .. } => at,
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => return None,
        };
        if let Some(message) = self.selector_or_link_gesture(gesture, at) {
            return message;
        }
        // The hero pane is painted separately from the Browser pane's list
        // slot, and its owner decides what input it claims.
        if self.hero_pane_rect().is_some_and(|rect| rect.contains(at)) {
            return self.hero_surface_gesture(gesture);
        }
        self.list_surface_gesture(gesture)
    }

    /// Painted pill rows resolve first: the pill painted under the pointer in
    /// the latest frame is the one that resolves (ADR 0024). Link labels are
    /// ordinary text; the panel retains valid URL geometry and owns the click
    /// effect request.
    fn selector_or_link_gesture(
        &mut self,
        gesture: MouseGesture,
        at: Position,
    ) -> Option<Option<Msg>> {
        if let Some(&index) = self.hits.selector.resolve(at) {
            return Some(self.slot_event(LibrarySlotEvent::SelectorPicked(index)));
        }
        if let Some(&index) = self.hits.workspace_selector.resolve(at) {
            return Some(self.slot_event(LibrarySlotEvent::WorkspaceSelectorPicked(index)));
        }
        if let MouseGesture::Click { .. } = gesture {
            if let Some(&index) = self.hits.links.resolve(at) {
                if let Some(url) = self.painted_link_urls.get(index).cloned().and_then(|url| {
                    super::super::overview_box::sanitize_url(&url).map(str::to_owned)
                }) {
                    return Some(Some(Msg::Shell(Box::new(ShellRequest::OpenUrl(url)))));
                }
            }
        }
        None
    }

    /// Resolve the hero pane's own input, including Workspace box episode rows.
    fn hero_surface_gesture(&mut self, gesture: MouseGesture) -> Option<Msg> {
        match gesture {
            MouseGesture::Click { at, modifier } => {
                let input = match modifier {
                    ClickModifier::Ctrl => MediaListSurfaceInput::ToggleClick(at),
                    ClickModifier::Shift => MediaListSurfaceInput::RangeClick(at),
                    ClickModifier::None => MediaListSurfaceInput::Click(at),
                };
                self.slot_event(LibrarySlotEvent::HeroPane(input))
            }
            MouseGesture::DoubleClick(at) => self.slot_event(LibrarySlotEvent::HeroPane(
                MediaListSurfaceInput::DoubleClick(at),
            )),
            MouseGesture::RightClick(at) => self.slot_event(LibrarySlotEvent::HeroPane(
                MediaListSurfaceInput::ContextClick(at),
            )),
            MouseGesture::Scroll { at, delta } => {
                self.slot_event(LibrarySlotEvent::HeroPane(MediaListSurfaceInput::Wheel {
                    at,
                    delta,
                }))
            }
            _ => None,
        }
    }

    /// Resolve gestures inside the painted browser list slot.
    fn list_surface_gesture(&mut self, gesture: MouseGesture) -> Option<Msg> {
        let at = match gesture {
            MouseGesture::Click { at, .. }
            | MouseGesture::DoubleClick(at)
            | MouseGesture::RightClick(at)
            | MouseGesture::Scroll { at, .. } => at,
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => return None,
        };
        if !self.list_rect().is_some_and(|rect| rect.contains(at)) {
            return None;
        }
        match gesture {
            MouseGesture::Click { at, modifier } => {
                let input = match modifier {
                    ClickModifier::Ctrl => MediaListSurfaceInput::ToggleClick(at),
                    ClickModifier::Shift => MediaListSurfaceInput::RangeClick(at),
                    ClickModifier::None => MediaListSurfaceInput::Click(at),
                };
                self.slot_event(LibrarySlotEvent::List(input))
            }
            MouseGesture::DoubleClick(at) => {
                // Only a destination whose browser rows are hero-bearing
                // opens the Library Hero overlay on double-click; a
                // not-hero-bearing owner (the podcast tab) activates the
                // resolved row directly, and its first click's message was
                // already delivered by its own gesture.
                let overlay_attempt = self.narrow_geometry.is_some()
                    && self
                        .owners
                        .active_mut()
                        .is_some_and(|owner| owner.double_click_opens_hero_overlay());
                if overlay_attempt {
                    if let Some(message) = self.open_hero_from_browser(Some(at)) {
                        return Some(message);
                    }
                }
                self.slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
                    at,
                )))
            }
            MouseGesture::RightClick(at) => self.slot_event(LibrarySlotEvent::List(
                MediaListSurfaceInput::ContextClick(at),
            )),
            MouseGesture::Scroll { at, delta } => {
                self.slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
                    at,
                    delta,
                }))
            }
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => None,
        }
    }

    fn overlay_gesture(&mut self, mouse: &MouseEvent, at: Position) -> Option<Msg> {
        let geometry = self.overlay_geometry.as_ref()?;
        let gesture = self.gestures.recognize(mouse)?;
        if let Some(&index) = self.hits.workspace_selector.resolve(at) {
            return self.slot_event(LibrarySlotEvent::WorkspaceSelectorPicked(index));
        }
        if let MouseGesture::Click { .. } = gesture {
            if let Some(&index) = self.hits.links.resolve(at) {
                if let Some(url) = self.painted_link_urls.get(index).cloned().and_then(|url| {
                    super::super::overview_box::sanitize_url(&url).map(str::to_owned)
                }) {
                    return Some(Msg::Shell(Box::new(ShellRequest::OpenUrl(url))));
                }
            }
        }
        if geometry
            .hero
            .workspace
            .is_some_and(|(panel, _)| panel.contains(at))
        {
            return self.hero_surface_gesture(gesture);
        }
        if matches!(gesture, MouseGesture::DoubleClick(_)) {
            return self.activate_overlay_hero_selection();
        }
        Some(Msg::TerminalEvent(
            crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
        ))
    }

    fn activate_overlay_hero_selection(&mut self) -> Option<Msg> {
        let result = self
            .owners
            .active_mut()
            .map(|owner| owner.activate_hero_selection())
            .unwrap_or(LeafKeyResult::Unhandled);
        match result {
            // A pointer gesture always claims as mouse. Preserve a
            // destination request, but never leak a keyboard claim from an
            // owner's legacy leaf disposition.
            LeafKeyResult::Consumed(Some(message))
                if matches!(message.as_ref(), Msg::TerminalEvent(_)) =>
            {
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            LeafKeyResult::Consumed(message) => message.map(|message| *message).or(Some(
                Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed),
            )),
            LeafKeyResult::Unhandled => {
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
        }
    }

    pub(super) fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let gesture_boundary =
            matches!(mouse.kind, MouseEventKind::Down(_) | MouseEventKind::Up(_));
        if matches!(mouse.kind, MouseEventKind::Moved) {
            let at = Position::new(mouse.column, mouse.row);
            self.hovered_selector = self.hits.selector.resolve(at).copied();
            self.hovered_link = self.hits.links.resolve(at).copied();
            return None;
        }
        // The panel resolves only geometry it painted; an unpainted frame
        // (no migrated owner) claims nothing.
        let Some(painted_area) = self.painted_area else {
            self.reset_boundary_gesture(gesture_boundary);
            return None;
        };
        let at = Position::new(mouse.column, mouse.row);
        if !painted_area.contains(at) {
            self.reset_boundary_gesture(gesture_boundary);
            return None;
        }
        // The overlay owns the Library pane's current-frame gesture. A
        // backdrop click dismisses and is consumed; covered browser geometry
        // is never replayed into the list.
        if self.hero_overlay_open {
            self.reset_boundary_gesture(gesture_boundary);
            if !matches!(mouse.kind, MouseEventKind::Moved)
                && !self
                    .overlay_geometry
                    .as_ref()
                    .is_some_and(|geometry| geometry.frame.contains(at))
            {
                self.dismiss_hero_overlay();
                return Some(Msg::TerminalEvent(
                    crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
                ));
            }
            if self
                .overlay_geometry
                .as_ref()
                .is_some_and(|geometry| geometry.frame.contains(at))
            {
                return self.overlay_gesture(mouse, at);
            }
        }
        match mouse.kind {
            // A left press inside the painted gap arms only the split drag;
            // a press outside never arms it, so pane gestures are untouched.
            MouseEventKind::Down(MouseButton::Left) if self.split_hit(at) => {
                let gesture = self.split_gestures.recognize(mouse);
                gesture.and_then(|gesture| self.split_gesture_msg(gesture, at))
            }
            MouseEventKind::Down(_) => {
                self.reset_split_gesture();
                self.surface_gesture(mouse)
            }
            // A recognized `Drag` implies an armed press: whichever
            // recognizer armed it consumes the continuation, so the split
            // drag tracks outside the gap and pane gestures never see a
            // gap-armed drag. Release closes whichever gesture armed; a
            // changed gap gesture emits its one persistence request, while a
            // click or pane gesture remains inert at this boundary.
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                let gesture = self.split_gestures.recognize(mouse);
                let unowned = gesture.is_none();
                if let Some(msg) = gesture.and_then(|gesture| self.split_gesture_msg(gesture, at)) {
                    return Some(msg);
                }
                if unowned {
                    self.reset_split_gesture();
                }
                self.surface_gesture(mouse)
            }
            _ => self.surface_gesture(mouse),
        }
    }

    /// Whether the painted split's gap claims `at`.
    fn split_hit(&self, at: Position) -> bool {
        self.split
            .as_ref()
            .is_some_and(|split| split.gap.contains(at))
    }

    /// Forget the last painted frame's geometry: the panel resolves only
    /// geometry it painted. `view` calls this before painting, so an unpainted
    /// frame (no migrated owner) and a breakpoint transition both leave only
    /// the current frame's geometry behind.
    pub(super) fn reset_frame(&mut self) {
        self.hits = SkeletonHits::default();
        self.wide_geometry = None;
        self.narrow_geometry = None;
        self.overlay_geometry = None;
        self.painted_area = None;
        self.split = None;
        self.image_paint = None;
        self.painted_link_urls.clear();
    }
}
