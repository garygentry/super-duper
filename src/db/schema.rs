// @generated automatically by Diesel CLI.

diesel::table! {
    dupe_file (id) {
        id -> Int4,
        canonical_name -> Text,
        file_index -> Int8,
        #[max_length = 1]
        drive_letter -> Bpchar,
        path_no_drive -> Text,
        file_size -> Int8,
        last_modified -> Timestamp,
        content_hash -> Int8,
        volume_serial_number -> Int4,
        parent_dir -> Text,
    }
}
