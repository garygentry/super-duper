#![allow(dead_code)]
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::File;
use std::hash::Hasher as _;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use twox_hash::XxHash64; // Trait needed for finish()

const HASH_LENGTH: usize = 1024; // 1KB

pub fn build_content_hash_map(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let partial_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();
    let confirmed_duplicates: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    let size_to_file_vec: Vec<_> = size_to_file_map.iter().collect();
    size_to_file_vec.par_iter().try_for_each(|files| {
        files
            .value()
            .par_iter()
            .try_for_each(|file| match read_portion(file) {
                Ok(data) => {
                    let hash = hash_data(&data)?;
                    partial_hash_to_file_map
                        .entry(hash)
                        .or_default()
                        .push(file.clone());
                    Ok(())
                }
                Err(e) => Err(e),
            })
    })?;

    let partial_hash_to_file_vec: Vec<_> = partial_hash_to_file_map.iter().collect();
    partial_hash_to_file_vec.par_iter().try_for_each(|files| {
        if files.value().len() > 1 {
            files
                .value()
                .par_iter()
                .try_for_each(|file| match read_full_file(file) {
                    Ok(data) => {
                        let hash = hash_data(&data)?;
                        confirmed_duplicates
                            .entry(hash)
                            .or_default()
                            .push(file.clone());
                        Ok(())
                    }
                    Err(e) => Err(e),
                })
        } else {
            Ok(())
        }
    })?;

    Ok(confirmed_duplicates)
}

fn read_portion(file: &PathBuf) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(file)?;
    let mut buffer = vec![0; HASH_LENGTH];

    // Read up to HASH_LENGTH bytes
    let bytes_read = f.read(&mut buffer)?;

    // Shrink the buffer to the actual number of bytes read
    buffer.truncate(bytes_read);

    Ok(buffer)
}

fn read_full_file(file: &PathBuf) -> io::Result<Vec<u8>> {
    let mut f = File::open(file)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn hash_data(data: &[u8]) -> io::Result<u64> {
    let mut hasher = XxHash64::with_seed(0); // Initialize hasher with a seed
    hasher.write(data);
    Ok(hasher.finish()) // Obtain the hash as u64
}
