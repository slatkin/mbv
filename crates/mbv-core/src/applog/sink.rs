use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_FILE_SIZE: u64 = 5_000_000;
const GENERATIONS: u8 = 3;

#[derive(Debug)]
pub struct FileSink {
    path: PathBuf,
    file: Option<File>,
    len: u64,
    max_size: u64,
    warned: bool,
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
        if self.file.is_none() {
            return;
        }
        let line_len = line.len() as u64 + 1;
        if self.len.saturating_add(line_len) > self.max_size {
            self.rotate();
        }
        if let Some(file) = self.file.as_mut()
            && file.write_all(line.as_bytes()).is_ok()
            && file.write_all(b"\n").is_ok()
        {
            self.len = self.len.saturating_add(line_len);
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

    // Returns true only when rotation and reopening succeed; failures preserve the open file.
    fn rotate(&mut self) -> bool {
        let result = rotate_paths(&self.path).and_then(|()| open_append(&self.path));
        match result {
            Ok((file, len)) => {
                self.file = Some(file);
                self.len = len;
                true
            }
            Err(error) => {
                self.warn(&error);
                false
            }
        }
    }

    fn warn(&mut self, error: &std::io::Error) {
        if !self.warned {
            eprintln!("mbv: log file {}: {error}", self.path.display());
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
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mbv-log-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn rotation_shifts_and_caps_three_generations() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).expect("create temp directory");
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
        fs::remove_dir_all(dir).expect("remove temp directory");
    }
}
