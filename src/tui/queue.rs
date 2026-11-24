use super::Model;
use super::{PanelFocus, QueueLink};
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{FooterPill, QueueSurfaceProjection};
use mbv_components::QueueComponent;
use mbv_tui_support::ToRatatui;
use mbv_ui_model::playback_target::NowPlayingStatus;
use mbv_ui_msg::{ComponentId, QueueColumnResize, QueueIntent, QueueRequest};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

impl Model {
    pub(in crate::tui) fn sync_queue(&mut self) {
        // D2 (`hide-empty-queue-column`): when the queue column is hidden
        // because the displayed queue emptied, the stored focus cannot stay
        // on Queue -- it would resurface when the queue refills and yank
        // focus away from the library the user is browsing. The write skips
        // `set_panel_focus`'s queue initial-item path for a queue that is
        // about to be left.
        if self.app.queue_column_hidden_empty()
            && self.app.stored_panel_focus() == PanelFocus::Queue
        {
            self.app.leave_emptied_queue_focus();
        }
        self.mount_queue_panel();
        // The projection lives once on the `App` (extract-client-core design
        // D5): the TUI and the GUI Clients consume the same update.
        let update = self.app.queue_surface_projection();
        // The visual slot's image projection (task 3.4, D9): the queue
        // projection — not the painter — issues every fetch for the now-playing
        // item and projects the slot's image state. The slot only paints while
        // playback is active (idle collapse), so the projection follows the
        // same gate the card's render used.
        if self.app.visual_slot_shown() {
            let resizing = self.refresh_resize_drag_hold();

            let record = crate::app::state::projection::card::refresh_queue_card_image(
                &mut self.app,
                resizing,
            );
            if let Some(record) = record {
                self.ensure_queue_card_title_overlay(&record, resizing);
            }
        } else if self.app.now_playing_status() != NowPlayingStatus::Idle {
            // The slot is hidden while playing (the user's hide toggle): the
            // artwork can no longer carry the title, so the header is its
            // only home again — the carried-over site must not leave the
            // header on its brand row over a slot that paints nothing.
            self.app.move_queue_card_title_to_header();
        }
        self.push_queue_projection(update);
    }

    /// Build the artwork's title-overlay variant from the sync pass's title-site
    /// record (invariant 16): the overlay's encoding is a terminal protocol, so
    /// it belongs to the shell. The core decides the site and keeps the plain
    /// base key as the paint's fallback while the variant's encode completes;
    /// a built variant replaces the projected key.
    ///
    /// Invariant 20: a resize drag builds nothing; the settled width composes
    /// it once the hold lifts.
    pub(in crate::tui) fn ensure_queue_card_title_overlay(
        &mut self,
        record: &crate::app::state::projection::card::QueueTitleRecord,
        resizing: bool,
    ) {
        self.app.set_queue_card_plain_key(record.base_key.clone());
        if resizing {
            return;
        }
        let (height, width) = self.app.last_card_size();
        let variant = self.ensure_title_overlay_protocol(
            &record.base_key,
            (width, height),
            &record.parts,
            record.logo_cache_key.as_deref(),
            &record.item_kind,
        );
        if let Some(variant) = variant {
            self.app.set_queue_card_variant_key(variant);
        }
    }

    fn mount_queue_panel(&mut self) {
        let id = ComponentId::Queue;
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(QueueComponent::new()), vec![])
                .expect("mount Queue");
            // The mount routes through the core focus stack (design D3): the
            // record joins the mounted set, and the sync pass's focus step
            // applies the derived focus — Queue while the Panel focus is
            // Queue and no overlay holds the stack.
            self.focus_stack.mount(id.clone());
        }
    }

    fn push_queue_projection(&mut self, update: QueueSurfaceProjection) {
        let id = ComponentId::Queue;
        let terminal_size = self.terminal_size();
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>()
        {
            queue.set_pending_slot(update.pending_slot);
            // The shown entry changed: drop the marks and the viewport before
            // the start-slot cursor lands (design D7 of `gui-named-queues`).
            if update.presentation_reset {
                queue.reset_presentation();
            }
            if let Some(slots) = update.slots {
                queue.set_rows(&slots, update.playback);
            } else if let Some((target, row)) = update.patch {
                queue.set_row_patch(target, row);
            }
            queue.set_cursor(&update.cursor);
            queue.set_link(update.link);
            // The footer pills are all queue concern (owner indicator, playlist
            // source, autosave prefix, dirty suffix) — never the library
            // column's status bar. The owner pill always names the shown
            // Player owner (design D7), so it flips in the same sync pass the
            // shown link flips; the playback target's host lives on the queue
            // header row.
            let footer = self.app.queue_footer_projection();
            queue.set_status_pills(
                footer_pill_spans(&footer.owner),
                footer_pill_spans(&footer.playlist),
                footer.autosave.as_ref().map(footer_pill_spans),
                footer.dirty.as_ref().map(footer_pill_spans),
            );
            // The queue panel's placement (task 3.1): computed from the
            // same paint-free checkpoint the draw path consumes (the last
            // published card geometry; see `draw::queue_panel_placement`).
            // The frame is an appearance surface: unpinned mini view always
            // rests; a pinned mini view follows the window focus, while 80+
            // columns ignores window focus.
            queue.set_frame_focused(self.app.panel_appearance_focus(PanelFocus::Queue));
            queue.set_mini_view(self.app.is_mini_view());
            queue.set_area(
                super::draw::queue_panel_placement(
                    &self.app,
                    self.layout.card.height,
                    terminal_size,
                )
                .panel_area,
            );
        }
    }

    pub(in crate::tui) fn render_queue_boundary_at(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
    ) {
        let id = ComponentId::QueueBoundary;
        if self.application.mounted(&id) {
            self.application.view(&id, frame, area);
        }
    }

    pub(in crate::tui) fn render_queue_panel_at(
        &mut self,
        frame: &mut ratatui::Frame,
        placement: ratatui::layout::Rect,
    ) {
        let id = ComponentId::Queue;
        if !self.application.mounted(&id) {
            return;
        }
        // The queue panel paints its whole surface (frame, title, status,
        // list) at the placement computed by the root loop (task 3.1).
        if placement.width == 0 || placement.height == 0 {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>()
        {
            queue.set_area(placement);
        }
        self.application.view(&id, frame, placement);
    }

    pub(in crate::tui) fn handle_queue_request(&mut self, request: QueueRequest) {
        // The dispatch lives on the `App` (extract-client-core design D5), so
        // both Clients run one set of arms; the shell adds nothing to it.
        self.app.handle_queue_request(request);
    }

    pub(in crate::tui) fn handle_queue_intent(&mut self, intent: QueueIntent) {
        match intent {
            QueueIntent::Clear => self.app.request_clear_queue(),
            QueueIntent::ResizeColumn(direction) => {
                if self.app.effective_panel_mode() == super::PanelMode::Both {
                    self.app
                        .resize_queue_column(direction == QueueColumnResize::Wider);
                }
            }
            QueueIntent::PlayNow => {
                let (active, current_idx) = {
                    let status = self.app.player_status_snapshot();
                    (status.active, status.current_idx)
                };
                if active {
                    self.app.move_queue_cursor_to_playing_slot(current_idx);
                    // Jump-to-now-playing is an explicit, authoritative move.
                    self.app.reanchor_queue_cursor_to_playing();
                } else {
                    self.app
                        .flash("Nothing is playing".into(), ToastSeverity::Error);
                }
            }
            QueueIntent::SavePlaylist => {
                if self.app.local_queue_len() > 0 {
                    self.app
                        .open_save_playlist_dialog(super::SavePlaylistDialog {
                            input: self.app.queue_playlist_name().to_string(),
                            stage: super::SavePlaylistStage::EnterName,
                        });
                }
            }
            QueueIntent::Navigate { link, slot_id } => {
                let Some(cursor) = self.select_queue_slot(link, slot_id) else {
                    return;
                };
                let Some(item) = self.app.queue_for_link(link).emby_item_at(cursor) else {
                    return;
                };
                let item_id = item.id.clone();
                let item_type = item.item_type.clone();
                let libs = self.app.emby_library_identities();
                self.app.spawn_navigate_to_item(item_id, item_type, libs);
            }
        }
    }

    /// Resolves `slot_id` to its index in `link`'s queue and applies the
    /// link/focus/hold-window side effects, returning the resolved index.
    /// The index is the operand for the shell-owned effect the caller is
    /// about to run (D2: `remove_from_queue(index)`, `move_queue_item_up/
    /// down(index)`, `AppCommand::QueuePlayCursor(index)`). The component's
    /// own cursor is authoritative for selection; App's `queue_cursor` is
    /// not written here (task 3.1: the mirror is gone).
    fn select_queue_slot(
        &mut self,
        link: QueueLink,
        slot_id: mbv_queue::QueueSlotId,
    ) -> Option<usize> {
        self.app.select_queue_slot(link, slot_id)
    }
}

/// One footer pill as the TUI footer paints it: the pill's text in its
/// foreground role on the status pill surface. The TUI painter edge builds
/// these spans from the neutral footer projection (design D5).
fn footer_pill_spans(pill: &FooterPill) -> Vec<Span<'static>> {
    let mut style =
        Style::default()
            .fg(pill.role.color().to_ratatui())
            .bg(
                mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                    .fill
                    .to_ratatui(),
            );
    if pill.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    vec![Span::styled(pill.text.clone(), style)]
}

#[cfg(test)]
mod tests;
