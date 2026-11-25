use serde::{Deserialize, Serialize};

/// An action the Owner process runs on request from the Tray or its one CLI
/// flag. Each variant has exactly one `mbv-tui` CLI flag; `cli_flag`, `help`,
/// `from_cli_flag`, and `ALL` are the only places a flag string lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum OwnerAction {
    /// Move the session between the pinned panel and a terminal by replacing
    /// one Client with another; the Owner keeps playing.
    #[serde(rename = "swap-panel")]
    SwapPanel,
    /// Start a fresh GUI Client window attached to this Owner, with no
    /// effect on any attached Client. The wire name predates the GUI Client
    /// and stays for compatibility.
    #[serde(rename = "open-terminal")]
    Open,
}

impl OwnerAction {
    /// Every Owner action, in help-listing order. Hand-kept: a variant
    /// missing from this list has no flag and no help row.
    pub const ALL: &'static [OwnerAction] = &[OwnerAction::SwapPanel, OwnerAction::Open];

    /// The one `mbv-tui` CLI flag that runs this action.
    #[must_use]
    pub fn cli_flag(self) -> &'static str {
        match self {
            OwnerAction::SwapPanel => "--swap-panel",
            OwnerAction::Open => "--open",
        }
    }

    /// One-line help text shown by `mbv-tui --help`.
    #[must_use]
    pub fn help(self) -> &'static str {
        match self {
            OwnerAction::SwapPanel => "move the session between the pinned panel and a terminal",
            OwnerAction::Open => "open the mbv window",
        }
    }

    /// Resolve a CLI flag to its action. Searches [`OwnerAction::ALL`], so a
    /// flag with no `ALL` entry is unreachable from the CLI.
    #[must_use]
    pub fn from_cli_flag(flag: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|action| action.cli_flag() == flag)
    }
}
