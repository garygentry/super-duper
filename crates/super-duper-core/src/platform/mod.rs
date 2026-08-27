#[cfg(target_os = "windows")]
pub mod windows;

use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(crate) const UNKNOWN_STORAGE_DEVICE_KEY: &str = "storage:mapping-unavailable";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StorageMediaClass {
    Rotational,
    SolidState,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageDevice {
    pub key: String,
    pub media: StorageMediaClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContentSignatureMetadata {
    pub stable_identity: Option<String>,
    pub size: u64,
    pub modified_unix_nanos: Option<i64>,
    pub modified_time_is_coarse: bool,
    pub content_change_token: Option<String>,
}

impl StorageDevice {
    pub(crate) fn mapping_unavailable() -> Self {
        Self {
            key: UNKNOWN_STORAGE_DEVICE_KEY.to_owned(),
            media: StorageMediaClass::Unknown,
        }
    }
}

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
pub(crate) fn storage_device_for_path(path: &Path) -> StorageDevice {
    windows::storage_device_for_path(path)
}

#[cfg(all(not(target_os = "windows"), unix))]
pub(crate) fn storage_device_for_path(path: &Path) -> StorageDevice {
    use std::os::unix::fs::MetadataExt;

    std::fs::metadata(path)
        .map(|metadata| StorageDevice {
            key: format!("unix-device:{:016x}", metadata.dev()),
            media: StorageMediaClass::Unknown,
        })
        .unwrap_or_else(|_| StorageDevice::mapping_unavailable())
}

#[cfg(not(any(target_os = "windows", unix)))]
pub(crate) fn storage_device_for_path(_path: &Path) -> StorageDevice {
    StorageDevice::mapping_unavailable()
}

#[cfg(target_os = "windows")]
pub fn file_identity(path: &Path) -> io::Result<Option<String>> {
    windows::file_identity(path)
}

#[cfg(target_os = "windows")]
pub(crate) fn content_signature_metadata(path: &Path) -> io::Result<ContentSignatureMetadata> {
    windows::content_signature_metadata(path)
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

#[cfg(unix)]
pub(crate) fn content_signature_metadata(path: &Path) -> io::Result<ContentSignatureMetadata> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path)?;
    let modified_unix_nanos = metadata
        .mtime()
        .checked_mul(1_000_000_000)
        .and_then(|seconds| seconds.checked_add(metadata.mtime_nsec()))
        .filter(|value| *value > 0);
    Ok(ContentSignatureMetadata {
        stable_identity: Some(format!("{:016x}:{:016x}", metadata.dev(), metadata.ino())),
        size: metadata.len(),
        modified_unix_nanos,
        modified_time_is_coarse: metadata.mtime_nsec() == 0,
        content_change_token: Some(format!(
            "unix-ctime:{:016x}:{:08x}",
            metadata.ctime(),
            metadata.ctime_nsec()
        )),
    })
}

#[cfg(not(any(target_os = "windows", unix)))]
pub fn file_identity(_path: &Path) -> io::Result<Option<String>> {
    Ok(None)
}

#[cfg(not(any(target_os = "windows", unix)))]
pub(crate) fn content_signature_metadata(path: &Path) -> io::Result<ContentSignatureMetadata> {
    let metadata = std::fs::metadata(path)?;
    Ok(ContentSignatureMetadata {
        stable_identity: None,
        size: metadata.len(),
        modified_unix_nanos: None,
        modified_time_is_coarse: true,
        content_change_token: None,
    })
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
