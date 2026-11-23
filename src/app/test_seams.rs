//! Test seams shared through the core (`decouple-client-core` design D3). These
//! seams name no toolkit, so they stay here: only the seams that name
//! crossterm, tuirealm, or `mbv-components` move to `src/app/shell/test_seams.rs`.
//!
//! The session lookup and the two connect paths are read by shell code alone
//! (the terminal listener's Sessions and library-route paths), so they live in
//! the shell's seam module. The URL opener stays below because it names no
//! toolkit and both Clients reach it (the GUI Library `openUrl` command and the
//! TUI Hero link).

use std::sync::Mutex;

// Test seam for URL opens (the GUI Library `openUrl` command, the Hero's IMDb
// link): the command path reads it after the scheme gate, so a test observes
// the url the opener receives without a real `xdg-open`/`open` spawn.
#[cfg(test)]
pub(in crate::app) type OpenUrlFn = fn(&str);
#[cfg(test)]
pub(in crate::app) static OPEN_URL_OVERRIDE: Mutex<Option<OpenUrlFn>> = Mutex::new(None);
#[cfg(test)]
pub(in crate::app) static OPEN_URL_TEST_LOCK: Mutex<()> = Mutex::new(());
