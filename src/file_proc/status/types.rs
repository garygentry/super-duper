use std::{ path::PathBuf, time::Duration };

#[derive(Debug, Clone)]
pub struct ProcessStartStatusMessage {
    pub input_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScanStartStatusMessage {
    pub input_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScanAddInputFileStatusMessage {
    pub file_path: PathBuf,
    pub file_size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct ScanAddSizeDupeStatusMessage {
    pub count: usize,
    pub file_size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct HashProcStatusMessage {
    pub scan_file_count: usize,
    pub cache_hit_full_count: usize,
    pub cache_hit_partial_count: usize,
    pub file_size: u64,
    pub confirmed_dupe_count: usize,
}

#[derive(Clone, Debug)]
pub enum HashGenerateType {
    Partial,
    Full,
}

#[derive(Debug, Clone)]
pub struct HashStartGenerateFileHashStatusMessage {
    pub canonical_path: String,
    pub hash_type: HashGenerateType,
}
#[derive(Debug, Clone)]
pub struct HashFinishGenerateFileHashStatusMessage {
    pub canonical_path: String,
    pub hash_type: HashGenerateType,
    pub file_size: u64,
    pub duration: Duration,
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
    ProcessStart(ProcessStartStatusMessage),
    ScanStart(ScanStartStatusMessage),
    ScanAddInputFile(ScanAddInputFileStatusMessage),
    ScanAddRetainedFile(ScanAddSizeDupeStatusMessage),
    ScanFinish,
    HashStart,
    HashProc(HashProcStatusMessage),
    HashStartGenerateFileHash(HashStartGenerateFileHashStatusMessage),
    HashFinishGenerateFileHash(HashFinishGenerateFileHashStatusMessage),
    HashFinish,
    CacheToDupeStart,
    CacheToDupeProc(CacheToDupeProcStatusMessage),
    CacheToDupeFinish,
    // DbDupeFileInsertStart,
    // DbDupeFileInsertProc(DbDupeFileInsertProcStatusMessage),
    // DbDupeFileInsertFinish,
    ProcessFinish,
}
