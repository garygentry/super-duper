use dashmap::DashMap;
use std::path::PathBuf;

pub fn print_size_to_files_map(map: &DashMap<u64, Vec<PathBuf>>) {
    for entry in map.iter() {
        let (key, value) = entry.pair();
        println!("Key (file size): {}", key);
        for path in value.iter() {
            println!("\t{:?}", path);
        }
    }
}

pub fn print_content_hash_map(checksum_map: &DashMap<u64, Vec<PathBuf>>) {
    for entry in checksum_map.iter() {
        let (checksum, paths) = entry.pair();
        println!("Hash: {}", checksum);
        for path in paths.iter() {
            println!("\tFile: {:?}", path);
        }
        println!();
    }
}

pub fn print_file_info_vec(file_info_vec: &Vec<super::model::FileInfo>) {
    for file_info in file_info_vec {
        // file_info.print();
        println!("{:?}", file_info)
    }
}
