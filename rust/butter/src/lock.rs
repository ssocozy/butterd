//! fs2 uses the supported Unix flock API on Android. Rust 1.90 std file locks do not.
use std::{fs::{self, File, OpenOptions}, io, os::unix::fs::OpenOptionsExt, path::Path};
pub struct FileLock { _file: File }
impl FileLock {
    pub fn acquire(path: &Path) -> io::Result<Option<Self>> {
        match fs::symlink_metadata(path) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => return Err(io::Error::other("Invalid lock file")),
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        let file = OpenOptions::new().create(true).truncate(false).read(true).write(true).mode(0o600).open(path)?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Self { _file:file })),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }
    pub fn held(path: &Path) -> io::Result<bool> {
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
            _ => {}
        }
        Ok(Self::acquire(path)?.is_none())
    }
}

#[derive(Debug)]
pub struct Busy;
impl std::fmt::Display for Busy { fn fmt(&self, f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Another control operation is running; retry shortly") } }
impl std::error::Error for Busy {}
