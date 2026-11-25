//! The window's root (D6 of `replace-tauri-with-blitz`): the port of
//! `App` and `main.ts`. The stylesheet mounts with the cell-unit rule
//! of `window/cells.rs` first, so the first paint resolves every `calc()`
//! over the two properties (main.ts's `applyCellUnit` before mount), the
//! theme style (`window/theme.rs`) follows, and the tree composes the attach
//! screen, the overlays, the Settings screen, and the player and Library
//! trees exactly as `App` does. The `sync` on mount asks the core for
//! its first full batch, as the `onMount` `sync` did.

use std::sync::{Arc, LazyLock};

use blitz_shell::{BlitzShellEvent, BlitzShellProxy};
use dioxus_native::prelude::*;
use dioxus_native::use_window;
use dioxus_native::winit::window::{Window, WindowId};
use futures_util::StreamExt;
use mbv_config::{load_gui_panel_widths, save_gui_panel_widths};

use crate::bridge::frame::library::GuiLibraryTabFrame;
use crate::bridge::keys::DomKey;
use crate::bridge::pointer::DomPointerKind;
use crate::bridge::sections::GuiSectionBatch;
use crate::window::arrangement::{GuiArrangement, library_arrangement, settings_arrangement};
use crate::window::cells::cell_unit_rule;
use crate::window::channel::{CoreLink, EventSlot, WindowEvent};
use crate::window::clock::{Clock, FiredTimers};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::components::attach_screen::AttachScreen;
use crate::window::components::library_top_bar::{HOME_POSITION, LibraryTopBar};
use crate::window::components::panel_divider::{
    DividerDrag, DividerName, DividerSide, PanelDivider,
};
use crate::window::components::settings_screen::SettingsScreen;
use crate::window::components::toast_stack::ToastStack;
use crate::window::input::KeyOutcome;
use crate::window::layout::{
    LayoutEpoch, ViewportSize, note_presented_frame, track_container_size, use_container_size,
    use_layout_reporter,
};
use crate::window::library::keys::route_library_key;
use crate::window::library::parts::{LibraryPair, library_pair};
use crate::window::library::reset::reset_library_interaction;
use crate::window::library::state::LibraryState;
use crate::window::playhead::{Playhead, SystemTimes, Times};
use crate::window::pointer_target::{PointerTarget, PointerTargetStore};
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::Sections;
use crate::window::settings::state::SettingsState;
use crate::window::theme::{ThemeStyle, canvas_var};
use crate::window::{input, resize};
use render::{library_column, overlay_layer, queue_column};

/// The generated stylesheet (D11): `make gui-css` builds it from
/// `assets/app.css` over the Rust sources, and the output is committed.
const APP_GENERATED_CSS: &str = include_str!("../../assets/app.generated.css");

/// The root's one style element: the cell-unit rule first, then the
/// stylesheet (D6).
static ROOT_STYLE: LazyLock<String> =
    LazyLock::new(|| format!("{}\n{}", cell_unit_rule(), APP_GENERATED_CSS));

/// The root's `style` element (D6). The CSS reaches the document as the
/// element's own text child, never through `dangerous_inner_html`: the
/// `DocumentConfig` the launch copy builds leaves Blitz's HTML parser
/// provider at its default, and that default is a no-op (`blitz-dom`
/// 0.3.0-beta.4 `html.rs`, `DummyHtmlParserProvider`), so an inner-HTML
/// parse writes no text — `process_style_element` then reads an empty
/// string and the window carries no stylesheet at all. A text child needs
/// no parser, and the element's own flush reads it.
fn root_style_element() -> Element {
    let stylesheet = ROOT_STYLE.as_str();
    rsx! {
        style { "{stylesheet}" }
    }
}

/// Apply one batch the drain task received (D6): the sections write in one
/// pass and the playhead stamps the batch's anchor. No layout epoch bumps here
/// (D9): the batch's rows and panel states reach the document only after this
/// poll's render, so a bump now would measure the layout from before them. The
/// frame that resolves the change carries it and bumps then, whatever section
/// the batch touched.
fn apply_batch(sections: &mut Sections, playhead: &mut Playhead, batch: &GuiSectionBatch) {
    sections.apply_batch(batch);
    if let Some(anchor) = &batch.playhead {
        playhead.stamp_if_new(anchor);
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
            // A viewport change rewrites the scale factor the natural-size
            // caps divide by (D7). The layout size does not arrive here: the
            // launch copy writes it into the tree before the frame that resolves
            // at the new size (D9), so this event would be a turn late for it.
            WindowEvent::Viewport => {
                self.scale.0.set(self.scale_window.scale_factor());
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
            // A presented frame carries the frame's change fact (D12): a frame whose resolve
            // applied a tree change, a viewport change, or a font bumps the
            // epoch once, so every measurement effect re-reads its box against
            // the layout this frame painted — before any effect or render body
            // of the bump's own turn reads it. A frame that resolved nothing
            // new bumps nothing.
            WindowEvent::Frame { changed } => {
                note_presented_frame(self.epoch, changed);
            }
            // A section batch writes the stores (D6).
            WindowEvent::Batch(batch) => {
                apply_batch(&mut self.sections, &mut self.playhead, &batch);
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
/// values. The arrangement is the frame's own value — the one the Library
/// panel paints (design D4) — so the keys close the Narrow Hero overlay on
/// the fact the panel shows. The pointer target (design D1 of
/// `gui-pointer-target-interaction`) is read at key time through `peek`: no
/// component subscribes to it, so a hover change re-renders nothing.
fn window_key_router(
    library: LibraryState,
    settings: SettingsState,
    sections: Sections,
    arrangement: GuiArrangement,
    pointer_target: Signal<PointerTarget>,
) -> impl Fn(&DomKey) -> KeyOutcome + 'static {
    move |key| {
        route_library_key(
            key,
            Some(&sections),
            &library,
            Some(&settings),
            arrangement,
            &pointer_target.peek(),
        )
    }
}

/// The panel the Mini arrangement shows (design D8): the Queue until a press
/// on a destination tab shows the Library in its place. Every other
/// arrangement shows both columns, so only Mini reads this value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum MiniView {
    /// The Queue column alone, the mounted value.
    #[default]
    Queue,
    /// The Library panel in the Queue column's place.
    Library,
}

/// The view the Mini arrangement paints (design D8): the mounted pick, except
/// while the Playlists screen shows. That screen takes the Library panel's
/// slot, and Mini paints that slot on the Library pick only, so the screen
/// would open invisibly in the Queue view otherwise — its control and the
/// `playlists_open` action reach it in every arrangement, and the mounted pick
/// changes only on a destination press. The pick stays untouched, so closing
/// the screen returns to it.
#[must_use]
pub(crate) fn painted_view(view: MiniView, playlists_open: bool) -> MiniView {
    if playlists_open {
        MiniView::Library
    } else {
        view
    }
}

/// Whether the Queue column shows (design D8): every arrangement but Mini
/// shows it beside the Library panel, and Mini shows the view's pick.
#[must_use]
pub(crate) fn queue_column_shows(
    panel_shows: bool,
    arrangement: GuiArrangement,
    view: MiniView,
) -> bool {
    !panel_shows || arrangement != GuiArrangement::Mini || view == MiniView::Queue
}

/// Whether the Library panel shows (design D8): its content must be present,
/// and Mini shows it only while the view picks it.
#[must_use]
pub(crate) fn library_column_shows(
    panel_shows: bool,
    arrangement: GuiArrangement,
    view: MiniView,
) -> bool {
    panel_shows && (arrangement != GuiArrangement::Mini || view == MiniView::Library)
}

/// The window's answer to a header destination press (design D8): in Mini the
/// Library panel takes the Queue column's place. While the Library shows, a
/// press on the already active destination's own control — its tab, or the
/// Home icon while the Home content is present — puts the Queue back, so the
/// active destination's control is the Queue/Library toggle there. Every other
/// arrangement paints both columns, so the press leaves the view alone.
pub(crate) fn destination_pressed(
    arrangement: GuiArrangement,
    view: &mut Signal<MiniView>,
    pressed: usize,
    current_tab: Option<usize>,
    home_active: bool,
) {
    if arrangement != GuiArrangement::Mini {
        return;
    }
    let already_active = if pressed == HOME_POSITION {
        home_active
    } else {
        current_tab == Some(pressed)
    };
    let showing_library = *view.peek() == MiniView::Library;
    view.set(if showing_library && already_active {
        MiniView::Queue
    } else {
        MiniView::Library
    });
}

/// The global header's facts (design D7): the presented destination tabs, the
/// active one, and whether the Home content is present. The header paints the
/// same sections the Library panel narrows for its lists, so the two cannot
/// name different destinations.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HeaderFacts {
    /// The presented destination tabs in catalog order.
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position, or `None` while no tab is active.
    pub current_tab: Option<usize>,
    /// Whether the Home content is present, which the Home icon marks.
    pub home_active: bool,
}

/// Read the header's facts from the sections the frame carries.
#[must_use]
pub(crate) fn header_facts(sections: &Sections) -> HeaderFacts {
    let list = sections.library_list.cloned();
    let detail = sections.library_detail.cloned();
    HeaderFacts {
        tabs: list
            .as_ref()
            .map(|list| list.tabs.clone())
            .unwrap_or_default(),
        current_tab: list.as_ref().and_then(|list| list.current_tab),
        home_active: library_pair(list.as_deref(), detail.as_deref())
            .is_some_and(|pair| matches!(pair, LibraryPair::Home(..))),
    }
}

/// The Queue column's class for its one root slot. Two columns pin its width;
/// a queue-only panel fills the window. A queue-only panel never paints the
/// collapsed rail (the `Panel` gates the collapse toggle on the Library
/// panel beside it), so no class end-aligns a rail.
#[must_use]
fn queue_column_class(two_columns: bool) -> &'static str {
    if two_columns {
        "flex h-full min-h-0 flex-col flex-none"
    } else {
        "flex h-full min-h-0 flex-col flex-1"
    }
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
/// The id the window's root element carries (`main`, D8).
///
/// The shell delivers a keydown to the document's focused node, and this
/// window's one `keydown` handler lives on the root: a key with nothing
/// focused falls back to the document's `<html>` element, which carries no
/// Dioxus id, so the event reaches no handler. Blitz clears the DOM focus on
/// a click that matches no text input, checkbox, radio, summary, label or
/// link (blitz-dom's `handle_click`: `if !matched { clear_focus() }`) — every
/// queue row is such a click — and it drops the focus with a removed node
/// (a closed search field). The launch copy finds the root by this id and
/// gives the keyboard back (`window::launch::restore_root_focus`).
pub(crate) const ROOT_ELEMENT_ID: &str = "mbv-root";

#[component]
pub fn App() -> Element {
    let window = use_window();
    let window_id = window.id();
    let slot = use_hook(consume_context::<EventSlot>);
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
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
    // The pointer target (gui-pointer-target-interaction design D1): the one
    // value a GUI key's routing reads. The launch copy writes it after each
    // window event from Blitz's hover node, and its readers peek only — no
    // component subscribes, so a hover change re-renders nothing.
    let pointer_target = use_signal(PointerTarget::default);
    provide_context(pointer_target);
    let pointer_target_store = consume_context::<PointerTargetStore>();
    use_hook(move || pointer_target_store.provide(pointer_target));

    // The sync on mount (D6): the window asks for the full snapshot once its
    // section listener is subscribed. The drain above is that listener; both
    // live for the window's life, as `App`'s `onMount` set them up.
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
        reset_settings.sync_settings_frame(frame.as_deref());
    });

    // The panel container the layout report carries the size of (D9): the
    // box the panel states are picked from. The launch copy writes a resize's
    // size into it before the frame that resolves at that size, and the effect
    // writes the measured one after each frame a change resolved; the
    // measurement reports to the core and hands to `Panel`, so the state the
    // window paints and the capacity and hit map the core applies come from one
    // size.
    let viewport_size = use_hook(consume_context::<ViewportSize>);
    let container = use_container_size(viewport_size, &window);

    // The window channel's drain (D4): every event from outside the tree
    // dispatches here.
    use_window_drain(
        Drain {
            proxy: use_hook(consume_context::<BlitzShellProxy>),
            window_id,
            scale_window: Arc::clone(&window),
            scale,
            epoch,
            art_events,
            fired,
            sections: sections.clone(),
            playhead: playhead.clone(),
            library: library.clone(),
        },
        slot,
    );

    let mut container_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_container_size(container_mounted, container);

    // The two-panel arrangement (add-gui-library-panel design D4): the Queue
    // panel beside the Library panel, picked from the measured Library column
    // width — the window width minus the Queue panel's current width. The
    // Queue panel's column and the Wide list column are the session's widths
    // (design D4): the dividers and the arrangement read one resolution.
    let connected = sections.connected.cloned();
    let attach = sections.attach.cloned();
    let settings_open = sections.settings.read().is_some();
    let overlay = sections.overlay.cloned();
    let panel = sections.panel.cloned();
    let playlists_open = sections.playlists.read().is_some();
    let has_library_list = sections.library_list.read().is_some();
    let container_size = container.cloned();
    let widths = resize::panel_widths(container_size, &panel, &widths_store.dragged());
    let arrangement = library_arrangement(container_size, widths.queue);
    // The Settings screen's own measure: its fixed 260 px sidebar, never the
    // dragged Queue column's width (design D11).
    let settings_screen_arrangement = settings_arrangement(container_size);
    // The Library panel's one presence fact (design D2, D10, D12): the
    // window holds the active destination's content, whose painters the
    // panel mounts. The key router gates on the same fact. (`libraryShown`
    // of `lib/library/state.ts`; its port lands with task 6.1.)
    let panel_shows = has_library_list || playlists_open;
    // The panel row's two columns (design D8): both in every arrangement but
    // Mini, which shows the Mini view's pick. The header's destination press
    // flips that pick while Mini holds; the Playlists screen is the Library
    // pick while it shows, whichever view is mounted.
    let mini_view = use_signal(MiniView::default);
    let mini = painted_view(mini_view.cloned(), playlists_open);
    let queue_visible = queue_column_shows(panel_shows, arrangement, mini);
    let library_visible = library_column_shows(panel_shows, arrangement, mini);
    let two_columns = queue_visible && library_visible;
    // The global header's facts (design D7) and its press (design D8): the
    // header reports which destination it opened, and the window flips the
    // same view the panel row reads. The active facts the press reads are this
    // render's, so the toggle answers the destination the frame carries.
    let header = header_facts(&sections);
    let header_current_tab = header.current_tab;
    let header_home_active = header.home_active;
    let mut opened_mini_view = mini_view;
    let on_destination_opened = Callback::new(move |pressed: usize| {
        destination_pressed(
            arrangement,
            &mut opened_mini_view,
            pressed,
            header_current_tab,
            header_home_active,
        );
    });
    // The container's backdrop holds one fill at every focus state (user
    // decision 2026-11-15), unlike the TUI's focus-reactive
    // `panel_appearance_focus` pick.
    let queue_canvas = canvas_var("queue-column");
    // Every handler holds its own link to the core (D4).
    let key_link = CoreLink::clone(&link);
    let press_link = CoreLink::clone(&link);
    // The press path's Library store (gui-collection-dropdown design D7): a
    // press anywhere in the window outside the collection dropdown closes it,
    // and the dropdown's own region stops the presses that land on it.
    let press_library = library.clone();
    let release_link = CoreLink::clone(&link);
    let move_link = CoreLink::clone(&link);
    let wheel_link = CoreLink::clone(&link);
    let cancel_link = CoreLink::clone(&link);
    let leave_link = CoreLink::clone(&link);
    // The divider drag's handlers hold their own copies (D4).
    let mut drag_release = divider_drag;
    let mut drag_cancel = divider_drag;
    let drag_move = divider_drag;
    // The drag's width callback (D14): every step clamps and saves, as the
    // the former client `dragQueueWidth` did. It needs no layout bump: the step writes the
    // width the tree paints, so the frame that resolves the new inline width
    // reports the change and the measure follows (D9).
    let mut drag_widths = widths_store.clone();
    let mut reset_widths = widths_store;
    let drag_container = container;

    rsx! {
        {root_style_element()}
        ThemeStyle {}
        main {
            id: "{ROOT_ELEMENT_ID}",
            class: "h-dvh w-full overflow-hidden bg-background text-foreground",
            tabindex: 0,
            onmounted: move |event| root_mounted.set(Some(event)),
            onkeydown: move |event: KeyboardEvent| {
                // Every keydown meets the window content's router first,
                // which takes the keys it handles and lets the forwarder send
                // the rest (add-gui-library-panel design D3). The Settings
                // steps run ahead of the Library ones (row 8.1); both read
                // the window's section stores and the interaction stores.
                let router = window_key_router(
                    library.clone(),
                    settings_state.clone(),
                    sections.clone(),
                    arrangement,
                    pointer_target,
                );
                input::on_keydown(&event, &router, &key_link);
            },
            onpointerdown: move |event: PointerEvent| {
                // A press outside the collection dropdown closes it and clears
                // its filter text, a press outside the Shows view popover or
                // the season dropdown closes it, and a press outside the
                // collapsed Search control's panel closes it, wherever in the
                // window the press lands (gui-collection-dropdown design D7,
                // D8; gtds design D15; user request 2026-11-19). The control's
                // own surface stops this handler, so a press inside acts on
                // the control. The close changes no pointer fact, so the press
                // still reaches the core.
                press_library.close_popovers();
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
                // before the move forwards to the core, as the former client
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
            // No `onblur` here: Blitz blurs this root whenever a click moves
            // the focus off it (every press on a Queue row does), so a blur
            // is not the window losing the pointer, and forwarding it as a
            // `cancel` erased the core's double-click pair on every press.
            if connected && attach.is_none() {
                div {
                    // The window's one column (design D7): the global header
                    // above the panel row, in every arrangement. The panel row
                    // keeps the box the arrangement measures, so the header's
                    // 56 px band sits outside that measure.
                    class: "flex h-full min-h-0 w-full min-w-0 flex-col",
                    if !settings_open {
                        LibraryTopBar {
                            tabs: header.tabs.clone(),
                            // The Playlists screen is the selected destination
                            // while it shows: no tab and not Home marks itself.
                            current_tab: if playlists_open { None } else { header.current_tab },
                            home_active: header.home_active && !playlists_open,
                            playlists_active: playlists_open,
                            on_destination_opened: Some(on_destination_opened),
                            mini: arrangement == GuiArrangement::Mini,
                        }
                    }
                    div {
                        class: "relative flex min-h-0 flex-1 w-full min-w-0 overflow-hidden",
                        "data-arrangement": arrangement.name(),
                        style: "background: {queue_canvas};",
                        onmounted: move |event| container_mounted.set(Some(event)),
                        if settings_open {
                            // The Settings screen replaces the two panels while
                            // its section is present (add-gui-settings-screen
                            // design D11): no player panel, queue, or Library
                            // panel mounts, and playback continues. The screen's
                            // Mini check measures the window minus its fixed
                            // 260 px sidebar, which the root measures separately
                            // from the Library panel's own arrangement.
                            SettingsScreen { arrangement: settings_screen_arrangement }
                        } else {
                            // Paint order, left to right: Library panel, Queue
                            // divider, Queue column (design D1).
                            {library_column(
                                library_visible,
                                playlists_open,
                                has_library_list,
                                arrangement,
                                widths,
                            )}
                            if two_columns && !panel.collapsed {
                                // The Queue panel's divider (design D4): dragging
                                // it resizes the Queue column and, with it, the
                                // Library column the arrangement measures; a
                                // double-click returns the canvas width.
                                PanelDivider {
                                    side: DividerSide::Right,
                                    name: DividerName::Queue,
                                    width: widths.queue,
                                    on_drag: move |width| {
                                        drag_widths
                                            .drag_queue_width(width, drag_container.cloned().width);
                                    },
                                    on_reset: move |()| {
                                        reset_widths.reset_queue_width();
                                    },
                                }
                            }
                            {queue_column(
                                queue_visible,
                                two_columns,
                                queue_column_class(two_columns),
                                widths.queue,
                                container,
                                library_visible,
                            )}
                        }
                    }
                }
                {overlay_layer(overlay.as_ref())}
                // The floating toast cards (design D7 of `gui-floating-toasts`):
                // mounted last, so they paint over the panels, the Settings
                // screen, and the overlays, and nothing while none is live.
                ToastStack {}
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

mod render;

#[cfg(test)]
mod tests;
