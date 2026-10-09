use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::playback::PendingQueueAction;
use mbv_ui_model::context_menu::AudiobookshelfMenuTarget;

/// Resolve one target's local apply against the cached progress (design D6).
/// Mark Played applies every target at its cached position (0 when there is
/// no entry); Mark Unplayed resets only targets cached as finished, to 0 —
/// an unfinished or unknown target is the server's no-op and changes nothing
/// locally either. Returns the target's queue content identity and the
/// seconds to reconcile with.
fn audiobookshelf_mark_apply_target(
    app: &App,
    target: &AudiobookshelfMenuTarget,
    finished: bool,
) -> Option<(mbv_queue::QueueItemContentId, f64)> {
    let (content_id, cached) = match target {
        AudiobookshelfMenuTarget::Episode {
            library_item_id,
            episode_id,
        } => (
            mbv_queue::QueueItemContentId::Audiobookshelf {
                library_item_id: library_item_id.clone(),
                episode_id: episode_id.clone(),
            },
            app.audiobookshelf_cached_episode_progress(library_item_id, episode_id)
                .map(|progress| (progress.current_time_seconds, progress.is_finished)),
        ),
        AudiobookshelfMenuTarget::Book { library_item_id } => (
            mbv_queue::QueueItemContentId::AudiobookshelfBook {
                library_item_id: library_item_id.clone(),
            },
            app.audiobookshelf_cached_book_progress(library_item_id)
                .map(|progress| (progress.current_time_seconds, progress.is_finished)),
        ),
    };
    let seconds = if finished {
        cached.map_or(0.0, |(seconds, _)| seconds)
    } else {
        // Mark Unplayed resets only targets cached as finished; an unfinished
        // or unknown target is the server's no-op.
        let (_, cached_finished) = cached?;
        if !cached_finished {
            return None;
        }
        0.0
    };
    Some((content_id, seconds))
}

impl App {
    pub(in crate::app) fn drain_audiobookshelf_events(&mut self) -> bool {
        let mut produced = self.drain_audiobookshelf_worker_events();
        produced |= self.drain_audiobookshelf_setup_event();
        produced |= self.drain_audiobookshelf_catalog_event();
        produced |= self.drain_audiobookshelf_mark_event();
        produced
    }

    /// Drain the finished-state mark worker's completion (design D5). A
    /// completion from a superseded setup generation is dropped whole, the
    /// same gate the shows completion applies on arrival.
    fn drain_audiobookshelf_mark_event(&mut self) -> bool {
        let Some(receiver) = self.setup.audiobookshelf_mark_rx.take() else {
            return false;
        };
        match receiver.rx.try_recv() {
            Ok(completion) if self.audiobookshelf_runtime.accepts(completion.generation) => {
                self.apply_audiobookshelf_mark_completion(completion);
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.setup.audiobookshelf_mark_rx = Some(receiver);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // The mark worker always sends one completion before exiting;
                // a missing message means it died early. Classify it like the
                // other Audiobookshelf worker disconnects.
                self.handle_audiobookshelf_worker_disconnect(receiver.generation);
                true
            }
            Ok(_) => false,
        }
    }

    fn apply_audiobookshelf_mark_completion(
        &mut self,
        completion: crate::app::dispatch::session::service_startup::AudiobookshelfMarkCompletion,
    ) {
        match completion.result {
            // Row 7.3 (standard-media-context-menus): an accepted mark applies
            // locally at this hand-off point. The drain's `produced` report is
            // what re-projects the active Audiobookshelf owner's content (the
            // run loop pushes both ABS owners when the drain produced work).
            Ok(()) => {
                self.apply_audiobookshelf_mark_locally(&completion.targets, completion.finished);
            }
            Err(error)
                if matches!(
                    error.class,
                    mbv_audiobookshelf::AudiobookshelfFailureClass::AuthenticationRejected
                ) =>
            {
                // The existing Audiobookshelf authentication failure
                // classification: fail the Service into NeedsAuthentication
                // and clear the saved credential (the catalog completion's
                // credential-rejection path).
                self.fail_audiobookshelf_service(
                    completion.generation,
                    mbv_core::service_runtime::ServiceState::NeedsAuthentication,
                );
                let _ = self.clear_audiobookshelf_authentication();
                self.flash(
                    format!("Couldn't update Audiobookshelf play state: {error}"),
                    ToastSeverity::Error,
                );
            }
            Err(error) => {
                self.flash(
                    format!("Couldn't update Audiobookshelf play state: {error}"),
                    ToastSeverity::Error,
                );
            }
        }
    }

    /// Apply an accepted mark locally (design D6). Mark Played applies every
    /// target at its cached position (0 when there is no entry); Mark Unplayed
    /// resets only targets cached as finished, to 0. Browse progress goes
    /// through the shared reconcile path for every applied target, including
    /// the actively owned session's item; queue slots get one
    /// `QueueOp::ApplyProgress` through the local queue link, with the active
    /// Player-owned slot left out (spec: "The actively owned session is not
    /// modified by a mark").
    fn apply_audiobookshelf_mark_locally(
        &mut self,
        targets: &[AudiobookshelfMenuTarget],
        finished: bool,
    ) {
        // Resolve every target against the cached progress first: the browse
        // reconcile below mutates the same maps this read walks. Each resolved
        // apply keeps its own target, so a target dropped by the resolve (an
        // unfinished Mark Unplayed no-op) cannot shift the pairing between the
        // remaining targets and their applies (unit 7 review: zip misalignment).
        let mut applies = Vec::with_capacity(targets.len());
        for target in targets {
            if let Some((content_id, seconds)) =
                audiobookshelf_mark_apply_target(self, target, finished)
            {
                applies.push((target, content_id, seconds));
            }
        }
        let mut updates = Vec::with_capacity(applies.len());
        for (target, content_id, seconds) in applies {
            match target {
                AudiobookshelfMenuTarget::Episode {
                    library_item_id,
                    episode_id,
                } => self.reconcile_audiobookshelf_progress(
                    library_item_id,
                    episode_id,
                    seconds,
                    finished,
                ),
                AudiobookshelfMenuTarget::Book { library_item_id } => {
                    self.reconcile_audiobookshelf_book_progress(library_item_id, seconds, finished);
                }
            }
            if self.player_owns_active_match(&content_id) {
                continue;
            }
            updates.push(mbv_ctrl::ProgressUpdate {
                content_id,
                position_ticks: crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks(
                    seconds,
                ),
                finished,
            });
        }
        if updates.is_empty() {
            return;
        }
        let result = {
            let (player, _) = self.queue_link(crate::app::QueueScope::Local);
            player
                .remote()
                .send_queue_op(mbv_remote_player::QueueOp::ApplyProgress { updates })
        };
        if let Err(error) = result {
            self.flash(error.to_string(), ToastSeverity::Warning);
        }
    }

    fn drain_audiobookshelf_worker_events(&mut self) -> bool {
        let mut produced = false;
        for test in [false, true] {
            let receiver = if test {
                self.setup.audiobookshelf_test_rx.take()
            } else {
                self.setup.audiobookshelf_startup_rx.take()
            };
            let Some(receiver) = receiver else { continue };
            match receiver.rx.try_recv() {
                Ok(completion) => {
                    produced = true;
                    self.apply_audiobookshelf_completion(completion);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if test {
                        self.setup.audiobookshelf_test_rx = Some(receiver);
                    } else {
                        self.setup.audiobookshelf_startup_rx = Some(receiver);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    produced = true;
                    self.handle_audiobookshelf_worker_disconnect(receiver.generation);
                }
            }
        }
        produced
    }

    fn drain_audiobookshelf_setup_event(&mut self) -> bool {
        let Some(receiver) = self.setup.audiobookshelf_setup_rx.take() else {
            return false;
        };
        match receiver.try_recv() {
            Ok(completion) => {
                self.apply_audiobookshelf_setup_completion(completion);
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.setup.audiobookshelf_setup_rx = Some(receiver);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.handle_audiobookshelf_setup_worker_disconnect();
                true
            }
        }
    }

    fn drain_audiobookshelf_catalog_event(&mut self) -> bool {
        let Some(receiver) = self.setup.audiobookshelf_catalog_rx.take() else {
            return false;
        };
        match receiver.rx.try_recv() {
            Ok(completion) if self.audiobookshelf_runtime.accepts(completion.generation) => {
                self.apply_audiobookshelf_catalog_completion(completion);
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.setup.audiobookshelf_catalog_rx = Some(receiver);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.handle_audiobookshelf_worker_disconnect(receiver.generation);
                true
            }
            Ok(_) => false,
        }
    }

    fn apply_audiobookshelf_catalog_completion(
        &mut self,
        completion: crate::app::dispatch::session::service_startup::AudiobookshelfCatalogCompletion,
    ) {
        match completion.result {
            Ok((libraries, progress, book_progress)) => self.apply_audiobookshelf_catalog(
                completion.generation,
                libraries,
                &progress,
                &book_progress,
            ),
            Err(error)
                if matches!(
                    error.class,
                    mbv_audiobookshelf::AudiobookshelfFailureClass::AuthenticationRejected
                ) =>
            {
                self.fail_audiobookshelf_service(
                    completion.generation,
                    mbv_core::service_runtime::ServiceState::NeedsAuthentication,
                );
                let _ = self.clear_audiobookshelf_authentication();
            }
            Err(_) => {
                self.expire_launch_service(mbv_queue::ServiceKind::Audiobookshelf);
            }
        }
    }

    fn apply_audiobookshelf_catalog(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        libraries: Vec<mbv_audiobookshelf::AudiobookshelfLibrary>,
        progress: &std::collections::HashMap<
            (String, String),
            mbv_audiobookshelf::AudiobookshelfProgress,
        >,
        book_progress: &std::collections::HashMap<
            String,
            mbv_audiobookshelf::AudiobookshelfBookProgress,
        >,
    ) {
        self.audiobookshelf_libraries = libraries;
        self.audiobookshelf_browse = self
            .audiobookshelf_libraries
            .iter()
            .cloned()
            .map(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState::new)
            .collect();
        self.audiobookshelf_book_browse = self
            .audiobookshelf_libraries
            .iter()
            .cloned()
            .map(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState::new)
            .collect();
        for index in 0..self.audiobookshelf_browse.len() {
            self.activate_audiobookshelf_position(index);
            self.activate_audiobookshelf_book_position(index);
        }
        self.apply_audiobookshelf_catalog_progress(progress, book_progress);
        self.start_audiobookshelf_catalog_fetches(generation);
        self.resolve_launch_service_tab(mbv_queue::ServiceKind::Audiobookshelf);
    }

    fn apply_audiobookshelf_catalog_progress(
        &mut self,
        progress: &std::collections::HashMap<
            (String, String),
            mbv_audiobookshelf::AudiobookshelfProgress,
        >,
        book_progress: &std::collections::HashMap<
            String,
            mbv_audiobookshelf::AudiobookshelfBookProgress,
        >,
    ) {
        for index in 0..self.audiobookshelf_browse.len() {
            let book_kind =
                mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::from_media_type(
                    &self.audiobookshelf_libraries[index].media_type,
                );
            // Podcast libraries reconcile the episode progress map; book libraries the book progress map.
            match book_kind {
                mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast => {
                    self.audiobookshelf_browse[index]
                        .progress
                        .clone_from(progress);
                }
                mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book => {
                    self.audiobookshelf_book_browse[index]
                        .progress
                        .clone_from(book_progress);
                }
            }
        }
    }

    fn start_audiobookshelf_catalog_fetches(
        &self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        for (index, library) in self.audiobookshelf_libraries.iter().enumerate() {
            let book_kind =
                mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::from_media_type(
                    &library.media_type,
                );
            match book_kind {
                mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast => {
                    crate::app::dispatch::session::service_startup::start_audiobookshelf_shows(
                        self.config.lock().unwrap().clone(),
                        generation,
                        self.audiobookshelf_browse[index].catalog_request,
                        library.id.clone(),
                        0,
                        self.channels.lib_tx.clone(),
                    );
                    crate::app::dispatch::session::service_startup::start_audiobookshelf_shelves(
                        self.config.lock().unwrap().clone(),
                        generation,
                        library.id.clone(),
                        self.channels.lib_tx.clone(),
                    );
                }
                mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book => {
                    crate::app::dispatch::session::service_startup::start_audiobookshelf_books(
                        self.config.lock().unwrap().clone(),
                        generation,
                        self.audiobookshelf_book_browse[index].catalog_request,
                        library.id.clone(),
                        0,
                        self.channels.lib_tx.clone(),
                    );
                }
            }
        }
    }

    /// Drain and act on retained notification-originated actions (clear-queue
    /// confirmation and notification-failure flag).
    /// Extracted from `run()`'s loop body; returns whether any action was
    /// received so the caller can fold that into its own `had_events` for render scheduling.
    pub(in crate::app) fn drain_notif_actions(&mut self) -> bool {
        let mut produced = false;
        while let Ok(action) = self.channels.notif_action_rx.try_recv() {
            produced = true;
            match action.as_str() {
                "clear:yes" => {
                    self.dismiss_confirm();
                    self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
                }
                "__notif_failed__" => {
                    self.notif_failed = true;
                }
                _ => {} // dismissed, "ignore", "cancel", or empty
            }
        }
        produced
    }

    /// Drain the sessions-poll channel, dispatching each event to
    /// `handle_session_event`. Extracted from `run()`'s loop body; returns
    /// whether any event was received so the caller can fold that into
    /// `had_events`.
    pub(in crate::app) fn drain_session_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok(ev) = self.channels.sessions_rx.try_recv() {
            produced = true;
            self.handle_session_event(ev);
        }
        produced
    }
}
