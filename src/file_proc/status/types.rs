use std::path::PathBuf;

#[derive(Debug, Default, Clone, Copy)]
pub struct FileProcStats {
    pub scan_file_count: usize,
    pub scan_file_size: u64,
    pub scan_dupe_file_count: usize,
    pub scan_dupe_file_size: u64,

    pub hash_proc_scan_file_count: usize,
    pub hash_proc_scan_file_size: u64,
    pub hash_full_cache_hit_count: usize,
    pub hash_partial_cache_hit_count: usize,
    pub hash_partial_gen_count: usize,

    pub hash_full_gen_count: usize,
    pub hash_proc_confirmed_dupe_count: usize,

    pub cache_to_dupe_file_count: usize,
    pub db_dupe_file_insert_count: usize,
}

#[derive(Debug, Clone)]
pub struct ScanAddRawStatusMessage {
    pub file_path: PathBuf,
    pub file_size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct ScanAddDupeStatusMessage {
    pub count: usize,
    pub file_size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct HashProcStatusMessage {
    pub scan_file_proc_count: usize,
    pub full_cache_hit_count: usize,
    pub partial_cache_hit_count: usize,
    pub file_size: u64,
    pub confirmed_dupe_count: usize,
}

#[derive(Debug, Default, Clone)]
pub struct HashGenCacheFileStatusMessage {
    pub canonical_path: String,
    pub partial_count: usize,
    pub full_count: usize,
}

#[derive(Debug, Default, Clone)]
pub struct CacheToDupeProcStatusMessage {
    pub count: usize,
}

#[derive(Debug, Default, Clone)]
pub struct DbDupeFileInsertProcStatusMessage {
    pub rows_inserted: usize,
}

#[derive(Clone, Debug)]
pub enum StatusMessage {
    ProcessBegin,
    ScanBegin,
    ScanAddRaw(ScanAddRawStatusMessage),
    ScanAddDupe(ScanAddDupeStatusMessage),
    ScanEnd,
    HashBegin,
    HashProc(HashProcStatusMessage),
    HashGenCacheFile(HashGenCacheFileStatusMessage),
    HashEnd,
    CacheToDupeBegin,
    CacheToDupeProc(CacheToDupeProcStatusMessage),
    CacheToDupeEnd,
    DbDupeFileInsertBegin,
    DbDupeFileInsertProc(DbDupeFileInsertProcStatusMessage),
    DbDupeFileInsertEnd,
    ProcessEnd,
}
