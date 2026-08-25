PRAGMA user_version = 1;

CREATE TABLE IF NOT EXISTS status_run (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE CHECK(length(operation_id) BETWEEN 1 AND 128),
    product_run_id INTEGER,
    metrics_contract_version INTEGER NOT NULL CHECK(metrics_contract_version > 0),
    engine_version TEXT NOT NULL,
    worker_version TEXT,
    app_version TEXT,
    product_schema_version INTEGER,
    input_signature TEXT NOT NULL CHECK(length(input_signature) BETWEEN 1 AND 256),
    state TEXT NOT NULL CHECK(state IN
        ('pending', 'running', 'cancelling', 'completed', 'cancelled', 'failed', 'interrupted')),
    started_unix_millis INTEGER,
    completed_unix_millis INTEGER,
    last_monotonic_nanos INTEGER NOT NULL DEFAULT 0 CHECK(last_monotonic_nanos >= 0),
    last_sequence INTEGER NOT NULL DEFAULT 0 CHECK(last_sequence >= 0),
    error_code TEXT,
    error_message TEXT,
    created_unix_millis INTEGER NOT NULL,
    updated_unix_millis INTEGER NOT NULL,
    CHECK(completed_unix_millis IS NULL OR started_unix_millis IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS status_phase (
    run_id INTEGER NOT NULL REFERENCES status_run(id) ON DELETE CASCADE,
    phase TEXT NOT NULL CHECK(phase IN
        ('discovering', 'candidate_screening', 'full_hashing', 'persisting',
         'analyzing_folders', 'finalizing')),
    state TEXT NOT NULL CHECK(state IN
        ('pending', 'running', 'completed', 'cancelled', 'failed', 'interrupted')),
    started_monotonic_nanos INTEGER CHECK(started_monotonic_nanos IS NULL OR started_monotonic_nanos >= 0),
    completed_monotonic_nanos INTEGER CHECK(completed_monotonic_nanos IS NULL OR completed_monotonic_nanos >= 0),
    active_nanos INTEGER NOT NULL DEFAULT 0 CHECK(active_nanos >= 0),
    PRIMARY KEY(run_id, phase),
    CHECK(completed_monotonic_nanos IS NULL OR started_monotonic_nanos IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS status_counter (
    run_id INTEGER NOT NULL REFERENCES status_run(id) ON DELETE CASCADE,
    phase TEXT NOT NULL CHECK(phase IN
        ('overall', 'discovering', 'candidate_screening', 'full_hashing', 'persisting',
         'analyzing_folders', 'finalizing')),
    metric TEXT NOT NULL,
    value INTEGER NOT NULL CHECK(value >= 0),
    updated_sequence INTEGER NOT NULL CHECK(updated_sequence >= 0),
    PRIMARY KEY(run_id, phase, metric)
);

CREATE TABLE IF NOT EXISTS status_device (
    run_id INTEGER NOT NULL REFERENCES status_run(id) ON DELETE CASCADE,
    device_key TEXT NOT NULL CHECK(length(device_key) BETWEEN 1 AND 256),
    volume_key TEXT NOT NULL CHECK(length(volume_key) BETWEEN 1 AND 256),
    filesystem TEXT,
    capacity_bytes INTEGER CHECK(capacity_bytes IS NULL OR capacity_bytes >= 0),
    free_bytes_at_start INTEGER CHECK(free_bytes_at_start IS NULL OR free_bytes_at_start >= 0),
    bus_type TEXT,
    media_type TEXT,
    model TEXT,
    PRIMARY KEY(run_id, device_key, volume_key)
);

CREATE TABLE IF NOT EXISTS status_host_sample (
    run_id INTEGER NOT NULL REFERENCES status_run(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK(sequence >= 0),
    observed_unix_millis INTEGER NOT NULL,
    monotonic_nanos INTEGER NOT NULL CHECK(monotonic_nanos >= 0),
    phase TEXT CHECK(phase IS NULL OR phase IN
        ('discovering', 'candidate_screening', 'full_hashing', 'persisting',
         'analyzing_folders', 'finalizing')),
    process_cpu_nanos INTEGER CHECK(process_cpu_nanos IS NULL OR process_cpu_nanos >= 0),
    process_private_bytes INTEGER CHECK(process_private_bytes IS NULL OR process_private_bytes >= 0),
    process_working_set_bytes INTEGER CHECK(process_working_set_bytes IS NULL OR process_working_set_bytes >= 0),
    process_peak_working_set_bytes INTEGER CHECK(process_peak_working_set_bytes IS NULL OR process_peak_working_set_bytes >= 0),
    process_read_operations INTEGER CHECK(process_read_operations IS NULL OR process_read_operations >= 0),
    process_read_bytes INTEGER CHECK(process_read_bytes IS NULL OR process_read_bytes >= 0),
    process_write_operations INTEGER CHECK(process_write_operations IS NULL OR process_write_operations >= 0),
    process_write_bytes INTEGER CHECK(process_write_bytes IS NULL OR process_write_bytes >= 0),
    system_cpu_basis_points INTEGER CHECK(system_cpu_basis_points IS NULL OR system_cpu_basis_points BETWEEN 0 AND 10000),
    system_available_memory_bytes INTEGER CHECK(system_available_memory_bytes IS NULL OR system_available_memory_bytes >= 0),
    system_committed_memory_bytes INTEGER CHECK(system_committed_memory_bytes IS NULL OR system_committed_memory_bytes >= 0),
    unavailable_counter_count INTEGER NOT NULL DEFAULT 0 CHECK(unavailable_counter_count >= 0),
    PRIMARY KEY(run_id, sequence)
);

CREATE TABLE IF NOT EXISTS status_device_sample (
    run_id INTEGER NOT NULL REFERENCES status_run(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK(sequence >= 0),
    device_key TEXT NOT NULL CHECK(length(device_key) BETWEEN 1 AND 256),
    read_bytes_per_second INTEGER CHECK(read_bytes_per_second IS NULL OR read_bytes_per_second >= 0),
    read_iops_millis INTEGER CHECK(read_iops_millis IS NULL OR read_iops_millis >= 0),
    average_read_latency_micros INTEGER CHECK(average_read_latency_micros IS NULL OR average_read_latency_micros >= 0),
    active_millis_per_second INTEGER CHECK(active_millis_per_second IS NULL OR active_millis_per_second BETWEEN 0 AND 1000),
    queue_depth_millis INTEGER CHECK(queue_depth_millis IS NULL OR queue_depth_millis >= 0),
    unavailable_counter_count INTEGER NOT NULL DEFAULT 0 CHECK(unavailable_counter_count >= 0),
    PRIMARY KEY(run_id, sequence, device_key),
    FOREIGN KEY(run_id, sequence) REFERENCES status_host_sample(run_id, sequence) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_status_run_product_run
    ON status_run(product_run_id, id);
CREATE INDEX IF NOT EXISTS idx_status_run_input_device_comparison
    ON status_run(input_signature, engine_version, id DESC);
CREATE INDEX IF NOT EXISTS idx_status_host_sample_run_time
    ON status_host_sample(run_id, monotonic_nanos, sequence);
CREATE INDEX IF NOT EXISTS idx_status_device_sample_run_device_time
    ON status_device_sample(run_id, device_key, sequence);
