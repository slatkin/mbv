//! The column divider (`replace-tauri-with-blitz` task 5.1, the port of
//! `PanelDivider`; add-gui-library-panel design D4): the Queue panel's
//! column and the Wide arrangement's list column are resized by dragging.
//! The drag reports the sized column's new width in pixels and the session
//! keeps it ([`crate::window::resize`]), so this component holds nothing
//! beyond the drag itself. A double-click reports the reset to the default
//! width, and the pointer affordance is the column-resize cursor over a band
//! that lights while hovered or dragged. The band takes no width in the row:
//! its negative margins lay it over the seam, above both columns, so the two
//! columns meet directly.
//!
//! the former client port listened for the drag's moves on the `window`: the pointer
//! leaves the band's few pixels as soon as the drag starts. The Dioxus tree
//! has no window listener, so the drag lives in the [`DividerDrag`] store the
//! root provides: the divider's `pointerdown` arms it with the drag's
//! callback, and the root's `pointermove`, `pointerup`, and `pointercancel`
//! handlers drive it — exactly the events the `window` listeners took. The
//! sized column sits on one side of the divider, so a drag right widens a
//! column left of the divider and narrows one right of it.

use dioxus_native::prelude::*;

use crate::window::input::f32_from_f64;
use crate::window::resize::DIVIDER_WIDTH_PX;

/// The side of the divider the sized column sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerSide {
    /// The sized column is left of the divider: a drag right widens it.
    Left,
    /// The sized column is right of the divider: a drag left widens it.
    Right,
}

/// What the divider resizes, for the accessible name and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerName {
    /// The Queue panel's column.
    Queue,
    /// The Wide arrangement's list column.
    List,
}

impl DividerName {
    /// The `data-divider` value the former markup paints.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::List => "list",
        }
    }

    /// The divider's accessible name.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Queue => "Resize the Queue panel",
            Self::List => "Resize the Library list",
        }
    }
}

/// One drag under way: where it started, the width it started from, which
/// side the sized column sits on, the dragged divider's own drag look, and
/// the callback that reports each new width.
#[derive(Clone, Copy)]
struct ActiveDrag {
    origin_x: f64,
    start_width: f32,
    /// `+1.0` for a `Left` column, `-1.0` for a `Right` one.
    sign: f64,
    /// The dragged divider's painted state; the drag's end clears it.
    dragging: Signal<bool>,
    on_drag: Callback<f32>,
}

/// The drag one divider holds: the port of `PanelDivider`'s `window`
/// listeners. The root creates it once (`use_hook`), provides it as context,
/// and drives it from its pointer handlers; the divider arms it. At most one
/// drag runs at a time — a new `pointerdown` on the other divider replaces
/// the first, as the window's listeners did.
#[derive(Clone, Copy)]
pub struct DividerDrag {
    active: Signal<Option<ActiveDrag>>,
}

impl std::fmt::Debug for DividerDrag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DividerDrag")
            .field("drag under way", &self.active.peek().is_some())
            .finish()
    }
}

impl Default for DividerDrag {
    fn default() -> Self {
        Self::new()
    }
}

impl DividerDrag {
    /// A store with no drag under way.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: Signal::new(None),
        }
    }

    /// A divider's `pointerdown`: arm the drag over
    /// the window's pointer events.
    pub fn start(
        &mut self,
        origin_x: f64,
        start_width: f32,
        side: DividerSide,
        dragging: Signal<bool>,
        on_drag: Callback<f32>,
    ) {
        self.active.set(Some(ActiveDrag {
            origin_x,
            start_width,
            sign: match side {
                DividerSide::Left => 1.0,
                DividerSide::Right => -1.0,
            },
            dragging,
            on_drag,
        }));
    }

    /// The root's `pointermove`: report the
    /// sized column's new width — the start width plus the signed drag
    /// distance. A drag under way ends only on a release, a cancel, or the
    /// divider's unmount.
    pub fn pointer_move(&self, client_x: f64) {
        let Some(drag) = self.active.cloned() else {
            return;
        };
        let width = drag.start_width + f32_from_f64(drag.sign * (client_x - drag.origin_x));
        drag.on_drag.call(width);
    }

    /// The root's `pointerup` or `pointercancel`: the
    /// drag ends and the band's drag look clears.
    pub fn end(&mut self) {
        let Some(mut drag) = self.active.cloned() else {
            return;
        };
        drag.dragging.set(false);
        self.active.set(None);
    }
}

/// The props of the column divider (the `PanelDivider` props, in
/// order).
#[derive(Props, Clone, PartialEq)]
pub struct PanelDividerProps {
    /// The side of the divider the sized column sits on.
    pub side: DividerSide,
    /// The sized column's width when a drag starts, in pixels.
    pub width: f32,
    /// What the divider resizes, for the accessible name and the tests.
    pub name: DividerName,
    /// A drag's new width for the sized column, in pixels.
    pub on_drag: Callback<f32>,
    /// A double-click: the sized column returns to its default width.
    pub on_reset: Callback<()>,
}

impl std::fmt::Debug for PanelDividerProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The callbacks carry no painted fact; the divider's identity and
        // width do.
        f.debug_struct("PanelDividerProps")
            .field("side", &self.side)
            .field("width", &self.width)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The column divider (task 5.1). The band paints the column-resize cursor
/// over the drag affordance; the drag itself lives in the [`DividerDrag`]
/// store the root drives.
#[component]
pub fn PanelDivider(props: PanelDividerProps) -> Element {
    let mut divider_drag = consume_context::<DividerDrag>();
    let mut dragging = use_signal(|| false);

    // A divider that unmounts mid-drag (a rearrangement or the collapse
    // toggle) leaves no drag armed: the former client `$effect` teardown stopped the
    // window listeners, and the band's own drag look is the ownership fact —
    // it is `true` only while this divider holds the drag.
    let mut drop_drag = divider_drag;
    use_drop(move || {
        if dragging.cloned() {
            drop_drag.end();
        }
    });

    let side = props.side;
    let on_drag = props.on_drag;
    let width = props.width;

    rsx! {
        button {
            "type": "button",
            "aria-label": props.name.label(),
            "data-divider": props.name.name(),
            "data-divider-dragging": dragging.cloned().then_some(""),
            class: "relative z-10 flex-none self-stretch border-0 p-0 hover:bg-[var(--canvas-border)] data-[divider-dragging]:bg-[var(--canvas-border)]",
            style: "width: {DIVIDER_WIDTH_PX}px; margin-inline: -{DIVIDER_WIDTH_PX / 2.0}px; cursor: col-resize;",
            onpointerdown: move |event: PointerEvent| {
                // Only the primary button drags, as the former client `startDrag`
                // did; the press itself forwards to the core through the
                // root's handler, as every other press does.
                if !matches!(
                    event.data.trigger_button(),
                    Some(dioxus_html::input_data::MouseButton::Primary)
                ) {
                    return;
                }
                event.prevent_default();
                dragging.set(true);
                divider_drag.start(
                    event.data.client_coordinates().x,
                    width,
                    side,
                    dragging,
                    on_drag,
                );
            },
            ondoubleclick: move |_| {
                props.on_reset.call(());
            },
        }
    }
}
