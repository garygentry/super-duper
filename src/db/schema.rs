// @generated automatically by Diesel CLI.

diesel::table! {
    dedupe_session (id) {
        id -> Int4,
        run_start_time -> Timestamp,
        process_duration -> Float4,
        scan_duration -> Float4,
        scan_input_paths -> Text,
        scan_file_count -> Int4,
        scan_file_size -> Int8,
        scan_size_dupe_file_count -> Int4,
        scan_size_dupe_file_size -> Int8,
        hash_duration -> Float4,
        hash_scan_file_count -> Int4,
        hash_scan_file_size -> Int8,
        hash_cache_hit_full_count -> Int4,
        hash_cache_hit_partial_count -> Int4,
        hash_gen_partial_count -> Int4,
        hash_gen_partial_duration -> Float4,
        hash_gen_partial_file_size -> Int8,
        hash_gen_full_count -> Int4,
        hash_gen_full_file_size -> Int8,
        hash_gen_full_duration -> Float4,
        hash_confirmed_dupe_count -> Int4,
        hash_confirmed_dupe_size -> Int8,
        hash_confirmed_dupe_distinct_count -> Int4,
        hash_confirmed_dupe_distinct_size -> Int8,
        cache_map_to_dupe_vec_duration -> Float4,
        cache_map_to_dupe_vec_count -> Int4,
        db_dupe_file_insert_duration -> Float4,
        db_dupe_file_insert_count -> Int4,
    }
}

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
        session_id -> Int4,
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
        session_id -> Int4,
    }
}

diesel::joinable!(dupe_file -> dedupe_session (session_id));
diesel::joinable!(dupe_file_part -> dedupe_session (session_id));

diesel::allow_tables_to_appear_in_same_query!(
    dedupe_session,
    dupe_file,
    dupe_file_part,
);
