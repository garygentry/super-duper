// @generated automatically by Diesel CLI.

diesel::table! {
    dupe_file (id) {
        id -> Int4,
        canonical_name -> Text,
        #[max_length = 1]
        drive_letter -> Bpchar,
        path_no_drive -> Text,
        file_size -> Int8,
        last_modified -> Timestamp,
        content_hash -> Int8,
        parent_dir -> Text,
    }
}
