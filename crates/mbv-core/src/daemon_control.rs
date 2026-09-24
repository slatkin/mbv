include!("daemon_control_queue.rs");

/// Fetches `item_ids` from Emby off the event-loop thread and sends the
/// result through `tx` as a `DaemonEvent`, built by `to_event`. Shared by
/// every ctrl handler that resolves item ids against Emby before rejoining
/// the daemon's single-threaded event loop.
fn spawn_item_lookup<F>(
    client: &Arc<Mutex<EmbyClient>>,
    tx: &mpsc::Sender<DaemonEvent>,
    item_ids: Vec<String>,
    to_event: F,
) where
    F: FnOnce(Result<Vec<EmbyItem>, String>) -> Option<DaemonEvent> + Send + 'static,
{
    let tx = tx.clone();
    let lookup_client = client.lock().unwrap().clone();
    std::thread::spawn(move || {
        if let Some(event) = to_event(lookup_client.get_items_by_ids(&item_ids)) {
            let _ = tx.send(event);
        }
    });
}

/// Plays a resolved-by-id playback intent. Replaces the legacy `PlayItems`
/// wire command, which carried both the wire shape and this internal
/// control-flow re-entry; the wire variant is gone (ADR 0020), so the
/// resolved-play path now lives here as a plain function.
#[allow(clippy::too_many_arguments)]
fn play_resolved_items(
    fetched: Vec<EmbyItem>,
    start_idx: usize,
    start_ticks: i64,
    new_source: crate::config::QueueSource,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    queue: &mut PlaybackQueue,
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
    transitions: &crate::playback_transition::OwnerTransitionState,
) {
    let queue_items: Vec<QueueItem> = fetched
        .iter()
        .cloned()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect();
    let start_idx = start_idx.min(queue_items.len().saturating_sub(1));
    *queue = PlaybackQueue::from_queue_items(queue_items, Some(start_idx));
    *source = new_source;
    mint_queue_lineage(shared_queue);
    broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
    if fetched.len() == 1 {
        let mut play_item = fetched[0].clone();
        if start_ticks > 0 {
            play_item.playback_position_ticks = start_ticks;
        }
        let c = Arc::new(client.lock().unwrap().clone());
        player.play(&play_item, c, 100);
    } else {
        let mut play_items = fetched;
        if start_ticks > 0 {
            play_items[start_idx].playback_position_ticks = start_ticks;
        }
        let c = Arc::new(client.lock().unwrap().clone());
        player.play_queue(play_items, start_idx, c, 100);
    }
}

/// Mints the next queue lineage into `SharedQueueState.lineage`, the single
/// source of truth for queue lineage (needed by other threads that seed
/// newly-connecting ctrl clients off `SharedQueueState`), and returns it.
fn mint_queue_lineage(shared_queue: &SharedQueueState) -> crate::ctrl::QueueLineage {
    let mut lineage = shared_queue.lineage.lock().unwrap();
    lineage.0 = lineage.0.checked_add(1).expect("owner queue lineage exhausted");
    *lineage
}

fn install_idle_queue_load(
    request_id: crate::ctrl::QueueLoadRequestId,
    slots: Vec<(QueueSlotId, QueueItem)>,
    cursor: usize,
    source: crate::config::QueueSource,
    reply_tx: &CtrlSender,
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) {
    let active_slot = slots.get(cursor).map(|(slot_id, _)| *slot_id);
    player.advance_sequence_generation();
    player.set_initial_queue(
        &slots.iter().map(|(_, item)| item.clone()).collect::<Vec<_>>(),
        cursor,
    );
    reset_slot_jumps(
        &mut owner.core.transitions,
        &mut owner.queued_transition_origin,
    );
    owner.core.queue = PlaybackQueue::from_slot_items(
        slots,
        active_slot,
        crate::playback_queue::QueueRevision::default(),
    );
    owner.core.source = source;
    mint_queue_lineage(shared_queue);
    owner.core.note_observed_active_slot(None);
    *shared_queue.observed_active_slot.lock().unwrap() = None;
    broadcast_queue_state(
        ctrl_clients,
        player,
        shared_queue,
        &owner.core.queue,
        &owner.core.source,
        &owner.core.transitions,
    );
    if let Err(error) = persist_stay_alive_owner_queue(owner, player, shared_queue) {
        log::error!(target: "queue", "failed to persist accepted Stay-alive queue load: {error}");
    }
    send_to(
        reply_tx,
        &CtrlEvent::UnifiedQueueLoadResult {
            request_id,
            result: crate::ctrl::QueueLoadResult::Accepted,
        },
    );
}

const IDLE_QUEUE_LOAD_STOP_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn cancel_pending_idle_queue_load(owner: &mut DaemonPlayerOwner, reason: &str) -> bool {
    let Some(pending) = owner.pending_idle_load.take() else {
        return false;
    };
    send_to(
        &pending.reply_tx,
        &CtrlEvent::UnifiedQueueLoadResult {
            request_id: pending.request_id,
            result: crate::ctrl::QueueLoadResult::Rejected {
                reason: reason.to_string(),
            },
        },
    );
    true
}

pub(super) fn cancel_pending_idle_queue_load_if_run_changed(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
) -> bool {
    let current_run = (0, player.status.lock().unwrap().sequence_generation);
    if owner
        .pending_idle_load
        .as_ref()
        .is_some_and(|pending| pending.stopped_run != current_run)
    {
        return cancel_pending_idle_queue_load(owner, "playback run changed during queue load");
    }
    false
}

pub(super) fn expire_pending_idle_queue_load(
    owner: &mut DaemonPlayerOwner,
    now: Instant,
) -> bool {
    if owner.pending_idle_load.as_ref().is_some_and(|pending| {
        now.duration_since(pending.started_at) >= IDLE_QUEUE_LOAD_STOP_TIMEOUT
    }) {
        return cancel_pending_idle_queue_load(
            owner,
            "timed out waiting for playback stop finalization",
        );
    }
    false
}

pub(super) fn complete_pending_idle_queue_load(
    run_identity: (crate::ctrl::PlaybackRequestId, crate::ctrl::PlaybackGeneration),
    failure: Option<String>,
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) -> bool {
    if !owner
        .pending_idle_load
        .as_ref()
        .is_some_and(|pending| pending.stopped_run == run_identity)
    {
        return false;
    }
    let pending = owner.pending_idle_load.take().expect("checked above");
    if let Some(reason) = failure {
        send_to(
            &pending.reply_tx,
            &CtrlEvent::UnifiedQueueLoadResult {
                request_id: pending.request_id,
                result: crate::ctrl::QueueLoadResult::Rejected { reason },
            },
        );
        return true;
    }
    install_idle_queue_load(
        pending.request_id,
        pending.slots,
        pending.cursor,
        pending.source,
        &pending.reply_tx,
        owner,
        player,
        shared_queue,
        ctrl_clients,
    );
    true
}

/// Sends the rejection reply for a command whose owner-role gate
/// (`CtrlCmd::requires_owner`) failed. Reply shape/event and reason text are
/// kept per-command, matching what each arm sent before the gate moved here.
/// Matches `OwnerGateRejection` exhaustively: its 3 variants are exactly the
/// gated commands, so there is no wildcard/unreachable arm to fall into.
#[allow(clippy::too_many_arguments)]
fn send_role_gate_rejection(
    rejection: crate::ctrl::OwnerGateRejection,
    reply_tx: &CtrlSender,
    ctrl_clients: &ClientRegistry,
    client_id: CtrlClientId,
    player: &Player,
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
    queue_lineage: crate::ctrl::QueueLineage,
) {
    match rejection {
        crate::ctrl::OwnerGateRejection::AdoptQueue => reject_command(
            reply_tx,
            ctrl_clients,
            client_id,
            player,
            queue,
            source,
            queue_lineage,
            "Stay-alive owner queues cannot be adopted by Clients".to_string(),
        ),
        crate::ctrl::OwnerGateRejection::QueueLoadIdle { request_id } => send_to(
            reply_tx,
            &CtrlEvent::UnifiedQueueLoadResult {
                request_id,
                result: crate::ctrl::QueueLoadResult::Rejected {
                    reason: "idle queue loads are supported only by the Stay-alive owner"
                        .to_string(),
                },
            },
        ),
        crate::ctrl::OwnerGateRejection::QueueSourceUpdate => send_to(
            reply_tx,
            &CtrlEvent::CommandRejected(
                "queue source updates are supported only by the Stay-alive owner".to_string(),
            ),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_ctrl_for_role(
    cmd: CtrlCmd,
    client_id: CtrlClientId,
    request: CtrlRequest<'_>,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    audio_only: bool,
    owner: &mut DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
    has_audiobookshelf: bool,
    merged_tx: &mpsc::Sender<DaemonEvent>,
    stay_alive: bool,
    role: crate::daemon::DaemonRole,
) {
    cancel_pending_idle_queue_load_if_run_changed(owner, player);
    if owner.pending_idle_load.is_some() && !matches!(&cmd, CtrlCmd::RequestShutdown) {
        if let CtrlCmd::UnifiedQueueLoadIdle { request_id, .. } = cmd {
            send_to(
                request.reply_tx,
                &CtrlEvent::UnifiedQueueLoadResult {
                    request_id,
                    result: crate::ctrl::QueueLoadResult::Rejected {
                        reason: "another idle queue load is pending".to_string(),
                    },
                },
            );
        } else {
            send_to(
                request.reply_tx,
                &CtrlEvent::CommandRejected("owner is finalizing an idle queue load".to_string()),
            );
        }
        return;
    }
    let DaemonPlayerOwner {
        core: PlayerOwnerState { queue, source, transitions, .. },
        intents: playback_intents,
        queued_transition_origin,
        ..
    } = &mut *owner;
    // `SharedQueueState.lineage` is the single source of truth (other
    // threads read it for cold ctrl-client snapshots); this is a
    // function-local snapshot so hot-path comparisons below don't
    // re-lock per read; `mint_queue_lineage` below writes the new value
    // straight to `shared_queue.lineage` for the next command's read.
    let queue_lineage = *shared_queue.lineage.lock().unwrap();
    match cmd.requires_owner() {
        crate::ctrl::OwnerGate::OwnerOnly(rejection) if role != crate::daemon::DaemonRole::Local => {
            send_role_gate_rejection(
                rejection,
                request.reply_tx,
                ctrl_clients,
                client_id,
                player,
                queue,
                source,
                queue_lineage,
            );
            return;
        }
        crate::ctrl::OwnerGate::NonOwnerOnly(rejection) if role == crate::daemon::DaemonRole::Local => {
            send_role_gate_rejection(
                rejection,
                request.reply_tx,
                ctrl_clients,
                client_id,
                player,
                queue,
                source,
                queue_lineage,
            );
            return;
        }
        _ => {}
    }
    let has_emby = !client.lock().unwrap().token.is_empty();
    if matches!(cmd, CtrlCmd::RequestShutdown) {
        log::info!(target: "daemon", "RequestShutdown received from ctrl client {client_id}");
        if stay_alive {
            log::info!(target: "daemon", "RequestShutdown rejected: daemon is in stay-alive mode");
            send_to(
                request.reply_tx,
                &CtrlEvent::ShutdownRejected {
                    reason: "daemon is in stay-alive mode".to_string(),
                },
            );
            return;
        }
        let is_local = ctrl_clients.lock().unwrap().is_local_client(client_id);
        if !is_local {
            send_to(
                request.reply_tx,
                &CtrlEvent::ShutdownRejected {
                    reason: "lifecycle requests require local transport".to_string(),
                },
            );
            return;
        }

        let player_status = player.status.lock().unwrap().clone();
        let mut queue_state = project_queue_state(queue, source, &player_status);

        if role != crate::daemon::DaemonRole::Local && queue_state.items.is_empty() {
            if let Some(existing) = crate::config::load_queue_state() {
                if !existing.items.is_empty() {
                    queue_state = existing;
                }
            }
        }

        if let Err(e) = crate::config::save_queue_state(&queue_state) {
            log::error!(
                target: "daemon",
                "coordinated shutdown rejected: queue persistence failed: {e}"
            );
            send_to(
                request.reply_tx,
                &CtrlEvent::ShutdownRejected {
                    reason: format!("queue persistence failed: {e}"),
                },
            );
            return;
        }

        let mut clients = ctrl_clients.lock().unwrap();
        if clients.authority == AuthorityHolder::EmbyRemote {
            clients.authority = AuthorityHolder::Ctrl;
        }
    }

    match cmd {
        CtrlCmd::Hello(_) => {
            log::warn!(target: "daemon", "unexpected ctrl protocol hello after negotiation");
        }
        CtrlCmd::UnifiedAdoptQueue {
            items,
            cursor,
            source: new_source,
        } => {
            // Adoption only applies to a Cold daemon — one with no queue yet.
            if !queue.is_empty() {
                log::warn!(
                    target: "daemon",
                    "ignoring UnifiedAdoptQueue: daemon already has a queue ({} slot(s))",
                    queue.len()
                );
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    "daemon already has a queue; adoption skipped".to_string(),
                );
                return;
            }
            let supports_abs_queue = ctrl_clients.lock().unwrap().supports_abs_queue(client_id);
            let supports_abs_book_queue = ctrl_clients
                .lock()
                .unwrap()
                .supports_abs_book_queue(client_id);
            if let Some(reason) =
                abs_queue_transport_rejection(&items, supports_abs_queue, supports_abs_book_queue)
            {
                reject_command(request.reply_tx, ctrl_clients, client_id, player, queue, source, queue_lineage, reason);
                return;
            }
            let (items, next_cursor) = admit_queue_items(
                items,
                Some(cursor),
                audio_only,
                has_emby,
                has_audiobookshelf,
            );
            player.set_initial_queue(&items, next_cursor);
            reset_slot_jumps(transitions, queued_transition_origin);
            *queue = PlaybackQueue::from_queue_items(items, Some(next_cursor));
            *source = new_source;
            mint_queue_lineage(shared_queue);
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);

            let adopted_slots: Vec<(QueueSlotId, String)> = queue
                .slots()
                .iter()
                .filter_map(|slot| {
                    slot.item
                        .as_emby()
                        .map(|item| (slot.slot_id, item.id.clone()))
                })
                .collect();
            if !adopted_slots.is_empty() {
                let item_ids: Vec<String> = adopted_slots
                    .iter()
                    .map(|(_, item_id)| item_id.clone())
                    .collect();
                spawn_item_lookup(client, merged_tx, item_ids, move |result| match result {
                    Ok(items) => {
                        let items_by_id: std::collections::HashMap<String, EmbyItem> =
                            items.into_iter().map(|item| (item.id.clone(), item)).collect();
                        let enriched = adopted_slots
                            .into_iter()
                            .filter_map(|(slot_id, item_id)| {
                                items_by_id.get(&item_id).cloned().map(|item| (slot_id, item))
                            })
                            .collect();
                        Some(DaemonEvent::QueueEnriched(enriched))
                    }
                    Err(error) => {
                        log::warn!(target: "queue", "adopted queue enrichment fetch failed: {error}");
                        None
                    }
                });
            }
        }
        // Stale index-addressed jump from a cross-version peer. Slot-id +
        // request-identity JumpTo is now the only jump path, so an ordinal
        // index carries no evidence about which slot was intended: reject it
        // visibly via the existing command-rejection path (design D6).
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::JumpTo(_)) => {
            send_to(
                request.reply_tx,
                &CtrlEvent::CommandRejected(
                    "index-addressed queue jump is no longer supported; use unified queue commands"
                        .to_string(),
                ),
            );
        }
        CtrlCmd::PlayerCmd(pc) => {
            player.send_command(PlayerCommand::from(pc));
        },
        CtrlCmd::Stop => {
            player.stop();
            reset_slot_jumps(transitions, queued_transition_origin);
        }
        CtrlCmd::PlaybackIntent(intent) => {
            let pipe_output = client.lock().unwrap().config.audio_pipe_enabled;
            let accepted = playback_intents.accept(client_id, intent.clone(), pipe_output);
            let coalesced = accepted.iter().any(|event| {
                matches!(
                    event.outcome,
                    crate::ctrl::PlaybackIntentOutcome::Coalesced { .. }
                )
            });
            for event in accepted {
                send_to(request.reply_tx, &CtrlEvent::PlaybackIntent(event));
            }
            if let Some(status) = playback_intents.pipe_status() {
                log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, playback_intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                send_to(request.reply_tx, &CtrlEvent::PipePlaybackStatus(status));
            }
            if coalesced {
                return;
            }
            match intent.action {
                crate::ctrl::PlaybackIntentAction::Play {
                    item_ids,
                    start_idx,
                    start_ticks,
                    source: intent_source,
                } => {
                    if !has_emby {
                        return;
                    }
                    playback_intents.mark_resolving(intent.request_id);
                    if let Some(status) = playback_intents.pipe_status() {
                        log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, playback_intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                        send_to(request.reply_tx, &CtrlEvent::PipePlaybackStatus(status));
                    }
                    let request_id = intent.request_id;
                    let generation = intent.generation;
                    spawn_item_lookup(client, merged_tx, item_ids, move |fetched| {
                        Some(DaemonEvent::PlaybackResolved {
                            start_idx,
                            start_ticks,
                            source: intent_source,
                            client_id,
                            request_id,
                            generation,
                            fetched,
                        })
                    });
                }
                crate::ctrl::PlaybackIntentAction::Stop => {
                    player.stop();
                    reset_slot_jumps(transitions, queued_transition_origin);
                }
                crate::ctrl::PlaybackIntentAction::SetPaused { paused } => {
                    if player.status.lock().unwrap().paused != paused {
                        player.send_command(PlayerCommand::TogglePause);
                    }
                }
                action @ (crate::ctrl::PlaybackIntentAction::Next
                | crate::ctrl::PlaybackIntentAction::Previous) => {
                    // A relative step advances from the *desired* active slot
                    // — the newest queued or in-flight transition, else the
                    // slot the run observes playing, else the queue's active
                    // marker. Stepping from the published `current_idx`
                    // mirror instead recomputes the neighbor from a
                    // coordinate that lags one transition behind while a jump
                    // settles, so rapid Next presses kept landing on (or
                    // re-issuing) the wrong slot.
                    let base_idx = transitions
                        .queued_latest()
                        .or_else(|| transitions.in_flight())
                        .map(|t| t.target)
                        .or_else(|| {
                            let observed = *shared_queue.observed_active_slot.lock().unwrap();
                            observed
                        })
                        .or_else(|| queue.active_slot_id())
                        .and_then(|slot| queue.slot_index(slot));
                    let neighbor_idx = base_idx.and_then(|idx| match action {
                        crate::ctrl::PlaybackIntentAction::Previous => idx.checked_sub(1),
                        _ => Some(idx + 1).filter(|&next| next < queue.len()),
                    });
                    log::info!(
                        target: "transition",
                        "playback intent: action={:?} queued_latest={:?} in_flight={:?} observed_active_slot={:?} queue_active_slot={:?} base_idx={:?} neighbor_idx={:?} queue_len={}",
                        action,
                        transitions.queued_latest().map(|t| t.target),
                        transitions.in_flight().map(|t| t.target),
                        *shared_queue.observed_active_slot.lock().unwrap(),
                        queue.active_slot_id(),
                        base_idx,
                        neighbor_idx,
                        queue.len(),
                    );
                    if let Some(slot_id) =
                        neighbor_idx.and_then(|idx| queue.slots().get(idx).map(|s| s.slot_id))
                    {
                        dispatch_slot_jump(
                            transitions,
                            queued_transition_origin,
                            ctrl_clients,
                            player,
                            shared_queue,
                            queue,
                            source,
                            client_id,
                            crate::playback_transition::Transition::new(
                                intent.request_id,
                                intent.generation,
                                slot_id,
                            ),
                        );
                    }
                }
            }
        }
        CtrlCmd::RequestShutdown => {
            send_to(request.reply_tx, &CtrlEvent::ShutdownAccepted);
            let _ = merged_tx.send(DaemonEvent::Shutdown);
        }
        CtrlCmd::ApplyServiceSetup { .. } => {}
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id,
            slots,
            cursor,
            source: new_source,
        } => {
            let (supports_operation, supports_abs_queue, supports_abs_book_queue) = {
                let clients = ctrl_clients.lock().unwrap();
                (
                    clients.supports_owner_queue_load(client_id),
                    clients.supports_abs_queue(client_id),
                    clients.supports_abs_book_queue(client_id),
                )
            };
            let reason = if !supports_operation {
                Some("peer did not negotiate owner queue-load capability".to_string())
            } else {
                abs_queue_transport_rejection(
                    slots.iter().map(|slot| &slot.item),
                    supports_abs_queue,
                    supports_abs_book_queue,
                )
            };
            if let Some(reason) = reason {
                send_to(
                    request.reply_tx,
                    &CtrlEvent::UnifiedQueueLoadResult {
                        request_id,
                        result: crate::ctrl::QueueLoadResult::Rejected { reason },
                    },
                );
                return;
            }
            let submitted: Vec<_> = slots
                .into_iter()
                .map(|slot| (QueueSlotId::from_raw(slot.slot_id), slot.item))
                .collect();
            let was_nonempty = !submitted.is_empty();
            let (admitted, next_cursor) = admit_queue_slots(
                submitted,
                Some(cursor),
                audio_only,
                has_emby,
                has_audiobookshelf,
            );
            let admission_error = if was_nonempty && admitted.is_empty() {
                Some("Playback owner rejected the queue load".to_string())
            } else {
                audio_only_rejection(audio_only, admitted.iter().map(|(_, item)| item))
            };
            if let Some(reason) = admission_error {
                send_to(
                    request.reply_tx,
                    &CtrlEvent::UnifiedQueueLoadResult {
                        request_id,
                        result: crate::ctrl::QueueLoadResult::Rejected { reason },
                    },
                );
                return;
            }

            let stopped_run = (0, player.status.lock().unwrap().sequence_generation);
            if player.status.lock().unwrap().active {
                owner.pending_idle_load = Some(PendingIdleQueueLoad {
                    request_id,
                    slots: admitted,
                    cursor: next_cursor,
                    source: new_source,
                    reply_tx: request.reply_tx.clone(),
                    stopped_run,
                    started_at: Instant::now(),
                });
                player.stop();
                return;
            }

            install_idle_queue_load(
                request_id,
                admitted,
                next_cursor,
                new_source,
                request.reply_tx,
                owner,
                player,
                shared_queue,
                ctrl_clients,
            );
        }
        CtrlCmd::UnifiedQueueSourceUpdate { source: new_source, lineage } => {
            let supports_operation = ctrl_clients
                .lock()
                .unwrap()
                .supports_owner_queue_load(client_id);
            if !supports_operation {
                send_to(
                    request.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "peer did not negotiate owner queue-load capability".to_string(),
                    ),
                );
            } else if lineage != queue_lineage {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    "queue source update rejected: owner queue lineage changed".to_string(),
                );
            } else {
                *source = new_source;
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
            }
        }
        // ── Unified queue commands ──────────────────────────────────────
        CtrlCmd::UnifiedQueueReplace { items, slots, start_idx, source: new_source } => {
            let submitted_slots: Vec<(crate::playback_queue::QueueSlotId, QueueItem)> = if slots.is_empty() {
                items
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| (crate::playback_queue::QueueSlotId::from_raw((index + 1) as u64), item))
                    .collect()
            } else {
                slots
                    .into_iter()
                    .map(|slot| (crate::playback_queue::QueueSlotId::from_raw(slot.slot_id), slot.item))
                    .collect()
            };
            let submitted_items: Vec<QueueItem> = submitted_slots.iter().map(|(_, item)| item.clone()).collect();
            let supports_abs_queue = ctrl_clients.lock().unwrap().supports_abs_queue(client_id);
            let supports_abs_book_queue = ctrl_clients
                .lock()
                .unwrap()
                .supports_abs_book_queue(client_id);
            if let Some(reason) = abs_queue_transport_rejection(
                &submitted_items,
                supports_abs_queue,
                supports_abs_book_queue,
            ) {
                reject_command(request.reply_tx, ctrl_clients, client_id, player, queue, source, queue_lineage, reason);
                return;
            }
            let (slots, next_cursor) = admit_queue_slots(
                submitted_slots,
                start_idx,
                audio_only,
                has_emby,
                has_audiobookshelf,
            );
            if slots.is_empty() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    "Playback owner rejected the queue replacement".to_string(),
                );
                return;
            }
            // Audio-only admission: reject if the daemon is in audio-only
            // mode and any item is non-audio.
            let admitted_items: Vec<QueueItem> =
                slots.iter().map(|(_, item)| item.clone()).collect();
            if let Some(reason) = audio_only_rejection(audio_only, &admitted_items) {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    reason,
                );
                return;
            }
            let active_slot = slots.get(next_cursor).map(|(slot_id, _)| *slot_id);
            *queue = PlaybackQueue::from_slot_items(
                slots,
                active_slot,
                crate::playback_queue::QueueRevision::default(),
            );
            *source = new_source;
            mint_queue_lineage(shared_queue);
            reset_slot_jumps(transitions, queued_transition_origin);
            // A new queue invalidates the previous playback observation (design
            // D2): clear it on both the shared snapshot and the owner core so
            // Next/Previous fall back to the new queue's active slot until the
            // first TrackChanged observation arrives. Momentary `None` is
            // correct — the observed slot follows playback, not the replace.
            // The shared clear happens with the reset so the publish is
            // already coherent; the owner-core clear happens before that
            // publish too, so both copies of the observation are gone by the
            // time a client can read the new snapshot.
            *shared_queue.observed_active_slot.lock().unwrap() = None;
            // `send_command` alone only reaches an already-running mpv
            // thread; on a freshly started daemon no thread exists yet, so
            // route through `submit_queue_slots`, which cold-starts one when
            // needed (as `play_resolved_items` does).
            let queue_slots = queue.slot_pairs();
            let all_audio = queue_slots.iter().all(|slot| slot.item.is_audio());
            let c = Arc::new(client.lock().unwrap().clone());
            let headless = player.headless_for(&c, all_audio);
            player.submit_queue_slots(queue_slots, next_cursor, Some(c), headless, 100);
            owner.core.note_observed_active_slot(None);
            // Publish after the submit, not before: the snapshot resolves its
            // active slot from the queue marker only while the player reports
            // active, and a cold-start submit is what flips that flag and seeds
            // the start item. Broadcasting first handed clients a new queue
            // with no active slot, so their now-playing projection fell back to
            // a stale `current_idx` and showed the queue's first row until the
            // next broadcast — the wrong-track flash this change removes.
            // Reborrowing `owner.core` here (rather than the arm's earlier
            // destructured fields) is what lets the publish follow the submit.
            broadcast_queue_state(
                ctrl_clients,
                player,
                shared_queue,
                &owner.core.queue,
                &owner.core.source,
                &owner.core.transitions,
            );
        }
        CtrlCmd::UnifiedQueueAppend { items } => {
            if items.is_empty() {
                return;
            }
            let supports_abs_queue = ctrl_clients.lock().unwrap().supports_abs_queue(client_id);
            let supports_abs_book_queue = ctrl_clients
                .lock()
                .unwrap()
                .supports_abs_book_queue(client_id);
            if let Some(reason) =
                abs_queue_transport_rejection(&items, supports_abs_queue, supports_abs_book_queue)
            {
                reject_command(request.reply_tx, ctrl_clients, client_id, player, queue, source, queue_lineage, reason);
                return;
            }
            let mut items = items;
            items.retain(|item| daemon_admits(item, audio_only, has_emby, has_audiobookshelf));
            if items.is_empty() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    "Playback owner rejected the queue append".to_string(),
                );
                return;
            }
            // Audio-only admission: reject if any appended item is non-audio.
            if let Some(reason) = audio_only_rejection(audio_only, &items) {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    reason,
                );
                return;
            }
            // Allocate the owner slot ids once, in the daemon's canonical
            // queue, and hand the same ids to the Playback run.
            let items_for_player: Vec<ExecSlot> = items
                .into_iter()
                .map(|item| {
                    let slot_id = queue.append(item.clone());
                    ExecSlot {
                        slot_id,
                        item,
                    }
                })
                .collect();
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
            // Append to the player's queue rather than replacing the whole queue.
            player.send_command(PlayerCommand::QueueAppend {
                items: items_for_player,
            });
        }
        CtrlCmd::UnifiedQueueRemoveSlot { slot_id } => {
            let sid = QueueSlotId::from_raw(slot_id);
            if queue.slot(sid).is_none() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    "slot not found; remove skipped".to_string(),
                );
            } else if queue.active_slot_id() == Some(sid) {
                queue.remove_active_slot_confirmed(sid);
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
                if queue.is_empty() {
                    // Clear the player's queue and stop.
                    player.advance_sequence_generation();
                    player.send_command(PlayerCommand::SubmitQueue {
                        items: Vec::new(),
                        start_idx: 0,
                    });
                    player.stop();
                    reset_slot_jumps(transitions, queued_transition_origin);
                } else {
                    // Removing the playing slot forces a track change that
                    // carries no awaited transition identity, so anything in
                    // flight can never settle: interrupt it deliberately.
                    reset_slot_jumps(transitions, queued_transition_origin);
                    player.send_command(PlayerCommand::QueueRemove(sid));
                }
            } else {
                queue.remove_slot(sid);
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
                player.send_command(PlayerCommand::QueueRemove(sid));
            }
        }
        CtrlCmd::UnifiedQueueRemoveSlots { slot_ids } => {
            // One canonical revision and one published snapshot for the whole
            // edit: a client that selected a range must never observe the
            // queue shrinking one row per round trip.
            let mut removed = Vec::new();
            let mut removed_active = false;
            for slot_id in slot_ids {
                let sid = QueueSlotId::from_raw(slot_id);
                if queue.slot(sid).is_none() {
                    continue;
                }
                if queue.active_slot_id() == Some(sid) {
                    queue.remove_active_slot_confirmed(sid);
                    removed_active = true;
                } else {
                    queue.remove_slot(sid);
                }
                removed.push(sid);
            }
            if removed.is_empty() {
                return;
            }
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
            if removed_active {
                // Removing the playing slot forces a track change that carries
                // no awaited transition identity, so anything in flight can
                // never settle: interrupt it deliberately (same as the
                // single-slot arm).
                reset_slot_jumps(transitions, queued_transition_origin);
            }
            if queue.is_empty() {
                player.advance_sequence_generation();
                player.send_command(PlayerCommand::SubmitQueue {
                    items: Vec::new(),
                    start_idx: 0,
                });
                player.stop();
                reset_slot_jumps(transitions, queued_transition_origin);
            } else {
                // The player run keeps its own queue copy; its per-slot edits
                // are not published, so one command per removed slot is fine
                // and lets it resolve the active-slot hand-off itself.
                for sid in removed {
                    player.send_command(PlayerCommand::QueueRemove(sid));
                }
            }
        }
        CtrlCmd::UnifiedQueueMoveSlot { slot_id, to_index } => {
            let sid = QueueSlotId::from_raw(slot_id);
            if queue.slot(sid).is_none() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    queue_lineage,
                    "slot not found; move skipped".to_string(),
                );
            } else {
                queue.move_slot(sid, to_index);
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
                player.send_command(PlayerCommand::QueueMove(sid, to_index));
            }
        }
        CtrlCmd::UnifiedQueuePlaySlot { slot_id } => {
            let sid = QueueSlotId::from_raw(slot_id);
            match queue.slot(sid) {
                Some(_) => {
                    // No client request id on this command, so the owner mints
                    // one to correlate the settling observation.
                    let (request_id, generation) = transitions.mint_local_id();
                    // `dispatch_slot_jump` publishes the snapshot after
                    // accepting, so the pending slot reaches Clients.
                    dispatch_slot_jump(
                        transitions,
                        queued_transition_origin,
                        ctrl_clients,
                        player,
                        shared_queue,
                        queue,
                        source,
                        client_id,
                        crate::playback_transition::Transition::new(request_id, generation, sid),
                    );
                }
                None => {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        queue,
                        source,
                        queue_lineage,
                        "slot not found; play skipped".to_string(),
                    );
                }
            }
        }
        CtrlCmd::UnifiedQueueClear => {
            queue.clear();
            *source = crate::config::QueueSource::Unknown;
            mint_queue_lineage(shared_queue);
            player.advance_sequence_generation();
            player.send_command(PlayerCommand::SubmitQueue {
                items: Vec::new(),
                start_idx: 0,
            });
            player.stop();
            reset_slot_jumps(transitions, queued_transition_origin);
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
        }
    }
}

fn owner_admin_transport_allowed(
    role: crate::daemon::DaemonRole,
    kind: crate::config::ServiceKind,
    transport: Option<CtrlTransport>,
) -> bool {
    let role_allowed = role == crate::daemon::DaemonRole::Packaged
        || kind == crate::config::ServiceKind::Audiobookshelf;
    role_allowed && transport == Some(CtrlTransport::Local)
}
