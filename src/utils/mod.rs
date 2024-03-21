use std::{
    io::{self, Write},
    path::Path,
};

pub fn hide_cursor() {
    print!("\x1B[?25l");
    io::stdout().flush().unwrap();
}

pub fn show_cursor() {
    print!("\x1B[?25h");
    io::stdout().flush().unwrap();
}

pub fn non_overlapping_directories(dirs: Vec<String>) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    for dir in dirs {
        let dir_path = Path::new(&dir);
        let mut should_add = true;

        let result_clone = result.clone(); // Clone result for comparison

        for res_dir in &result_clone {
            let res_dir_path = Path::new(res_dir);

            // Check if dir_path is a subdirectory of res_dir_path
            if dir_path.starts_with(res_dir_path) {
                should_add = false;
                break;
            }

            // Check if res_dir_path is a subdirectory of dir_path
            if res_dir_path.starts_with(dir_path) {
                // If res_dir_path is a subdirectory of dir_path, remove it
                result.retain(|x| x != res_dir);
                break;
            }
        }

        if should_add {
            result.push(dir);
        }
    }

    result
}
