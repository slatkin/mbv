//! `mbv --desktop` resolves how to start from the desktop entry: pinned in a
//! pinwin panel when pinwin is installed, otherwise in the user's terminal.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;

/// Contract: the launcher prefers a pinned pinwin panel (`--no-tray`, so the
/// panel owns the only tray icon) over a terminal, and reports `None` when
/// neither launcher is installed.
pub fn desktop_command(on_path: impl Fn(&str) -> bool) -> Option<&'static [&'static str]> {
    if on_path("pinwin") {
        Some(&["pinwin", "--no-tray", "mbv"])
    } else if on_path("xdg-terminal-exec") {
        Some(&["xdg-terminal-exec", "mbv"])
    } else {
        None
    }
}

/// True when `name` resolves to an executable file in a `PATH` directory.
pub fn executable_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        std::fs::metadata(dir.join(name))
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

/// Replace this process with `argv`, exiting non-zero if the replacement fails.
pub fn exec(argv: &[&str]) -> ! {
    let error = std::process::Command::new(argv[0]).args(&argv[1..]).exec();
    eprintln!("mbv: failed to start {}: {error}", argv[0]);
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case(&["pinwin", "xdg-terminal-exec"], Some(&["pinwin", "--no-tray", "mbv"] as &[&str]))]
    #[case(&["xdg-terminal-exec"], Some(&["xdg-terminal-exec", "mbv"] as &[&str]))]
    #[case(&[], None)]
    fn desktop_command_prefers_pinwin_over_terminal(
        #[case] available: &[&str],
        #[case] expected: Option<&[&str]>,
    ) {
        assert_eq!(desktop_command(|name| available.contains(&name)), expected);
    }
}
