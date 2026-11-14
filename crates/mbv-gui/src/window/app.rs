//! The window's root (D6 of `replace-tauri-with-blitz`): the port of
//! `App.svelte` and `main.ts`. The stylesheet mounts with the cell-unit rule
//! of `window/cells.rs` first, so the first paint resolves every `calc()`
//! over the two properties (main.ts's `applyCellUnit` before mount), the
//! theme style (`window/theme.rs`) follows, and the tree composes the attach
//! screen, the overlays, the Settings screen, and the player and Library
//! trees exactly as `App.svelte` does. The `sync` on mount asks the core for
//! its first full batch, as the `onMount` `sync` did.

use std::sync::{Arc, LazyLock};

use blitz_shell::{BlitzShellEvent, BlitzShellProxy};
use dioxus_native::prelude::*;
use dioxus_native::use_window;
use dioxus_native::winit::window::{Window, WindowId};
use futures_util::StreamExt;
use mbv_config::{load_gui_panel_widths, save_gui_panel_widths};

use crate::bridge::frame::focus::GuiPanel;
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::keys::DomKey;
use crate::bridge::layout::LayoutSize;
use crate::bridge::pointer::DomPointerKind;
use crate::bridge::sections::GuiSectionBatch;
use crate::window::arrangement::{GuiArrangement, library_arrangement, settings_arrangement};
use crate::window::cells::cell_unit_rule;
use crate::window::channel::{CoreLink, EventSlot, WindowEvent};
use crate::window::clock::{Clock, FiredTimers};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::components::attach_screen::AttachScreen;
use crate::window::components::confirm_overlay::ConfirmOverlay;
use crate::window::components::daemon_lost_overlay::DaemonLostOverlay;
use crate::window::components::library_panel::LibraryPanel;
use crate::window::components::panel::Panel;
use crate::window::components::panel_divider::{
    DividerDrag, DividerName, DividerSide, PanelDivider,
};
use crate::window::components::settings_screen::SettingsScreen;
use crate::window::input::KeyOutcome;
use crate::window::layout::{
    LayoutEpoch, RemeasureRequest, track_container_size, use_layout_reporter,
};
use crate::window::library::keys::route_library_key;
use crate::window::library::reset::reset_library_interaction;
use crate::window::library::state::LibraryState;
use crate::window::playhead::{Playhead, SystemTimes, Times};
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::Sections;
use crate::window::settings::state::SettingsState;
use crate::window::theme::{ThemeStyle, canvas_var};
use crate::window::{input, resize};

/// The generated stylesheet (D11): `make gui-css` builds it from
/// `assets/app.css` over the Rust sources, and the output is committed.
const APP_GENERATED_CSS: &str = include_str!("../../assets/app.generated.css");

/// The root's one style element: the cell-unit rule first, then the
/// stylesheet (D6).
static ROOT_STYLE: LazyLock<String> =
    LazyLock::new(|| format!("{}\n{}", cell_unit_rule(), APP_GENERATED_CSS));

/// Apply one batch the drain task received (D6): the sections write in one
/// pass, the playhead stamps the batch's anchor, and the layout epoch bumps
/// when the batch changes a measured fact (D9) — a panel state change, the
/// Settings screen's open or close, or a list's row count. A row a batch
/// mounts re-measures through [`RemeasureRequest`] instead (D12): its own
/// first measure asks for the bump, after the layout has placed it.
fn apply_batch(
    sections: &mut Sections,
    playhead: &mut Playhead,
    mut epoch: LayoutEpoch,
    batch: &GuiSectionBatch,
) {
    let rows_before = sections.queue.peek().rows.len();
    let panel_before = *sections.panel.peek();
    let settings_before = sections.settings.peek().is_some();
    sections.apply_batch(batch);
    if let Some(anchor) = &batch.playhead {
        playhead.stamp_if_new(anchor);
    }
    let rows_after = sections.queue.peek().rows.len();
    let panel_after = *sections.panel.peek();
    let settings_after = sections.settings.peek().is_some();
    let panel_state_changed = panel_before.collapsed != panel_after.collapsed
        || panel_before.queue_folded != panel_after.queue_folded;
    if rows_before != rows_after || panel_state_changed || settings_before != settings_after {
        epoch.bump();
    }
}

/// The drain task's state (D4): the contexts the event dispatch reads and
/// writes. The root builds one and hands it to [`use_window_drain`].
#[derive(Clone)]
struct Drain {
    proxy: BlitzShellProxy,
    window_id: WindowId,
    /// The window the drain reads the scale factor from (D7, D9).
    scale_window: Arc<dyn Window>,
    scale: ScaleFactor,
    epoch: LayoutEpoch,
    remeasure: RemeasureRequest,
    art_events: ArtEvents,
    fired: FiredTimers,
    sections: Sections,
    playhead: Playhead,
    library: LibraryState,
}

impl Drain {
    /// Dispatch one drained event (D4).
    fn dispatch(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Quit => {
                self.proxy.send_event(BlitzShellEvent::CloseWindow {
                    window_id: self.window_id,
                });
            }
            // A viewport change is a layout fact (D9): every measurement
            // effect re-measures, and the scale factor the natural-size caps
            // divide by rewrites (D7).
            WindowEvent::Viewport => {
                self.scale.0.set(self.scale_window.scale_factor());
                self.epoch.bump();
            }
            // An art answer reaches its `ArtImage` through the store (D7).
            WindowEvent::Art(event) => self.art_events.push(event),
            // A passed timer reaches the surface that armed it (D10); the
            // playhead's minute clock re-arms through the same route, and the
            // Search query's debounce sends its settled query over its own.
            WindowEvent::Timer(timer) => {
                self.playhead.on_timer(timer);
                self.library.on_search_timer(timer);
                self.fired.push(timer);
            }
            // A presented frame advances the playhead's prediction (D10), and
            // carries the mount-time re-measure requests (D12): the layout of
            // this frame is resolved, so the bump re-measures every `Truncate`
            // box that mounted unlaid — before any effect or render body of
            // the bump's own turn reads it. A request is taken once, and only
            // a box's first measure makes one, so the bump it drives cannot
            // request another.
            WindowEvent::Frame => {
                self.playhead.on_frame();
                if self.remeasure.take() {
                    self.epoch.bump();
                }
            }
            // A section batch writes the stores (D6).
            WindowEvent::Batch(batch) => {
                apply_batch(&mut self.sections, &mut self.playhead, self.epoch, &batch);
            }
        }
    }
}

/// The window channel's drain (D4): the root's one long-lived future. The
/// receiver is taken once; the drain lives for the window's life.
fn use_window_drain(drain: Drain, slot: EventSlot) {
    use_future(move || {
        let mut drain = drain.clone();
        let slot = slot.clone();
        async move {
            let Some(mut events) = slot.take() else {
                return;
            };
            while let Some(event) = events.next().await {
                drain.dispatch(event);
            }
        }
    });
}

/// Fold one frame's `uiResetEpoch` into the width store (D14): the effect
/// body the root runs, shared with the headless test that proves it
/// settles. Reads the `panel` section only — a read of the width store's
/// own `uiResetEpoch` signal here would re-queue the effect forever, since
/// every signal write marks its subscribers (an equal value included).
#[must_use]
fn note_panel_ui_reset(store: &mut PanelWidthsStore, sections: &Sections) -> bool {
    let panel = sections.panel.cloned();
    store.note_ui_reset_epoch(panel.ui_reset_epoch)
}

/// The window content's key router (D8, tasks 6.2 and 8.1): the Settings
/// steps and the Library steps answer a keydown before the forwarder sends
/// the key to the core, in that order — the Settings screen paints in place
/// of the panels while its frame section is present, so its steps run first.
/// The router owns its section stores and interaction stores, so the
/// handler's `move` closure can hand the trait object up; the routing steps
/// read the sections through `peek`, so a keydown copies none of their
/// values.
fn window_key_router(
    library: LibraryState,
    settings: SettingsState,
    sections: Sections,
) -> impl Fn(&DomKey) -> KeyOutcome + 'static {
    move |key| route_library_key(key, Some(&sections), &library, Some(&settings))
}
/// Fold one frame's `uiResetEpoch` into the window's presentation (D14, the
/// port of `reset.ts`): a changed value resets the panel widths, the Library
/// interaction stores, and the Settings interaction state on the same fact.
fn note_ui_reset(
    store: &mut PanelWidthsStore,
    sections: &Sections,
    library: &LibraryState,
    settings: &SettingsState,
) {
    let reset = note_panel_ui_reset(store, sections);
    if reset {
        reset_library_interaction(library);
        settings.reset_settings_ui();
    }
}

/// The window's root (D6).
#[component]
pub fn App() -> Element {
    let window = use_window();
    let window_id = window.id();
    let slot = use_hook(consume_context::<EventSlot>);
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    // The mount-time re-measure requests (D12): a `Truncate` whose first
    // measure reads an unlaid box sets the request, and the drain's
    // presented-frame handler takes it and bumps the epoch once. The launch
    // copy provides it as a root context, so its loop presents the frame that
    // answers the request; a tree built without the launch copy gets its own.
    // The root only provides it — no effect reads it, so a request re-runs no
    // effect.
    let root_remeasure = try_consume_context::<RemeasureRequest>();
    let remeasure = use_hook(|| root_remeasure.unwrap_or_default());
    provide_context(remeasure.clone());
    let art_events = use_hook(ArtEvents::new);
    provide_context(art_events);
    let fired = use_hook(FiredTimers::new);
    provide_context(fired);
    let scale_window = Arc::clone(&window);
    let scale = use_hook(move || ScaleFactor(Signal::new(scale_window.scale_factor())));
    provide_context(scale);
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let clock = consume_context::<Arc<dyn Clock>>();
    // The playhead's clock and the Search debounce's clock (D10) are one
    // context value: the clone the playhead's hook moves keeps the original
    // for the Library store's debounce to arm.
    let playhead_clock = Arc::clone(&clock);
    let times: Arc<dyn Times> = Arc::new(SystemTimes::new());
    let playhead = use_hook(move || Playhead::new(Arc::clone(&playhead_clock), times));
    provide_context(playhead.clone());
    let reporter = use_layout_reporter();
    provide_context(reporter);
    let widths_store = use_hook(|| {
        PanelWidthsStore::new(load_gui_panel_widths(), Arc::new(save_gui_panel_widths))
    });
    provide_context(widths_store.clone());
    let divider_drag = use_hook(DividerDrag::new);
    provide_context(divider_drag);
    let link = consume_context::<CoreLink>();
    // The Library panel's interaction store (D15): the port of `state.ts`'s
    // and `scroll.ts`'s module stores as one context value, over the sections
    // and the core link the commands go through.
    let library = use_hook(|| {
        LibraryState::new(
            sections.clone(),
            CoreLink::clone(&link),
            playhead.position,
            Arc::clone(&clock),
        )
    });
    provide_context(library.clone());
    // The Settings screen's interaction store (D15, row 8.1): the port of
    // `lib/settings/state.ts`'s and `pick-menu.ts`'s module stores as one
    // context value, over the core link the commands go through.
    let settings_state = use_hook(|| SettingsState::new(CoreLink::clone(&link)));
    provide_context(settings_state.clone());

    // The window channel's drain (D4): every event from outside the tree
    // dispatches here.
    use_window_drain(
        Drain {
            proxy: use_hook(consume_context::<BlitzShellProxy>),
            window_id,
            scale_window: Arc::clone(&window),
            scale,
            epoch,
            remeasure,
            art_events,
            fired,
            sections: sections.clone(),
            playhead: playhead.clone(),
            library: library.clone(),
        },
        slot,
    );

    // The sync on mount (D6): the window asks for the full snapshot once its
    // section listener is subscribed. The drain above is that listener; both
    // live for the window's life, as `App.svelte`'s `onMount` set them up.
    let sync_link = CoreLink::clone(&link);
    use_effect(move || {
        sync_link.sync();
    });

    // The root element takes focus on mount (D8): every keydown reaches its
    // one `onkeydown` handler.
    let mut root_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    use_effect(move || {
        let mounted = root_mounted.cloned();
        spawn(async move {
            if let Some(event) = mounted {
                let _ = event.set_focus(true).await;
            }
        });
    });

    // The Settings screen's interaction state follows the `settings` section
    // (add-gui-settings-screen design D1), and a new `uiResetEpoch` in the
    // `panel` section resets the window's presentation — the Library
    // interaction stores, the panel widths, and the Settings state. The
    // widths reset here, the Library interaction stores with them (the port
    // of `reset.ts`'s `resetLibraryInteraction`), and the Settings state with
    // its port (row 8.1, `resetSettingsUi`); the `settings` section then
    // folds in (`syncSettingsFrame`). The effect reads the section stores and
    // writes the interaction stores — never one it read.
    let mut reset_store = widths_store.clone();
    let reset_sections = sections.clone();
    let reset_library = library.clone();
    let reset_settings = settings_state.clone();
    use_effect(move || {
        note_ui_reset(
            &mut reset_store,
            &reset_sections,
            &reset_library,
            &reset_settings,
        );
        let frame = reset_sections.settings.cloned();
        reset_settings.sync_settings_frame(frame.as_ref());
    });

    // The panel container the layout report carries the size of (D9): the
    // box the panel states are picked from. Its measured size reports to the
    // core and hands to `Panel`, so the state the window paints and the
    // capacity and hit map the core applies come from one measurement.
    let container: Signal<LayoutSize> = use_signal(LayoutSize::default);
    let mut container_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_container_size(container_mounted, container);

    // The two-panel arrangement (add-gui-library-panel design D4): the Queue
    // panel beside the Library panel, picked from the measured Library column
    // width — the window width minus the Queue panel's current width. The
    // Queue panel's column and the Wide list column are the session's widths
    // (design D4): the dividers and the arrangement read one resolution.
    let connected = sections.connected.cloned();
    let attach = sections.attach.cloned();
    let settings = sections.settings.cloned();
    let overlay = sections.overlay.cloned();
    let focus = sections.focus.cloned();
    let panel = sections.panel.cloned();
    let library_list = sections.library_list.cloned();
    let container_size = container.cloned();
    let widths = resize::panel_widths(container_size, &panel, &widths_store.dragged());
    let arrangement = library_arrangement(container_size, widths.queue);
    // The Settings screen's own measure: its sidebar is the canvas's fixed
    // 260 px in the Queue column's place, so the screen's Mini threshold is
    // the window width minus that width, never the dragged Queue column's
    // (design D11; `App.svelte`'s
    // `libraryArrangement(container, SETTINGS_SIDEBAR_WIDTH_PX)`).
    let settings_screen_arrangement = settings_arrangement(container_size);
    let queue_focused = focus.panel == GuiPanel::Queue;
    // The Library panel's one presence fact (design D2, D10, D12): the
    // window holds the active destination's content, whose painters the
    // panel mounts. The key router gates on the same fact. (`libraryShown`
    // of `lib/library/state.ts`; its port lands with task 6.1.)
    let panel_shows = library_list.is_some();
    let queue_visible = !panel_shows || arrangement != GuiArrangement::Mini || queue_focused;
    let library_visible = panel_shows && (arrangement != GuiArrangement::Mini || !queue_focused);
    let two_columns = queue_visible && library_visible;
    // The container's backdrop fill stays focus-reactive like the TUI's
    // `panel_appearance_focus` picks it.
    let queue_canvas = canvas_var(if queue_focused {
        "queue-column-focused"
    } else {
        "queue-column"
    });
    // Every handler holds its own link to the core (D4).
    let key_link = CoreLink::clone(&link);
    let press_link = CoreLink::clone(&link);
    let release_link = CoreLink::clone(&link);
    let move_link = CoreLink::clone(&link);
    let wheel_link = CoreLink::clone(&link);
    let cancel_link = CoreLink::clone(&link);
    let leave_link = CoreLink::clone(&link);
    let blur_link = CoreLink::clone(&link);
    // The divider drag's handlers hold their own copies (D4).
    let mut drag_release = divider_drag;
    let mut drag_cancel = divider_drag;
    let drag_move = divider_drag;
    // The drag's width callback (D14): every step clamps, saves, and bumps
    // the layout epoch, as the Svelte `dragQueueWidth` and the ResizeObserver
    // replacement did.
    let mut drag_widths = widths_store.clone();
    let mut drag_epoch = epoch;
    let drag_container = container;
    let mut reset_widths = widths_store.clone();
    let mut reset_epoch = epoch;

    rsx! {
        style { dangerous_inner_html: ROOT_STYLE.as_str() }
        ThemeStyle {}
        main {
            class: "h-dvh w-full overflow-hidden bg-background text-foreground",
            tabindex: 0,
            onmounted: move |event| root_mounted.set(Some(event)),
            onkeydown: move |event: KeyboardEvent| {
                // Every keydown meets the window content's router first,
                // which takes the keys it handles and lets the forwarder send
                // the rest (add-gui-library-panel design D3). The Settings
                // steps run ahead of the Library ones (row 8.1); both read
                // the window's section stores and the interaction stores.
                let router =
                    window_key_router(library.clone(), settings_state.clone(), sections.clone());
                input::on_keydown(&event, &router, &key_link);
            },
            onpointerdown: move |event: PointerEvent| {
                input::on_pointer(DomPointerKind::Press, &event.data, &press_link);
            },
            onpointerup: move |event: PointerEvent| {
                // A divider drag ends on the same release the core receives,
                // as the window's `window` listener did.
                drag_release.end();
                input::on_pointer(DomPointerKind::Release, &event.data, &release_link);
            },
            onpointermove: move |event: PointerEvent| {
                // A divider drag's moves report through the drag's callback
                // before the move forwards to the core, as the Svelte
                // `window` listener and the root's forwarder both did.
                drag_move.pointer_move(event.data.client_coordinates().x);
                input::on_pointer(DomPointerKind::Move, &event.data, &move_link);
            },
            onwheel: move |event: WheelEvent| {
                let scale = window.scale_factor();
                let viewport_height = f64::from(window.surface_size().height) / scale;
                input::on_wheel(&event.data, viewport_height, &wheel_link);
            },
            oncontextmenu: move |event: MouseEvent| {
                // The window's own context menu is turned off
                // (add-tauri-gui-client D11); blitz already emits the right
                // press (D8), so the handler only prevents the default.
                input::on_contextmenu(&event);
            },
            onpointercancel: move |event: PointerEvent| {
                // The cancelled gesture drops the divider's drag too.
                drag_cancel.end();
                input::on_pointer_cancel(&event.data, &cancel_link);
            },
            onpointerleave: move |event: PointerEvent| {
                input::on_pointer_leave(&event.data, &leave_link);
            },
            onblur: move |_: FocusEvent| {
                input::on_blur(&blur_link);
            },
            if connected && attach.is_none() {
                div {
                    class: "relative flex h-full min-h-0 w-full min-w-0 overflow-hidden",
                    "data-arrangement": arrangement.name(),
                    style: "background: {queue_canvas};",
                    onmounted: move |event| container_mounted.set(Some(event)),
                    if settings.is_some() {
                        // The Settings screen replaces the two panels while
                        // its section is present (add-gui-settings-screen
                        // design D11): no player panel, queue, or Library
                        // panel mounts, and playback continues. The screen's
                        // Mini check measures the window minus its fixed
                        // 260 px sidebar, which the root measures separately
                        // from the Library panel's own arrangement.
                        SettingsScreen { arrangement: settings_screen_arrangement }
                    } else {
                        if queue_visible {
                            div {
                                class: if two_columns {
                                    "flex h-full min-h-0 flex-col flex-none"
                                } else {
                                    "flex h-full min-h-0 flex-col flex-1"
                                },
                                "data-queue-column": "",
                                style: if two_columns {
                                    format!("width: {}px;", widths.queue)
                                } else {
                                    String::new()
                                },
                                Panel {
                                    container: container.cloned(),
                                    library_visible,
                                }
                            }
                        }
                        if two_columns && !panel.collapsed {
                            // The Queue panel's divider (design D4): dragging
                            // it resizes the Queue column and, with it, the
                            // Library column the arrangement measures; a
                            // double-click returns the canvas width.
                            PanelDivider {
                                side: DividerSide::Left,
                                name: DividerName::Queue,
                                width: widths.queue,
                                on_drag: move |width| {
                                    drag_widths
                                        .drag_queue_width(width, drag_container.cloned().width);
                                    drag_epoch.bump();
                                },
                                on_reset: move |()| {
                                    reset_widths.reset_queue_width();
                                    reset_epoch.bump();
                                },
                            }
                        }
                        if library_visible && library_list.is_some() {
                            LibraryPanel {
                                arrangement,
                                list_width: widths.list,
                                library_column: widths.library_column,
                            }
                        }
                    }
                }
                if let Some(frame) = overlay.as_ref() {
                    match frame {
                        // The App's raised confirmation modal, window-modal
                        // over every panel (add-gui-library-panel queue
                        // fixes): Enter and Esc answer in the core's overlay
                        // key rules, and the buttons report their press
                        // regions.
                        GuiOverlayFrame::Confirm { message, buttons } => rsx! {
                            ConfirmOverlay {
                                message: message.clone(),
                                buttons: buttons.clone(),
                            }
                        },
                        // The App's raised daemon-lost recovery dialog,
                        // window-modal over every panel: the dialog blocks
                        // every other input until one of its choices runs,
                        // and the buttons report their press regions.
                        GuiOverlayFrame::DaemonLost {
                            last_playing_title,
                            daemon_log_path,
                            restart_error,
                            buttons,
                        } => rsx! {
                            DaemonLostOverlay {
                                last_playing_title: last_playing_title.clone(),
                                daemon_log_path: daemon_log_path.clone(),
                                restart_error: restart_error.clone(),
                                buttons: buttons.clone(),
                            }
                        },
                        // The row menu, track popover, and playback-target
                        // chooser paint inside their panels; the root mounts
                        // nothing for them, as `App.svelte` does.
                        _ => rsx! {},
                    }
                }
            } else {
                // The pre-attach states (design D9): the wait before the
                // first batch, the Owner attach outcomes, and the mismatch
                // prompt. The attach screen reads the `attach` section
                // itself.
                AttachScreen {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
