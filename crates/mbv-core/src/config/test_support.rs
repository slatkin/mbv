// Test-only scratch-directory helper. Included into `config`'s module scope
// (see `config.rs`) so callers reach it as `crate::config::TestTempDir`, next
// to `TestStateDirGuard` in `config_types_paths.rs`.

/// Scratch directory for tests that must exercise a real filesystem path.
///
/// Removes itself on drop -- including when the test panics -- so a failing
/// run cannot accumulate directories under the system temp dir.
#[cfg(any(test, feature = "test-support"))]
pub struct TestTempDir {
    dir: PathBuf,
    xdg_home: bool,
    prev_state_home: Option<std::ffi::OsString>,
    prev_config_home: Option<std::ffi::OsString>,
}

#[cfg(any(test, feature = "test-support"))]
impl TestTempDir {
    /// Fresh `mbv-test-<uuid>` directory under the system temp dir.
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            xdg_home: false,
            prev_state_home: std::env::var_os("XDG_STATE_HOME"),
            prev_config_home: std::env::var_os("XDG_CONFIG_HOME"),
        }
    }

    /// Also makes this the process-wide `XDG_STATE_HOME`/`XDG_CONFIG_HOME`, so
    /// code that resolves `state_dir()`/`config_dir()` on a *spawned* thread
    /// (which the thread-local `TestStateDirGuard` cannot reach) lands here
    /// rather than in the process-wide fallback dir, which is never removed.
    ///
    /// Consumes and returns `self` (rather than lending) so the caller's
    /// `let _scratch = TestTempDir::new().as_xdg_home();` binds an owned guard
    /// that lives to the end of the test -- a borrowed return would drop the
    /// temporary at the end of that statement, restoring the env and deleting
    /// the directory before the test body ran.
    pub fn as_xdg_home(mut self) -> Self {
        std::env::set_var("XDG_STATE_HOME", &self.dir);
        std::env::set_var("XDG_CONFIG_HOME", &self.dir);
        std::env::remove_var("MBV_SYSTEM");
        self.xdg_home = true;
        self
    }

    pub fn path(&self) -> &std::path::Path {
        &self.dir
    }

    pub fn join(&self, name: impl AsRef<std::path::Path>) -> PathBuf {
        self.dir.join(name)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Default for TestTempDir {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for TestTempDir {
    fn drop(&mut self) {
        // Restore before deleting: a later test in the same process must never
        // inherit a scratch path that is about to disappear.
        if self.xdg_home {
            restore_env("XDG_STATE_HOME", self.prev_state_home.take());
            restore_env("XDG_CONFIG_HOME", self.prev_config_home.take());
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(any(test, feature = "test-support"))]
fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}

// Test-only escape hatch: `state_dir()` (and therefore `queue_state_path()`,
// `save_queue_state`/`load_queue_state`/`clear_queue_state`) is used not just
// by tests that are explicitly *about* path resolution, but incidentally by
// any test that drives consume-mode/queue logic through `App` methods like
// `save_queue_state()` -- e.g. tests that fire `PlayerEvent::Stopped` and
// assert on in-memory queue state have no reason to care where the file
// lands, so historically nobody bothered to isolate them. But
// `XDG_STATE_HOME`/`MBV_SYSTEM` are process-global env vars: an unguarded
// test's call to `state_dir()` observes whatever value another, *properly
// locked* test happens to have set at that exact moment (env vars have no
// per-thread scoping), so it can transiently read -- and write into --
// a locked test's private tempdir mid-race, corrupting it. A thread-local
// override sidesteps the whole problem for these incidental callers: it's
// only visible on the thread that set it, so two tests running on different
// threads can never observe (or clobber) each other's override, no lock
// required. See `TestStateDirGuard` and issue #106.
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_STATE_DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

// `config_dir()` (config.toml, via `save_config_settings`) gets the exact
// same treatment as `state_dir()` above, for the exact same reason: some
// App-level settings toggles (`cycle_subtitle_mode`, `handle_library_routes_enter`,
// closing a multiselect settings popup) write to disk synchronously rather
// than through the debounced `settings_save_at` path, so any unguarded test
// that reaches one of them writes straight into the developer's real
// config.toml. `TestStateDirGuard` sets this override alongside its own so
// every test that already uses it (including, automatically, every `App`
// built in a test binary via `_test_state_dir_guard`) gets both for free.
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_CONFIG_DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
static TEST_DEFAULT_STATE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(any(test, feature = "test-support"))]
pub struct TestStateDirGuard;

#[cfg(any(test, feature = "test-support"))]
impl TestStateDirGuard {
    /// Points `state_dir()` at a fresh, unique tempdir for the lifetime of
    /// this guard, visible only on the calling thread. Use this in any test
    /// that drives `App` logic which might incidentally call
    /// `save_queue_state`/`restore_queue_state` (e.g. via consume-mode event
    /// handling) but isn't itself testing path resolution -- so it never
    /// touches a real on-disk path or races a sibling test.
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
        Self::new_at(dir)
    }

    /// Points `state_dir()` (and `config_dir()`) at `dir` for the lifetime
    /// of this guard. Both point at the same directory -- their file names
    /// never collide (`config.toml` vs. `prefs.json`/`token.json`/
    /// `queue_state.json`) -- so one guard, one tempdir, one cleanup.
    pub fn new_at(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let _ = std::fs::create_dir_all(&dir);
        TEST_STATE_DIR_OVERRIDE.with(|c| *c.borrow_mut() = Some(dir.clone()));
        TEST_CONFIG_DIR_OVERRIDE.with(|c| *c.borrow_mut() = Some(dir));
        TestStateDirGuard
    }

    /// Installs a fresh override only when this thread does not already have
    /// one. This lets broad app-test fixtures isolate incidental queue-state
    /// writes without shadowing tests that explicitly seeded queue state first.
    pub fn new_if_unset() -> Option<Self> {
        if TEST_STATE_DIR_OVERRIDE.with(|c| c.borrow().is_some()) {
            None
        } else {
            Some(Self::new())
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Default for TestStateDirGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for TestStateDirGuard {
    fn drop(&mut self) {
        // Both overrides point at the same physical directory (see
        // `new_at`) and are always set/cleared together, so only one
        // `take()` needs to delete the directory -- but both thread-locals
        // must be cleared regardless, or the config override would keep
        // pointing at a directory this guard is about to delete.
        let dir = TEST_STATE_DIR_OVERRIDE.with(|c| c.borrow_mut().take());
        TEST_CONFIG_DIR_OVERRIDE.with(|c| c.borrow_mut().take());
        if let Some(dir) = dir {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
