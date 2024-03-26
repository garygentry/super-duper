#![allow(dead_code)]

use crossterm::{cursor, terminal, ExecutableCommand, QueueableCommand};
use std::io::{stdout, Write};
use std::thread_local;

thread_local! {
    static STDOUT: std::io::Stdout = stdout();
}

fn compress_file_path(path: &str) -> String {
    let max_length = 100;
    let len = path.len();

    // If the path is already shorter than max_length, return it as is
    if len <= max_length {
        return path.to_string();
    }

    let start_len = (max_length - 3) / 2;
    let end_len = max_length - start_len - 3;

    let mut compressed_path = String::new();
    compressed_path.push_str(&path[..start_len]);
    compressed_path.push_str("...");
    compressed_path.push_str(&path[len - end_len..]);

    compressed_path
}

pub fn print_status(status: &str) {
    // Capture the status in a local variable to ensure it lives long enough
    let compressed_status = compress_file_path(status);
    // let compressed_status = status.replace('\n', "");

    // Access stdout using thread-local storage
    STDOUT.with(|stdout| {
        // Lock stdout for writing
        let mut stdout = stdout.lock();

        // Save cursor position
        stdout.queue(cursor::SavePosition).unwrap();

        // Write compressed status to stdout
        stdout.write_all(compressed_status.as_bytes()).unwrap();

        // Restore cursor position
        stdout.queue(cursor::RestorePosition).unwrap();

        // Flush stdout to ensure the message is immediately displayed
        stdout.flush().unwrap();

        // Move cursor to the beginning of the next line and clear to the end of the line
        stdout.queue(cursor::RestorePosition).unwrap();
        stdout
            .queue(terminal::Clear(terminal::ClearType::FromCursorDown))
            .unwrap();
    });
}
