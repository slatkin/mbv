//! Build script: embeds the build identity as `BUILD_ID` (design D3,
//! emby-session-output-followups task 4.1). The identity is the git short
//! SHA plus `-dirty` when the tree is dirty, or `unknown` when git is
//! unavailable (tarball builds).
//!
//! Rerun tracking names only git paths that exist: `git rev-parse
//! --git-path HEAD`, the symbolic ref's `--git-path`, the branch ref file
//! under `<git-common-dir>/refs/heads/<branch>`, the `packed-refs`
//! report, and the per-worktree HEAD reflog (`--git-path logs/HEAD`).
//! The common-dir path is emitted as well as the per-worktree one
//! because a shared branch's worktree path does not exist. The reflog is
//! the fallback that keeps the identity fresh across commits when the
//! branch ref was packed at build time: `git pack-refs` folds the loose
//! ref into `packed-refs`, which a later commit never rewrites, so the
//! reflog (appended by every commit) is the only watched path that
//! changes. A literal `.git/HEAD` is never named — in a worktree `.git`
//! is a file, and Cargo reruns a build script on every build when a named
//! path is missing, which would recompile this crate and every dependent
//! each time. When git is unavailable no git path is emitted at all, so
//! the default whole-project tracking applies.

use std::process::Command;

/// Runs `git` with `args` and returns its trimmed stdout, or `None` when
/// git is unavailable or the command fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    Some(stdout.trim().to_string())
}

/// Emits `cargo:rerun-if-changed` for `path` only when it exists: a missing
/// path makes Cargo rerun the script on every build.
fn rerun_if_changed(path: &str) {
    if std::fs::metadata(path).is_ok() {
        println!("cargo:rerun-if-changed={path}");
    }
}

/// Watches the git path `args` reports (a `git rev-parse` path query): emits
/// nothing when git fails or the path does not exist.
fn watch_git_path(args: &[&str]) {
    if let Some(path) = git(args) {
        rerun_if_changed(&path);
    }
}

fn main() {
    // Everything below keys off git; without it the identity is a constant
    // `unknown` and no rerun path is emitted.
    let Some(head_sha) = git(&["rev-parse", "--short=12", "HEAD"]) else {
        println!("cargo:rustc-env=BUILD_ID=unknown");
        return;
    };

    // `--porcelain` output is empty exactly when the worktree is clean;
    // ignored files (build artifacts) never appear.
    let dirty = git(&["status", "--porcelain"]).is_some_and(|status| !status.is_empty());
    let build_id = if dirty {
        format!("{head_sha}-dirty")
    } else {
        head_sha
    };
    println!("cargo:rustc-env=BUILD_ID={build_id}");

    watch_git_path(&["rev-parse", "--git-path", "HEAD"]);
    // The branch ref file: `--git-path` returns the per-worktree path, but
    // a shared branch lives in the common dir, so both candidates are
    // emitted when they exist. A detached HEAD has no symbolic ref at all.
    if let Some(branch) = git(&["symbolic-ref", "--short", "HEAD"]) {
        watch_git_path(&["rev-parse", "--git-path", &branch]);
        if let Some(common_dir) = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
        {
            rerun_if_changed(&format!("{common_dir}/refs/heads/{branch}"));
        }
    }
    watch_git_path(&["rev-parse", "--git-path", "packed-refs"]);
    // The per-worktree HEAD reflog: every commit appends to it, so it stays
    // the commit signal even when the branch ref has been packed into
    // `packed-refs` (which a commit never rewrites).
    watch_git_path(&["rev-parse", "--git-path", "logs/HEAD"]);
}
