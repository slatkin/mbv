//! ctrl-protocol-conformance task 3.1 (design D6): the Owner's decode path —
//! the `serde_json::from_str::<CtrlCmd>` call in the `core_ctrl_spawn.rs`
//! ctrl line loop — must accept every command fixture in the shared v10
//! corpus, so a wire shape the Client can send never fails to decode in the
//! packaged owner (`mbvd` runs `mbv-daemon`).

use std::fs;
use std::path::{Path, PathBuf};

use mbv_ctrl::CtrlCmd;

fn commands_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocol/ctrl/v10/fixtures/commands")
}

/// Every command fixture decodes through the same call the daemon's ctrl
/// line loop uses; the panic names the failing file.
#[test]
fn every_command_fixture_decodes_as_a_ctrl_cmd() {
    let dir = commands_dir();
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            panic!(
                "command fixture corpus unreadable at {}: {e}",
                dir.display()
            )
        })
        .map(|entry| entry.expect("fixture dir entry readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "command fixture corpus is empty at {}",
        dir.display()
    );
    for path in paths {
        let line = fs::read_to_string(&path).expect("fixture readable");
        if let Err(e) = serde_json::from_str::<CtrlCmd>(&line) {
            panic!("{} does not decode as CtrlCmd: {e}", path.display());
        }
    }
}
