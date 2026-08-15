fn spawn_ctrl_client<S>(
    stream: S,
    transport: CtrlTransport,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientRegistry,
    control_credential: Option<String>,
    player_status: Arc<Mutex<crate::player::PlayerStatus>>,
    shared_queue: SharedQueueState,
) where
    S: CtrlStream,
{
    let Ok(writer_stream) = stream.try_clone_stream() else {
        return;
    };
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    let mut daemon_hello = CtrlHello::current();
    if control_credential.is_none() {
        daemon_hello
            .capabilities
            .retain(|cap| cap != crate::ctrl::CTRL_CAP_CONTROL_AUTH);
    }
    if let Ok(hello_json) = serde_json::to_string(&CtrlEvent::Hello(daemon_hello)) {
        ev_tx.send(CtrlOutbound::Event(hello_json)).ok();
    }

    std::thread::spawn(move || {
        let mut w = writer_stream;
        for outbound in ev_rx {
            match outbound {
                CtrlOutbound::Event(line) => {
                    if writeln!(w, "{line}").is_err() {
                        break;
                    }
                }
                CtrlOutbound::Flush(ack) => {
                    let _ = w.flush();
                    let _ = ack.send(());
                }
            }
        }
        w.shutdown_stream();
    });
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        let Some(Ok(line)) = lines.next() else {
            return;
        };
        let (
            supports_abs_queue,
            supports_abs_progress,
            supports_abs_book_queue,
            supports_abs_book_progress,
        ) = match serde_json::from_str::<CtrlCmd>(&line) {
            Ok(CtrlCmd::Hello(info)) => {
                if let Err(e) = info.validate_peer() {
                    log::warn!(target: "daemon", "rejecting ctrl client: {e}");
                    return;
                }
                if let Some(control_credential) = control_credential.as_deref() {
                    if info.control_token.is_none() {
                        log::warn!(target: "daemon", "rejecting ctrl client: missing Control credential");
                        return;
                    }
                    if let Err(e) = info.validate_control_credential(control_credential) {
                        log::warn!(target: "daemon", "rejecting ctrl client: {e}");
                        return;
                    }
                }
                (
                    info.supports_abs_queue(),
                    info.supports_abs_progress(),
                    info.supports_abs_book_queue(),
                    info.supports_abs_book_progress(),
                )
            }
            Ok(_) => {
                log::warn!(target: "daemon", "rejecting ctrl client: missing protocol hello");
                return;
            }
            Err(e) => {
                log::warn!(target: "daemon", "rejecting ctrl client: invalid protocol hello: {e}");
                return;
            }
        };

        let status = player_status.lock().unwrap().clone();
        let q = shared_queue.queue.lock().unwrap();
        let source = shared_queue.source.lock().unwrap().clone();
        let init_event = unified_queue_state_for_peer(
            &status,
            &q,
            &source,
            supports_abs_queue,
            supports_abs_book_queue,
        );
        if let Ok(init_json) = serde_json::to_string(&init_event) {
            ev_tx.send(CtrlOutbound::Event(init_json)).ok();
        }
        let reply_tx = ev_tx.clone();
        let client_id = ctrl_clients.lock().unwrap().connect(
            ev_tx,
            transport,
            supports_abs_queue,
            supports_abs_progress,
            supports_abs_book_queue,
            supports_abs_book_progress,
        );

        for line in lines {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            if let Ok(cmd) = serde_json::from_str::<CtrlCmd>(&line) {
                let _ = merged_tx.send(DaemonEvent::Ctrl(cmd, client_id, reply_tx.clone()));
            }
        }
        let _ = merged_tx.send(DaemonEvent::CtrlDisconnected(client_id));
    });
}
