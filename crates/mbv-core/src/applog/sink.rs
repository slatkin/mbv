//! Size-based log-file rotation for the application logger.
//!
//! `FileSink` appends one rendered log line at a time and shifts numbered
//! generations when the active file reaches its configured size. Open, rotate,
//! and write failures are reported to stderr at most once per sink; failed
//! writes are counted and dropped. The one-shot failure notice below is one of
//! the sanctioned direct-stderr-write paths (with the applog stderr sink layer
//! and the palette.json maintainer hint), the deliberate escape hatch where the
//! print macros are denied (issue #908, documented in the lint headers).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Default maximum size before rotation, per the application logging spec's
/// "Log files rotate by size" requirement.
const MAX_FILE_SIZE: u64 = 5_000_000;
/// Number of retained rotated files, per the application logging spec's
/// "Log files rotate by size" requirement.
const GENERATIONS: u8 = 3;

#[derive(Debug)]
pub struct FileSink {
    path: PathBuf,
    file: Option<File>,
    len: u64,
    max_size: u64,
    warned: bool,
    write_failures: u64,
}

impl FileSink {
    #[must_use]
    pub fn new(path: PathBuf, max_size: u64) -> Self {
        let mut sink = Self {
            path,
            file: None,
            len: 0,
            max_size,
            warned: false,
            write_failures: 0,
        };
        sink.open();
        if sink.len > sink.max_size {
            sink.rotate();
        }
        sink
    }

    #[must_use]
    pub fn at_default_size(path: PathBuf) -> Self {
        Self::new(path, MAX_FILE_SIZE)
    }

    pub fn write_line(&mut self, line: &str) {
        self.write_line_with(line, Write::write_all);
    }

    fn write_line_with(
        &mut self,
        line: &str,
        write: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
    ) {
        if self.file.is_none() {
            return;
        }
        let line_len = line.len() as u64 + 1;
        if self.len.saturating_add(line_len) > self.max_size {
            self.rotate();
        }
        let mut record = String::with_capacity(line.len() + 1);
        record.push_str(line);
        record.push('\n');
        let result = self
            .file
            .as_mut()
            .map(|file| write(file, record.as_bytes()));
        match result {
            Some(Ok(())) => self.len = self.len.saturating_add(line_len),
            Some(Err(error)) => {
                self.write_failures = self.write_failures.saturating_add(1);
                self.warn(&error);
            }
            None => {}
        }
    }

    fn open(&mut self) {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            && let Err(error) = fs::create_dir_all(parent)
        {
            self.warn(&error);
            return;
        }
        match open_append(&self.path) {
            Ok((file, len)) => {
                self.file = Some(file);
                self.len = len;
            }
            Err(error) => self.warn(&error),
        }
    }

    // A rename-chain failure keeps writing to the current file (design D8);
    // only `adopt_reopen` may retire the handle.
    fn rotate(&mut self) -> bool {
        match rotate_paths(&self.path) {
            Err(error) => {
                self.warn(&error);
                false
            }
            Ok(()) => self.adopt_reopen(open_append(&self.path)),
        }
    }

    // Contract: a reopen failure after a successful rename chain must drop the
    // stale handle. It points at the file just renamed to `.log.1`, so keeping
    // it would append every later line into a rotated generation that the next
    // rotation deletes, without ever recreating `path`. The sink degrades to
    // stderr-only output instead.
    fn adopt_reopen(&mut self, result: std::io::Result<(File, u64)>) -> bool {
        match result {
            Ok((file, len)) => {
                self.file = Some(file);
                self.len = len;
                true
            }
            Err(error) => {
                self.file = None;
                self.warn(&error);
                false
            }
        }
    }

    fn warn(&mut self, error: &std::io::Error) {
        if !self.warned {
            // The log sink cannot report a failed write through logging without
            // recursing into itself; this one-shot notice writes straight to
            // stderr instead (direct write — `print_stderr` is denied in
            // library crates, issue #908).
            let message = format!("mbv: log file {}: {error}\n", self.path.display());
            let _ = std::io::stderr().write_all(message.as_bytes());
            self.warned = true;
        }
    }
}

fn open_append(path: &Path) -> std::io::Result<(File, u64)> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let len = file.metadata()?.len();
    Ok((file, len))
}

fn rotate_paths(path: &Path) -> std::io::Result<()> {
    let oldest = generation_path(path, GENERATIONS);
    match fs::remove_file(oldest) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    for generation in (1..GENERATIONS).rev() {
        rename_if_exists(
            &generation_path(path, generation),
            &generation_path(path, generation + 1),
        )?;
    }
    rename_if_exists(path, &generation_path(path, 1))
}

fn generation_path(path: &Path, generation: u8) -> PathBuf {
    let mut path = path.as_os_str().to_owned();
    path.push(format!(".{generation}"));
    PathBuf::from(path)
}

fn rename_if_exists(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_config::TestTempDir;

    #[test]
    fn rotation_shifts_and_caps_three_generations() {
        let dir = TestTempDir::new();
        let path = dir.join("mbv.log");
        let mut sink = FileSink::new(path.clone(), 4);

        sink.write_line("one");
        sink.write_line("two");
        sink.write_line("tri");
        sink.write_line("for");
        sink.write_line("fiv");

        assert_eq!(fs::read_to_string(&path).expect("current log"), "fiv\n");
        assert_eq!(
            fs::read_to_string(generation_path(&path, 1)).expect("generation 1"),
            "for\n"
        );
        assert_eq!(
            fs::read_to_string(generation_path(&path, 2)).expect("generation 2"),
            "tri\n"
        );
        assert_eq!(
            fs::read_to_string(generation_path(&path, 3)).expect("generation 3"),
            "two\n"
        );
        assert!(!generation_path(&path, 4).exists());

        drop(sink);
    }

    #[test]
    fn write_failures_are_counted_and_warned_once() {
        let dir = TestTempDir::new();
        let mut sink = FileSink::new(dir.join("mbv.log"), 1000);
        let fail_write = |_: &mut File, _: &[u8]| Err(std::io::Error::other("write failed"));

        sink.write_line_with("one", fail_write);
        sink.write_line_with("two", fail_write);

        assert_eq!(sink.write_failures, 2);
        assert!(sink.warned);
    }
}
