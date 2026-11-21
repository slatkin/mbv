//! The Settings screen (`replace-tauri-with-blitz` task 8.3, the port of
//! `SettingsScreen`; add-gui-settings-screen design D11, D13, ADR
//! 0037): the sidebar at the left window edge and the shown section's
//! content in the rest of the window, with the status line closing the
//! screen. The sidebar is the canvas's fixed 260 px, independent of the
//! Queue column's dragged width and its collapse toggle, and paints no
//! divider. Below the `libraryArrangement` Mini threshold only the pane that
//! holds the keyboard shows. The content is the canvas `main`: 32 px by 48 px
//! padding, a 1120 px maximum width, and the section's name as its 24 px
//! title. The root carries `min-w-0 flex-1`: the window root gives it a row
//! of a flex container, so without them the screen shrinks to its content
//! width instead of taking the canvas (live finding, Settings screen).
//!
//! The screen root owns the pointer press the former client `window` listener ran:
//! it hides the settings cursor, and it closes the open pick menu — the
//! press-outside close of `SettingsPickButton` (task 8.3's wiring
//! requirement). The pick button and its menu stop the press's propagation,
//! so a press that reaches this root is by construction outside them; their
//! own handlers hide the cursor first (`settings_pick_button.rs`'s
//! `press_inside`), because stopping alone would swallow the cursor hide.
//!
//! the former client screen's two `scrollIntoView` effects (keep the walked cursor
//! row in view) port as the cursor row's own reveal (D15's scroll rule):
//! Blitz exposes no descendant query from an effect, so every row that paints
//! `data-settings-cursor` keeps a mounted handle and scrolls itself to the
//! same block and inline positions (`components/cursor_reveal.rs`).

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::window::arrangement::{GuiArrangement, SETTINGS_SIDEBAR_WIDTH_PX};
use crate::window::components::settings_feeds::SettingsFeeds;
use crate::window::components::settings_keys::SettingsKeys;
use crate::window::components::settings_library::SettingsLibrary;
use crate::window::components::settings_rows::SettingsRows;
use crate::window::components::settings_services::SettingsServices;
use crate::window::components::settings_sessions::SettingsSessions;
use crate::window::components::settings_sidebar::SettingsSidebar;
use crate::window::components::settings_status_bar::SettingsStatusBar;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::entries::SettingsEntry;
use crate::window::settings::state::{
    SettingsPane, SettingsState, current_settings_entries, cursor_entry_id,
};

/// Each section's sidebar and title label. The
/// Services section shows as Sources at the user's choice (design D13, the
/// *Sources label* bullet): only the label changes.
#[must_use]
pub(crate) fn section_label(section: GuiSettingsSection) -> &'static str {
    match section {
        GuiSettingsSection::Services => "Sources",
        GuiSettingsSection::Playback => "Playback",
        GuiSettingsSection::Display => "Display",
        GuiSettingsSection::Session => "Session",
        GuiSettingsSection::Library => "Library",
        GuiSettingsSection::Queue => "Queue",
        GuiSettingsSection::Mpv => "mpv",
        GuiSettingsSection::Feeds => "Feeds",
        GuiSettingsSection::Actions => "Actions",
        GuiSettingsSection::Keys => "Keys",
        GuiSettingsSection::Sessions => "Sessions",
    }
}

/// The wire name of one section, as the settings command names it: the value
/// the sidebar's `data-settings-section` paints and the commands send.
#[must_use]
pub(crate) fn section_wire(section: GuiSettingsSection) -> &'static str {
    match section {
        GuiSettingsSection::Services => "services",
        GuiSettingsSection::Playback => "playback",
        GuiSettingsSection::Display => "display",
        GuiSettingsSection::Session => "session",
        GuiSettingsSection::Library => "library",
        GuiSettingsSection::Queue => "queue",
        GuiSettingsSection::Mpv => "mpv",
        GuiSettingsSection::Feeds => "feeds",
        GuiSettingsSection::Actions => "actions",
        GuiSettingsSection::Keys => "keys",
        GuiSettingsSection::Sessions => "sessions",
    }
}

/// The Settings screen's one frame read (the screen's and every section
/// component's shared view): the shown frame, its entries with the held
/// choice marks applied, and the entry id the content cursor rests on while
/// the content pane holds the keyboard. The section components read this
/// instead of taking the frame and its rows as props, so no repaint clones a
/// frame's rows through a prop (the port's store-read rule).
pub(crate) struct SettingsSectionView {
    /// The shown frame, held as the one handle every reader of it clones
    /// (`share-gui-sections-by-rc` design D3).
    pub frame: SectionRef<GuiSettingsFrame>,
    /// The section's entries with the held marks applied.
    pub entries: Vec<SettingsEntry>,
    /// The entry id the content cursor rests on, or `None` while the sidebar
    /// holds the keyboard or the cursor does not paint.
    pub cursor_id: Option<String>,
}

/// The two-column grid every section pane's cards sit in (design D13, the
/// canvas's Services grid). The Library pane stacks its own cards in one
/// column, because it paints the Rows pane first.
pub(crate) const SETTINGS_PANES_GRID_STYLE: &str =
    "grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 20px; align-items: start;";

/// The prologue every section pane runs (design D13): the two stores it
/// reads, its shown frame, the press its controls call, and the content
/// cursor's entry id.
pub(crate) struct SettingsPaneScaffold {
    /// The section stores.
    pub sections: Sections,
    /// The Settings store.
    pub settings: SettingsState,
    /// The shown frame.
    pub frame: SectionRef<GuiSettingsFrame>,
    /// A control's press: the entry's activation through the section's full
    /// entry list, the same one the keyboard's Enter runs.
    pub onpress: Callback<String>,
    /// The entry id the content cursor rests on.
    pub cursor_id: Option<String>,
}

/// The panes' shared prologue: consume the two contexts, read
/// [`settings_section_view`], and build the press over the view's entries.
/// A pane returns an empty element while no frame is shown.
#[must_use]
pub(crate) fn settings_pane() -> Option<SettingsPaneScaffold> {
    let sections = consume_context::<Sections>();
    let settings = consume_context::<SettingsState>();
    let view = settings_section_view(&sections, &settings)?;
    let press_settings = settings.clone();
    let press_entries = view.entries;
    let onpress = Callback::new(move |id: String| {
        press_settings.press_settings_entry(&press_entries, &id);
    });
    Some(SettingsPaneScaffold {
        sections,
        settings,
        frame: view.frame,
        onpress,
        cursor_id: view.cursor_id,
    })
}

/// The view the screen's components paint.
pub(crate) fn settings_section_view(
    sections: &Sections,
    settings: &SettingsState,
) -> Option<SettingsSectionView> {
    let frame = sections.settings.cloned()?;
    let ui = settings.ui.cloned();
    let entries = current_settings_entries(&frame.content, &ui);
    let cursor_id = if ui.pane == SettingsPane::Content && ui.keyboard {
        cursor_entry_id(&entries, &ui)
    } else {
        None
    };
    Some(SettingsSectionView {
        frame,
        entries,
        cursor_id,
    })
}

/// The props of the screen (the `SettingsScreen` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsScreenProps {
    /// The arrangement the Library-column measurement picks; only Mini
    /// changes the paint (one pane instead of two).
    pub arrangement: GuiArrangement,
}

/// The one Settings screen.
#[component]
pub fn SettingsScreen(props: SettingsScreenProps) -> Element {
    let sections = consume_context::<Sections>();
    let settings = consume_context::<SettingsState>();
    let status = sections.status.cloned();
    let Some(view) = settings_section_view(&sections, &settings) else {
        return rsx! {};
    };
    let label = section_label(view.frame.section);
    let mini = props.arrangement == GuiArrangement::Mini;
    let pane = settings.ui.cloned().pane;
    let press_settings = settings;
    rsx! {
        div {
            class: "flex h-full min-h-0 min-w-0 flex-1 flex-col",
            style: "background: var(--canvas-library-column);",
            "data-settings-screen": "",
            "data-settings-arrangement": props.arrangement.name(),
            // A pointer press hides the keyboard cursor, like
            // `:focus-visible`, and closes the open pick menu: the former client
            // `window` `pointerdown` listeners, owned here where the presses
            // meet (see the module doc).
            onpointerdown: move |_| screen_press(&press_settings),
            div {
                class: "flex min-h-0 flex-1",
                if !mini || pane == SettingsPane::Sidebar {
                    div {
                        class: if mini {
                            "flex h-full min-h-0 flex-col min-w-0 flex-1"
                        } else {
                            "flex h-full min-h-0 flex-col flex-none"
                        },
                        style: if mini {
                            String::new()
                        } else {
                            format!("width: {SETTINGS_SIDEBAR_WIDTH_PX}px;")
                        },
                        "data-settings-sidebar-pane": "",
                        SettingsSidebar {}
                    }
                }
                if !mini || pane == SettingsPane::Content {
                    div {
                        class: "h-full min-h-0 min-w-0 flex-1 overflow-y-auto",
                        style: "box-sizing: border-box; padding: 32px 48px; background: var(--canvas-library-column);",
                        "data-settings-content": "",
                        div {
                            class: "flex flex-col",
                            style: "max-width: 1120px; gap: 24px;",
                            h1 {
                                style: "margin: 0; font-size: 24px; font-weight: 600; color: var(--canvas-heading);",
                                "data-settings-content-title": "",
                                "{label}"
                            }
                            { content_pane(&view.frame) }
                        }
                    }
                }
            }
            SettingsStatusBar { status, section: label }
        }
    }
}

/// The shown section's content pane: one section component per content kind, each reading the
/// section stores itself.
fn content_pane(frame: &GuiSettingsFrame) -> Element {
    match &frame.content {
        GuiSettingsContent::Rows { .. } => rsx! { SettingsRows {} },
        GuiSettingsContent::Library { .. } => rsx! { SettingsLibrary {} },
        GuiSettingsContent::Services { .. } => rsx! { SettingsServices {} },
        GuiSettingsContent::Feeds { .. } => rsx! { SettingsFeeds {} },
        GuiSettingsContent::Keys { .. } => rsx! { SettingsKeys {} },
        GuiSettingsContent::Sessions { .. } => rsx! { SettingsSessions {} },
    }
}

/// A pointer press on the screen root (`SettingsScreen`'s
/// `svelte:window` `pointerdown`, plus the pick menu's press-outside close):
/// the open pick menu and the feed form's open kind menu close with no send,
/// and the settings cursor hides. The pick controls and the kind field stop
/// their presses' propagation before here, so a press that arrives is outside
/// them.
fn screen_press(settings: &SettingsState) {
    if settings.pick_menu.peek().is_some() {
        settings.close_pick_menu();
    }
    if settings
        .ui
        .peek()
        .feed_form
        .as_ref()
        .is_some_and(|form| form.kind_menu.is_some())
    {
        settings.close_feed_kind_menu();
    }
    settings.hide_settings_cursor();
}

#[cfg(test)]
mod tests;
