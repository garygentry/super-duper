use std::ffi::OsString;
use std::path::{Component, Path};

pub fn get_drive_letter(path: &Path) -> Option<OsString> {
    for component in path.components() {
        if let Component::Prefix(prefix_comp) = component {
            match prefix_comp.kind() {
                std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                    let drive_letter = (letter as char).to_string();
                    return Some(OsString::from(drive_letter));
                }
                _ => (),
            }
        }
    }
    None
}
