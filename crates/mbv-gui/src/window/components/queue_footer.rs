//! The Queue footer (task 5.6, the port of `QueueFooter`): one 40 px
//! band at the queue column's bottom, with the canvas's top border and side
//! padding, that merges what the TUI paints as the queue footer's status
//! pills and the Status bar's prefix and toast rows — the queue source, the
//! session marker, the prefix-armed state, and the toasts.
//!
//! The left group is the queue source: the autosave indicator (while
//! save-on-consume is on for a saved playlist), then the playlist text. The
//! canvas paints the source as plain text with no leading icon, and the
//! text reports no hit region — the GUI presents no playlists surface, so a
//! press on it would have nowhere to go, and a reported target that can
//! never fire is a lie in the layout report. (The TUI's pill opens its
//! Playlists overlay.) The right group is the dirty indicator (while the
//! queue differs from its saved playlist), then the session marker: the
//! footer owner pill's label — the Player-owner label (`woims` on the
//! canvas) — then its liveness dot, painted only while the shown Owner's
//! link is up (`ownerLive`, design D4a: the dot marks the live Owner). The
//! canvas's owner marker carries no leading icon either.
//!
//! The prefix-armed state paints the same `PREFIX` badge the TUI's status
//! row paints while prefix mode is armed, and it drops while a toast is
//! live — the TUI's pill-drop rule carried into the line. The toast paints
//! inline in the line, centred in its severity colour: the spec permits the
//! footer to show toasts in the line or in a conventional GUI form. A live
//! toast moves no layout: the line is always painted and the toast only
//! fills its centre span. The centre is empty while the projection holds
//! none of those states, as the canvas draws it.
//!
//! The armed prefix and the live toast are each one fact, and each paints
//! once a frame (design D10): while the Library panel shows, its Library
//! Status bar replaces its hints with them, so this line paints neither and
//! the band under the Library column owns them. While the Library panel
//! does not show, this line paints them as the TUI's status row does.
//!
//! The band paints the canvas's type and border (mono 12 px, a 1 px top
//! border in the queue-local `--canvas-border` token) over the queue column's
//! backdrop: the `queueFooterBand` name it carries is not part of the
//! window's `--mbv-*` token set, so the band paints no fill of its own
//! (`window/theme.rs::write_chrome` holds the rule and the reason), as the
//! reference build painted it. The text keeps the secondary-title token
//! (`--canvas-type-secondary`, the queue source is a secondary title) and
//! owner-marker sand (`--canvas-owner`) in place of the pills' theme roles.
//! The slice's `GuiStatusBar` carries no volume, mute, or visual-mode
//! content, so the line paints no interactive status pill and reports no
//! region at all — a point on the line resolves to nothing. It still
//! reports through the shared layout reporter so the merged report accounts
//! for the mounted surface.

use dioxus_native::prelude::*;

use crate::window::icons::{SaveCheck, SaveOff};
use crate::window::layout::{LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter};
use crate::window::pointer_target::PointerZone;
use crate::window::sections::Sections;
use crate::window::status::toast_color;
use crate::window::theme::role_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the Queue footer (the `QueueFooter` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueFooterProps {
    /// Whether the Library panel shows beside this one: the Library Status
    /// bar owns the armed prefix and live toast then, so the footer line
    /// paints neither.
    #[props(default)]
    pub library_visible: bool,
}

/// The Queue footer: the canvas footer band, painted from the `footer` and
/// `status` sections. Every paint that moved a region reports — the line's
/// contribution carries no region, so the reporter's dedupe holds its one
/// report for the surface's life.
#[component]
pub fn QueueFooter(props: QueueFooterProps) -> Element {
    let sections = consume_context::<Sections>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);

    // No region outlives the band: the contribution retracts on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    let watch_footer = sections.footer;
    let watch_status = sections.status;
    let watch_epoch = epoch.0;
    use_effect(move || {
        let _footer = watch_footer.cloned();
        let _status = watch_status.cloned();
        let _epoch = watch_epoch.cloned();
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            // The painted regions this frame reports: none. The line's pills
            // are informational, so a point on it resolves to no target; the
            // report still accounts for the mounted surface.
            effect_reporter.report_layout(key, LayoutContribution::default());
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
    });

    let footer = sections.footer.cloned();
    let status = sections.status.cloned();
    // The toast this line paints: the section's live toast, or `None` while
    // the Library panel shows and its Status bar paints it (design D10).
    let toast = if props.library_visible {
        None
    } else {
        status.toast.clone()
    };
    // Whether this line paints the armed prefix badge: prefix mode is armed,
    // no toast is live (the line's pill-drop rule), and the Library panel
    // does not show (design D10).
    let prefix = !props.library_visible && status.prefix_armed && toast.is_none();

    rsx! {
        // The canvas footer band: 40 px, 16 px side padding, the canvas's
        // 1 px top border, and the canvas's mono type. The left line is the
        // queue's source text, the right line the owner marker (label then
        // dot), and the centre carries the toast, the prefix badge, and the
        // autosave and dirty states while they are active.
        div {
            class: "flex items-center overflow-hidden font-mono whitespace-nowrap",
            style: "height: 40px; padding-inline: 16px; font-size: 12px; border-top: 1px solid var(--canvas-border); background: var(--mbv-queueFooterBand);",
            "data-footer-line": "",
            // The Queue zone (`gui-pointer-target-interaction` D1) reaches the
            // footer band: it sits under the Queue list box, outside the box's
            // own element, so it carries the marker too.
            "data-pointer-zone": PointerZone::Queue.token(),
            // The left group: the prefix badge while armed (dropped while a
            // toast is live, the TUI's pill-drop rule), the autosave
            // indicator, then the queue's source text — no leading icon, as
            // the canvas paints it.
            div {
                class: "flex items-center",
                style: "gap: 16px;",
                if prefix {
                    span {
                        class: "font-bold",
                        style: "color: var(--mbv-accent);",
                        "PREFIX"
                    }
                }
                if let Some(autosave) = footer.autosave.as_ref() {
                    SaveCheck {
                        size: 14,
                        color: role_var(autosave.role),
                        class: "shrink-0".to_owned(),
                    }
                }
                if !footer.playlist.text.is_empty() {
                    // The queue's source is a secondary title, so it paints
                    // the secondary token (design D14); the window paints
                    // no role here.
                    span {
                        style: "color: var(--canvas-type-secondary);",
                        "data-footer-source": "",
                        "{footer.playlist.text}"
                    }
                }
            }
            // The toast, inline in the line's centre in its severity colour
            // (the footer-line toast form the spec permits); empty while
            // none is live.
            {
                let toast_text = toast
                    .clone()
                    .map_or(String::new(), |toast| toast.text);
                let toast_css = toast
                    .as_ref()
                    .map_or(String::new(), |toast| {
                        format!("color: {};", toast_color(toast.severity))
                    });
                rsx! {
                    Truncate {
                        text: toast_text,
                        class: "min-w-0 flex-1 px-4 text-center".to_owned(),
                        css: toast_css,
                        style: TextStyle::mono(12.0, 400.0),
                    }
                }
            }
            // The right group: the dirty state while present, then the
            // session marker — the owner pill's label then its liveness dot
            // while the shown Owner's link is up — flush at the row's right
            // edge.
            div {
                class: "flex items-center",
                style: "gap: 16px;",
                if let Some(dirty) = footer.dirty.as_ref() {
                    span {
                        class: "flex items-center",
                        style: "gap: 8px;",
                        SaveOff {
                            size: 14,
                            color: role_var(dirty.role),
                            class: "shrink-0".to_owned(),
                        }
                        span {
                            class: dirty.bold.then(|| "font-bold".to_owned()),
                            style: "color: {role_var(dirty.role)};",
                            "{dirty.text}"
                        }
                    }
                }
                // The owner marker, label then dot: the canvas paints the
                // label in its sand token while the dot keeps the liveness
                // mark's own colour.
                span {
                    class: "flex items-center",
                    style: "gap: 6px; color: var(--canvas-owner);",
                    span {
                        "data-owner-label": "",
                        "{footer.owner.text}"
                    }
                    if footer.owner_live {
                        // The live-Owner dot (design D4a): the shown queue's
                        // Player owner answers on a live link.
                        span {
                            style: "color: var(--mbv-statusAvailable);",
                            "data-owner-live": "",
                            title: "Owner live",
                            "aria-label": "Owner live",
                            "●"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
