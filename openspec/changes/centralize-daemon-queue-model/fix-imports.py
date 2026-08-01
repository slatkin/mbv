DCORE = "/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_core.rs"
DCTRL = "/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_control.rs"

with open(DCORE) as f:
    dc = f.read()

old_import = 'use crate::ctrl::{\n    CtrlCmd, CtrlEvent, CtrlHello, CtrlState, DisconnectReason, PlaybackGeneration, PlaybackIntent,\n    PlaybackIntentAction, PlaybackIntentEvent, PlaybackIntentOutcome, PlaybackRequestId,\n};'
new_import = 'use crate::ctrl::{\n    CtrlCmd, CtrlEvent, CtrlHello, CtrlState, DisconnectReason, PlaybackGeneration, PlaybackIntent,\n    PlaybackIntentAction, PlaybackIntentEvent, PlaybackIntentOutcome, PlaybackRequestId,\n    WireCommand,\n};'
assert old_import in dc, "import block not found in daemon_core.rs"
dc = dc.replace(old_import, new_import)
with open(DCORE, "w") as f:
    f.write(dc)
print("WireCommand import added to daemon_core.rs")

with open(DCTRL) as f:
    dl = f.read()
old_progress = '                    // Capture progress context from active slot before removal\n                    let _progress_ctx = {\n                        let q = shared_queue.queue.lock().unwrap();\n                        q.active_slot().map(|slot| (slot.progress_state.clone(), slot.slot_id))\n                    };'
new_progress = '                    // Capture progress context from active slot before removal\n                    let _progress_ctx = {\n                        let q = shared_queue.queue.lock().unwrap();\n                        q.active_slot().map(|slot| slot.slot_id)\n                    };'
assert old_progress in dl, "progress_ctx block not found in daemon_control.rs"
dl = dl.replace(old_progress, new_progress)
with open(DCTRL, "w") as f:
    f.write(dl)
print("ProgressState clone removed")
