#![allow(dead_code)]
pub mod path;
pub mod prompt;
pub mod stats;

use std::path::{ Path, PathBuf };

pub fn to_non_overlapping_directories(dirs: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    for dir in dirs {
        let dir_path = Path::new(&dir)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(dir)); // Convert to absolute path
        let mut should_add = true;

        let result_clone = result.clone(); // Clone result for comparison

        for res_dir in &result_clone {
            let res_dir_path = Path::new(res_dir)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(res_dir)); // Convert to absolute path

            // Check if dir_path is a subdirectory of res_dir_path
            if dir_path.starts_with(&res_dir_path) {
                should_add = false;
                break;
            }

            // Check if res_dir_path is a subdirectory of dir_path
            if res_dir_path.starts_with(&dir_path) {
                // If res_dir_path is a subdirectory of dir_path, remove it
                result.retain(|x| x != res_dir);
                break;
            }
        }

        if should_add {
            result.push(dir_path.to_string_lossy().to_string()); // Convert back to String
        }
    }

    result
}
