/// Legacy v7 positional-index command variants, preserved for gated
/// deserialization when a v8 daemon accepts a v7 peer. These variants map
/// directly to the v7 wire tags and are only used during the one-release
/// compatibility window; after that window closes they will be deleted
/// along with the v7 acceptance path.
#[derive(Debug, Serialize, Deserialize)]
pub enum LegacyWireCommand {
    #[serde(rename = "PlaylistRemove")]
    QueueRemove(usize),
    #[serde(rename = "PlaylistMove")]
    QueueMove(usize, usize),
    #[serde(rename = "JumpTo")]
    JumpTo(usize),
}

impl From<LegacyWireCommand> for WireCommand {
    fn from(cmd: LegacyWireCommand) -> Self {
        match cmd {
            LegacyWireCommand::QueueRemove(idx) => WireCommand::QueueRemove(idx),
            LegacyWireCommand::QueueMove(from, to) => WireCommand::QueueMove(from, to),
            LegacyWireCommand::JumpTo(idx) => WireCommand::JumpTo(idx),
        }
    }
}
