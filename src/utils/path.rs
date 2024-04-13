use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub struct PathParts {
    pub drive_letter: Option<String>,
    pub path_without_drive: String,
    pub base_filename: String,
    pub extension: Option<String>,
    pub parent_dir: String,
}

fn get_drive_letter(path: &Path) -> Option<String> {
    for component in path.components() {
        if let Component::Prefix(prefix_comp) = component {
            match prefix_comp.kind() {
                std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                    // Convert the drive letter to a char, then create a String, and finally convert to OsString
                    let drive_letter = (letter as char).to_string();
                    return Some(OsString::from(drive_letter).to_string_lossy().into_owned());
                }
                // Handle other prefix types if necessary
                _ => (),
            }
        }
    }
    None
}

pub fn extract_path_components(path: &PathBuf) -> PathParts {
    let drive_letter = get_drive_letter(path);

    // Get the full path without the drive letter
    let path_without_drive = path
        .strip_prefix(
            drive_letter
                .as_ref()
                .map_or(Path::new(""), |d| Path::new(d)),
        )
        .unwrap_or_else(|_| path)
        .to_string_lossy()
        .into_owned();

    // Get the base file name (with no path)
    let base_filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    // Get the file extension
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    // Get the parent directory
    let parent_dir = path
        .parent()
        .and_then(|p| p.to_str().map(String::from))
        .expect("Failed to get parent directory or convert to string");

    // Return the extracted components as a PathParts structure
    PathParts {
        drive_letter,
        path_without_drive,
        base_filename,
        extension,
        parent_dir,
    }
}
