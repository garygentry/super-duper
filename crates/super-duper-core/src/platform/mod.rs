#[cfg(target_os = "windows")]
pub mod windows;

use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSafety {
    Missing,
    File,
    Directory,
    CloudPlaceholder,
    ReparsePoint,
    Other,
}

#[cfg(target_os = "windows")]
pub fn classify_path_without_open(path: &Path) -> io::Result<PathSafety> {
    windows::classify_path_without_open(path)
}

#[cfg(not(target_os = "windows"))]
pub fn classify_path_without_open(path: &Path) -> io::Result<PathSafety> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(PathSafety::ReparsePoint),
        Ok(metadata) if metadata.is_file() => Ok(PathSafety::File),
        Ok(metadata) if metadata.is_dir() => Ok(PathSafety::Directory),
        Ok(_) => Ok(PathSafety::Other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(PathSafety::Missing),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
pub fn get_drive_letter(path: &Path) -> Option<OsString> {
    windows::get_drive_letter(path)
}

#[cfg(not(target_os = "windows"))]
pub fn get_drive_letter(_path: &Path) -> Option<OsString> {
    None
}

#[cfg(target_os = "windows")]
pub fn file_identity(path: &Path) -> io::Result<Option<String>> {
    windows::file_identity(path)
}

#[cfg(unix)]
pub fn file_identity(path: &Path) -> io::Result<Option<String>> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path)?;
    Ok(Some(format!(
        "{:016x}:{:016x}",
        metadata.dev(),
        metadata.ino()
    )))
}

#[cfg(not(any(target_os = "windows", unix)))]
pub fn file_identity(_path: &Path) -> io::Result<Option<String>> {
    Ok(None)
}

pub fn get_path_without_drive_letter(path: &Path) -> PathBuf {
    let components: Vec<_> = path.components().collect();
    let without_drive = components
        .iter()
        .skip_while(|comp| matches!(comp, Component::Prefix(_)));

    let mut result_path = PathBuf::new();
    for component in without_drive {
        result_path.push(component.as_os_str());
    }
    result_path
}
