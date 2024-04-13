// @generated automatically by Diesel CLI.

diesel::table! {
    dupe_file (id) {
        id -> Int4,
        canonical_path -> Text,
        path_no_drive -> Text,
        file_name -> Text,
        file_extension -> Nullable<Text>,
        #[max_length = 1]
        drive_letter -> Nullable<Bpchar>,
        parent_dir -> Text,
        file_size -> Int8,
        last_modified -> Timestamp,
        full_hash -> Int8,
        partial_hash -> Int8,
    }
}

diesel::table! {
    dupe_file_part (id) {
        id -> Int4,
        canonical_path -> Text,
        name -> Text,
        file_size -> Int8,
        part_type -> Int4,
        parent_id -> Nullable<Int4>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    dupe_file,
    dupe_file_part,
);
