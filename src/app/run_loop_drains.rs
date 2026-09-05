use super::types_playback::PendingQueueAction;

use super::App;

impl App {
    pub(super) fn drain_audiobookshelf_events(&mut self) -> bool {
        let mut produced = false;
        for test in [false, true] {
            let receiver = if test {
                self.audiobookshelf_test_rx.take()
            } else {
                self.audiobookshelf_startup_rx.take()
            };
            let Some(receiver) = receiver else { continue };
            match receiver.rx.try_recv() {
                Ok(completion) => {
                    produced = true;
                    self.apply_audiobookshelf_completion(completion);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if test {
                        self.audiobookshelf_test_rx = Some(receiver);
                    } else {
                        self.audiobookshelf_startup_rx = Some(receiver);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    produced = true;
                    self.handle_audiobookshelf_worker_disconnect(receiver.generation);
                }
            }
        }
        if let Some(receiver) = self.audiobookshelf_setup_rx.take() {
            match receiver.try_recv() {
                Ok(completion) => {
                    produced = true;
                    self.apply_audiobookshelf_setup_completion(completion);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.audiobookshelf_setup_rx = Some(receiver);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    produced = true;
                    self.handle_audiobookshelf_setup_worker_disconnect();
                }
            }
        }
        if let Some(receiver) = self.audiobookshelf_catalog_rx.take() {
            match receiver.rx.try_recv() {
                Ok(completion) if self.audiobookshelf_runtime.accepts(completion.generation) => {
                    produced = true;
                    match completion.result {
                         Ok((libraries, progress, book_progress)) => {
                             self.audiobookshelf_libraries = libraries;
                             self.audiobookshelf_browse = self
                                 .audiobookshelf_libraries
                                 .iter()
                                 .cloned()
                                 .map(super::types_audiobookshelf_browse::AudiobookshelfBrowseState::new)
                                 .collect();
                             self.audiobookshelf_book_browse = self
                                 .audiobookshelf_libraries
                                 .iter()
                                 .cloned()
                                 .map(super::types_audiobookshelf_browse::AudiobookshelfBookBrowseState::new)
                                 .collect();
                             for index in 0..self.audiobookshelf_browse.len() {
                                 self.activate_audiobookshelf_position(index);
                                 self.activate_audiobookshelf_book_position(index);
                             }
                             for index in 0..self.audiobookshelf_browse.len() {
                                 let book_kind = super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::from_media_type(
                                     &self.audiobookshelf_libraries[index].media_type,
                                 );
                                 // Podcast libraries reconcile the episode progress
                                 // map; book libraries the book progress map.
                                 match book_kind {
                                     super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast => {
                                         self.audiobookshelf_browse[index].progress = progress.clone();
                                     }
                                     super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book => {
                                         self.audiobookshelf_book_browse[index].progress = book_progress.clone();
                                     }
                                 }
                             }
                             for library in &self.audiobookshelf_libraries {
                                 let book_kind = super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::from_media_type(&library.media_type);
                                 match book_kind {
                                     super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast => {
                                         super::service_startup::start_audiobookshelf_shows(
                                             self.config.lock().unwrap().clone(),
                                             completion.generation,
                                             library.id.clone(),
                                             0,
                                             self.lib_tx.clone(),
                                         );
                                         super::service_startup::start_audiobookshelf_shelves(
                                             self.config.lock().unwrap().clone(),
                                             completion.generation,
                                             library.id.clone(),
                                             self.lib_tx.clone(),
                                         );
                                     }
                                     super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book => {
                                         super::service_startup::start_audiobookshelf_books(
                                             self.config.lock().unwrap().clone(),
                                             completion.generation,
                                             library.id.clone(),
                                             0,
                                             self.lib_tx.clone(),
                                         );
                                     }
                                 }
                             }
                         },
                        Err(error) if matches!(error.class, mbv_core::audiobookshelf::AudiobookshelfFailureClass::AuthenticationRejected) => {
                            self.audiobookshelf_runtime.complete(
                                completion.generation,
                                mbv_core::service_runtime::ServiceState::NeedsAuthentication,
                            );
                            let _ = self.clear_audiobookshelf_authentication();
                        },
                        Err(_) => {}
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.audiobookshelf_catalog_rx = Some(receiver)
                }
                _ => {}
            }
        }
        produced
    }

    /// Drain and act on retained notification-originated actions (clear-queue
    /// confirmation and notification-failure flag).
    /// Extracted from `run()`'s loop body; returns whether any action was
    /// received so the caller can fold that into its own `had_events` for render scheduling.
    pub(super) fn drain_notif_actions(&mut self) -> bool {
        let mut produced = false;
        while let Ok(action) = self.notif_action_rx.try_recv() {
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
    pub(super) fn drain_session_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok(ev) = self.sessions_rx.try_recv() {
            produced = true;
            self.handle_session_event(ev);
        }
        produced
    }
}
