using Microsoft.Data.Sqlite;
using SuperDuper.Models;
using System.Text.Json;
using static SuperDuper.Models.FileTypeFilters;

namespace SuperDuper.Services;

/// <summary>
/// Owns all C#-side SQLite reads and UI-managed tables (review_decisions, undo_log, scan_profiles).
/// Opens the same super_duper.db that Rust writes to — WAL mode makes this safe.
/// Uses PRAGMA busy_timeout=5000 to tolerate Rust write phases.
/// </summary>
public class DatabaseService : IDatabaseService, IDisposable
{
    private readonly SqliteConnection _conn;
    private readonly SemaphoreSlim _lock = new(1, 1);

    public DatabaseService(string dbPath)
    {
        _conn = new SqliteConnection($"Data Source={dbPath}");
        _conn.Open();
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = "PRAGMA busy_timeout=5000; PRAGMA journal_mode=WAL;";
        cmd.ExecuteNonQuery();
    }

    public async Task EnsureSchemaAsync()
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                CREATE TABLE IF NOT EXISTS review_decisions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_id INTEGER NOT NULL UNIQUE,
                    group_id INTEGER NOT NULL,
                    action TEXT NOT NULL,
                    decided_at TEXT NOT NULL,
                    session_id INTEGER
                );

                CREATE TABLE IF NOT EXISTS undo_log (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    action_type TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    reversed INTEGER DEFAULT 0
                );

                CREATE INDEX IF NOT EXISTS idx_review_group ON review_decisions(group_id);
                CREATE INDEX IF NOT EXISTS idx_review_action ON review_decisions(action);

                CREATE TABLE IF NOT EXISTS scan_profiles (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    data TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                """;
            await cmd.ExecuteNonQueryAsync();

            // Migration: copy unexecuted deletion_plan rows into review_decisions
            cmd.CommandText = """
                INSERT OR IGNORE INTO review_decisions (file_id, group_id, action, decided_at, session_id)
                SELECT dp.file_id,
                       COALESCE(dgm.group_id, 0),
                       'delete',
                       dp.marked_at,
                       sf.last_seen_session_id
                FROM deletion_plan dp
                LEFT JOIN duplicate_group_member dgm ON dgm.file_id = dp.file_id
                LEFT JOIN scanned_file sf ON sf.id = dp.file_id
                WHERE dp.executed_at IS NULL;
                """;
            await cmd.ExecuteNonQueryAsync();
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task UpsertDecisionAsync(long fileId, long groupId, ReviewAction action, long? sessionId = null)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                INSERT INTO review_decisions (file_id, group_id, action, decided_at, session_id)
                VALUES ($fileId, $groupId, $action, $now, $sessionId)
                ON CONFLICT(file_id) DO UPDATE SET
                    action = excluded.action,
                    decided_at = excluded.decided_at,
                    session_id = excluded.session_id;
                """;
            cmd.Parameters.AddWithValue("$fileId", fileId);
            cmd.Parameters.AddWithValue("$groupId", groupId);
            cmd.Parameters.AddWithValue("$action", action.ToString().ToLowerInvariant());
            cmd.Parameters.AddWithValue("$now", DateTime.UtcNow.ToString("O"));
            cmd.Parameters.AddWithValue("$sessionId", (object?)sessionId ?? DBNull.Value);
            await cmd.ExecuteNonQueryAsync();
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<ReviewAction?> GetDecisionAsync(long fileId)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = "SELECT action FROM review_decisions WHERE file_id = $fileId";
            cmd.Parameters.AddWithValue("$fileId", fileId);
            var result = await cmd.ExecuteScalarAsync();
            if (result is string s)
                return Enum.TryParse<ReviewAction>(s, true, out var a) ? a : null;
            return null;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<ReviewStatus> GetGroupReviewStatusAsync(long groupId)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    COUNT(dgm.file_id) AS total,
                    COUNT(rd.file_id) AS reviewed
                FROM duplicate_group_member dgm
                LEFT JOIN review_decisions rd ON rd.file_id = dgm.file_id
                WHERE dgm.group_id = $groupId
                """;
            cmd.Parameters.AddWithValue("$groupId", groupId);
            using var reader = await cmd.ExecuteReaderAsync();
            if (await reader.ReadAsync())
            {
                var total = reader.GetInt32(0);
                var reviewed = reader.GetInt32(1);
                if (reviewed == 0) return ReviewStatus.Unreviewed;
                if (reviewed >= total) return ReviewStatus.Decided;
                return ReviewStatus.Partial;
            }
            return ReviewStatus.Unreviewed;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<(int Reviewed, int Total)> GetReviewProgressAsync(long sessionId)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    COUNT(DISTINCT dgm.file_id) AS total,
                    COUNT(DISTINCT rd.file_id) AS reviewed
                FROM duplicate_group dg
                JOIN duplicate_group_member dgm ON dgm.group_id = dg.id
                LEFT JOIN review_decisions rd ON rd.file_id = dgm.file_id
                WHERE dg.session_id = $sessionId
                """;
            cmd.Parameters.AddWithValue("$sessionId", sessionId);
            using var reader = await cmd.ExecuteReaderAsync();
            if (await reader.ReadAsync())
                return (reader.GetInt32(1), reader.GetInt32(0));
            return (0, 0);
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<IReadOnlyList<FileForDeletion>> GetDeletionQueueAsync()
    {
        await _lock.WaitAsync();
        try
        {
            var results = new List<FileForDeletion>();
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    rd.file_id,
                    sf.canonical_path,
                    sf.file_name,
                    sf.parent_dir,
                    sf.drive_letter,
                    sf.file_size,
                    sf.content_hash,
                    (
                        SELECT sf2.canonical_path
                        FROM duplicate_group_member dgm2
                        JOIN scanned_file sf2 ON sf2.id = dgm2.file_id
                        LEFT JOIN review_decisions rd2 ON rd2.file_id = dgm2.file_id
                        WHERE dgm2.group_id = rd.group_id
                          AND dgm2.file_id != rd.file_id
                          AND (rd2.action = 'keep' OR rd2.action IS NULL)
                        LIMIT 1
                    ) AS retained_copy_path
                FROM review_decisions rd
                JOIN scanned_file sf ON sf.id = rd.file_id
                WHERE rd.action = 'delete'
                ORDER BY sf.drive_letter, sf.parent_dir, sf.file_name
                """;
            using var reader = await cmd.ExecuteReaderAsync();
            while (await reader.ReadAsync())
            {
                results.Add(new FileForDeletion(
                    reader.GetInt64(0),
                    reader.GetString(1),
                    reader.GetString(2),
                    reader.GetString(3),
                    reader.IsDBNull(4) ? "" : reader.GetString(4),
                    reader.GetInt64(5),
                    reader.IsDBNull(6) ? 0 : reader.GetInt64(6),
                    reader.IsDBNull(7) ? null : reader.GetString(7)
                ));
            }
            return results;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<int> GetDeletionQueueCountAsync()
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = "SELECT COUNT(*) FROM review_decisions WHERE action = 'delete'";
            var result = await cmd.ExecuteScalarAsync();
            return result is long l ? (int)l : 0;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<IReadOnlyList<DbFileInfo>> QueryFilesInGroupAsync(long groupId)
    {
        await _lock.WaitAsync();
        try
        {
            var items = new List<DbFileInfo>();
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    sf.id, sf.canonical_path, sf.file_name, sf.parent_dir,
                    sf.drive_letter, sf.file_size, sf.last_modified,
                    sf.partial_hash, sf.content_hash,
                    1 AS is_duplicate,
                    cnt.copy_count,
                    dgm.group_id
                FROM duplicate_group_member dgm
                JOIN scanned_file sf ON sf.id = dgm.file_id
                JOIN (
                    SELECT group_id, COUNT(*) AS copy_count
                    FROM duplicate_group_member
                    WHERE group_id = $groupId
                ) cnt ON cnt.group_id = dgm.group_id
                WHERE dgm.group_id = $groupId
                ORDER BY sf.last_modified DESC, sf.canonical_path
                """;
            cmd.Parameters.AddWithValue("$groupId", groupId);

            using var reader = await cmd.ExecuteReaderAsync();
            while (await reader.ReadAsync())
            {
                items.Add(new DbFileInfo
                {
                    FileId = reader.GetInt64(0),
                    CanonicalPath = reader.GetString(1),
                    FileName = reader.GetString(2),
                    ParentDir = reader.GetString(3),
                    DriveLetter = reader.IsDBNull(4) ? "" : reader.GetString(4),
                    FileSize = reader.GetInt64(5),
                    LastModified = reader.IsDBNull(6) ? "" : reader.GetString(6),
                    PartialHash = reader.IsDBNull(7) ? 0 : reader.GetInt64(7),
                    ContentHash = reader.IsDBNull(8) ? 0 : reader.GetInt64(8),
                    IsDuplicate = true,
                    CopyCount = reader.GetInt32(10),
                    GroupId = reader.GetInt64(11)
                });
            }
            return items;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<PagedResult<DbFileInfo>> QueryFilesInDirectoryAsync(
        string dirPath, long sessionId, int offset, int limit,
        string sortColumn = "file_name", bool ascending = true)
    {
        await _lock.WaitAsync();
        try
        {
            var direction = ascending ? "ASC" : "DESC";
            var safeSort = sortColumn switch
            {
                "file_name" => "sf.file_name",
                "file_size" => "sf.file_size",
                "last_modified" => "sf.last_modified",
                _ => "sf.file_name"
            };

            using var countCmd = _conn.CreateCommand();
            countCmd.CommandText = """
                SELECT COUNT(*) FROM scanned_file sf
                WHERE sf.parent_dir = $dir AND sf.last_seen_session_id = $sessionId
                """;
            countCmd.Parameters.AddWithValue("$dir", dirPath);
            countCmd.Parameters.AddWithValue("$sessionId", sessionId);
            var totalObj = await countCmd.ExecuteScalarAsync();
            var total = totalObj is long l ? (int)l : 0;

            var items = new List<DbFileInfo>();
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = $"""
                SELECT
                    sf.id, sf.canonical_path, sf.file_name, sf.parent_dir,
                    sf.drive_letter, sf.file_size, sf.last_modified,
                    sf.partial_hash, sf.content_hash,
                    CASE WHEN dgm.file_id IS NOT NULL THEN 1 ELSE 0 END AS is_duplicate,
                    COALESCE(cnt.copy_count, 0) AS copy_count,
                    COALESCE(dgm.group_id, 0) AS group_id
                FROM scanned_file sf
                LEFT JOIN duplicate_group_member dgm ON dgm.file_id = sf.id
                LEFT JOIN (
                    SELECT dgm2.group_id, COUNT(*) AS copy_count
                    FROM duplicate_group_member dgm2
                    GROUP BY dgm2.group_id
                ) cnt ON cnt.group_id = dgm.group_id
                WHERE sf.parent_dir = $dir AND sf.last_seen_session_id = $sessionId
                ORDER BY {safeSort} {direction}
                LIMIT $limit OFFSET $offset
                """;
            cmd.Parameters.AddWithValue("$dir", dirPath);
            cmd.Parameters.AddWithValue("$sessionId", sessionId);
            cmd.Parameters.AddWithValue("$limit", limit);
            cmd.Parameters.AddWithValue("$offset", offset);

            using var reader = await cmd.ExecuteReaderAsync();
            while (await reader.ReadAsync())
            {
                items.Add(new DbFileInfo
                {
                    FileId = reader.GetInt64(0),
                    CanonicalPath = reader.GetString(1),
                    FileName = reader.GetString(2),
                    ParentDir = reader.GetString(3),
                    DriveLetter = reader.IsDBNull(4) ? "" : reader.GetString(4),
                    FileSize = reader.GetInt64(5),
                    LastModified = reader.IsDBNull(6) ? "" : reader.GetString(6),
                    PartialHash = reader.IsDBNull(7) ? 0 : reader.GetInt64(7),
                    ContentHash = reader.IsDBNull(8) ? 0 : reader.GetInt64(8),
                    IsDuplicate = reader.GetInt32(9) == 1,
                    CopyCount = reader.GetInt32(10),
                    GroupId = reader.GetInt64(11)
                });
            }

            return new PagedResult<DbFileInfo>(items, total);
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<(int DupeCount, int TotalCount)> GetDirectoryDensityAsync(string dirPath, long sessionId)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    COUNT(sf.id) AS total,
                    COUNT(dgm.file_id) AS dupes
                FROM scanned_file sf
                LEFT JOIN duplicate_group_member dgm ON dgm.file_id = sf.id
                WHERE sf.parent_dir = $dir AND sf.last_seen_session_id = $sessionId
                """;
            cmd.Parameters.AddWithValue("$dir", dirPath);
            cmd.Parameters.AddWithValue("$sessionId", sessionId);
            using var reader = await cmd.ExecuteReaderAsync();
            if (await reader.ReadAsync())
                return (reader.GetInt32(1), reader.GetInt32(0));
            return (0, 0);
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<SiblingInfo> GetSiblingContextAsync(long fileId)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    COUNT(sf2.id) AS total_siblings,
                    COUNT(dgm2.file_id) AS duped_siblings
                FROM scanned_file sf
                JOIN scanned_file sf2 ON sf2.parent_dir = sf.parent_dir
                    AND sf2.last_seen_session_id = sf.last_seen_session_id
                    AND sf2.id != sf.id
                LEFT JOIN duplicate_group_member dgm2 ON dgm2.file_id = sf2.id
                WHERE sf.id = $fileId
                """;
            cmd.Parameters.AddWithValue("$fileId", fileId);
            using var reader = await cmd.ExecuteReaderAsync();
            if (await reader.ReadAsync())
                return new SiblingInfo(reader.GetInt32(0), reader.GetInt32(1));
            return new SiblingInfo(0, 0);
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<IReadOnlyList<QuickWinItem>> GetQuickWinsAsync(long sessionId)
    {
        await _lock.WaitAsync();
        try
        {
            var wins = new List<QuickWinItem>();

            // 1. Identical directories — biggest savings from exact matches
            using (var cmd = _conn.CreateCommand())
            {
                cmd.CommandText = """
                    SELECT dn_a.path, dn_b.path, ds.shared_bytes
                    FROM directory_similarity ds
                    JOIN directory_node dn_a ON dn_a.id = ds.dir_a_id
                    JOIN directory_node dn_b ON dn_b.id = ds.dir_b_id
                    WHERE ds.match_type = 'exact'
                    ORDER BY ds.shared_bytes DESC
                    LIMIT 5
                    """;
                using var reader = await cmd.ExecuteReaderAsync();
                while (await reader.ReadAsync())
                {
                    wins.Add(new QuickWinItem(
                        "Identical Directories",
                        $"{reader.GetString(0)} ↔ {reader.GetString(1)}",
                        reader.GetInt64(2),
                        1,
                        null
                    ));
                }
            }

            // 2. Largest duplicate groups by wasted bytes
            using (var cmd = _conn.CreateCommand())
            {
                cmd.CommandText = """
                    SELECT dg.id, dg.wasted_bytes, dg.file_count, MIN(sf.file_name) AS sample_name
                    FROM duplicate_group dg
                    JOIN duplicate_group_member dgm ON dgm.group_id = dg.id
                    JOIN scanned_file sf ON sf.id = dgm.file_id
                    WHERE dg.session_id = $sessionId
                    GROUP BY dg.id
                    ORDER BY dg.wasted_bytes DESC
                    LIMIT 5
                    """;
                cmd.Parameters.AddWithValue("$sessionId", sessionId);
                using var reader = await cmd.ExecuteReaderAsync();
                while (await reader.ReadAsync())
                {
                    wins.Add(new QuickWinItem(
                        "Largest Duplicate Groups",
                        $"{reader.GetString(3)} ({reader.GetInt64(2)} copies)",
                        reader.GetInt64(1),
                        (int)reader.GetInt64(2),
                        reader.GetInt64(0)
                    ));
                }
            }

            return wins;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<IReadOnlyList<DbFileInfo>> SearchFilesAsync(long sessionId, string query, int limit = 20)
    {
        await _lock.WaitAsync();
        try
        {
            var items = new List<DbFileInfo>();
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    sf.id, sf.canonical_path, sf.file_name, sf.parent_dir,
                    sf.drive_letter, sf.file_size, sf.last_modified,
                    sf.partial_hash, sf.content_hash,
                    CASE WHEN dgm.file_id IS NOT NULL THEN 1 ELSE 0 END AS is_duplicate,
                    COALESCE(cnt.copy_count, 0) AS copy_count,
                    COALESCE(dgm.group_id, 0) AS group_id
                FROM scanned_file sf
                LEFT JOIN duplicate_group_member dgm ON dgm.file_id = sf.id
                LEFT JOIN (
                    SELECT dgm2.group_id, COUNT(*) AS copy_count
                    FROM duplicate_group_member dgm2
                    GROUP BY dgm2.group_id
                ) cnt ON cnt.group_id = dgm.group_id
                WHERE (sf.file_name LIKE $query OR sf.canonical_path LIKE $query)
                ORDER BY sf.file_name
                LIMIT $limit
                """;
            cmd.Parameters.AddWithValue("$query", $"%{query}%");
            cmd.Parameters.AddWithValue("$limit", limit);

            using var reader = await cmd.ExecuteReaderAsync();
            while (await reader.ReadAsync())
            {
                items.Add(new DbFileInfo
                {
                    FileId = reader.GetInt64(0),
                    CanonicalPath = reader.GetString(1),
                    FileName = reader.GetString(2),
                    ParentDir = reader.GetString(3),
                    DriveLetter = reader.IsDBNull(4) ? "" : reader.GetString(4),
                    FileSize = reader.GetInt64(5),
                    LastModified = reader.IsDBNull(6) ? "" : reader.GetString(6),
                    PartialHash = reader.IsDBNull(7) ? 0 : reader.GetInt64(7),
                    ContentHash = reader.IsDBNull(8) ? 0 : reader.GetInt64(8),
                    IsDuplicate = reader.GetInt32(9) == 1,
                    CopyCount = reader.GetInt32(10),
                    GroupId = reader.GetInt64(11)
                });
            }
            return items;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<IReadOnlyList<TreemapNode>> GetTreemapNodesAsync(long sessionId)
    {
        await _lock.WaitAsync();
        try
        {
            var nodes = new List<TreemapNode>();
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT
                    sf.parent_dir,
                    COUNT(sf.id) AS total_files,
                    COUNT(dgm.file_id) AS dupe_files,
                    SUM(sf.file_size) AS total_bytes
                FROM scanned_file sf
                LEFT JOIN duplicate_group_member dgm ON dgm.file_id = sf.id
                WHERE sf.last_seen_session_id = $sessionId
                GROUP BY sf.parent_dir
                HAVING total_bytes > 0
                ORDER BY total_bytes DESC
                LIMIT 30
                """;
            cmd.Parameters.AddWithValue("$sessionId", sessionId);
            using var reader = await cmd.ExecuteReaderAsync();
            while (await reader.ReadAsync())
            {
                var path = reader.GetString(0);
                var totalFiles = reader.GetInt32(1);
                var dupeFiles = reader.GetInt32(2);
                var totalBytes = reader.GetInt64(3);
                nodes.Add(new TreemapNode
                {
                    Path = path,
                    DisplayName = Path.GetFileName(path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar))
                                  is { Length: > 0 } name ? name : path,
                    TotalBytes = totalBytes,
                    DupeDensity = totalFiles > 0 ? (double)dupeFiles / totalFiles : 0,
                    DupeCount = dupeFiles,
                    TotalCount = totalFiles
                });
            }
            return nodes;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<PagedResult<DbGroupInfo>> QueryGroupsFilteredAsync(
        long sessionId, GroupFilterOptions filter, string sortColumn = "wasted_bytes",
        bool ascending = false, int offset = 0, int limit = 50)
    {
        await _lock.WaitAsync();
        try
        {
            var safeSort = sortColumn switch
            {
                "wasted_bytes" => "dg.wasted_bytes",
                "file_size" => "dg.file_size",
                "file_count" => "dg.file_count",
                "file_name" => "sample_name",
                _ => "dg.wasted_bytes"
            };
            var direction = ascending ? "ASC" : "DESC";

            var whereClauses = new List<string> { "dg.session_id = $sessionId" };
            if (filter.TextSearch is { Length: > 0 })
                whereClauses.Add("EXISTS (SELECT 1 FROM duplicate_group_member m JOIN scanned_file f ON f.id = m.file_id WHERE m.group_id = dg.id AND f.file_name LIKE $textSearch)");
            if (filter.FileTypeFilter is { Length: > 0 })
            {
                var extensions = filter.FileTypeFilter switch
                {
                    Images => "('jpg','jpeg','png','gif','bmp','webp','svg','tiff')",
                    Documents => "('pdf','doc','docx','txt','rtf','odt','xls','xlsx','csv','pptx')",
                    Video => "('mp4','avi','mkv','mov','wmv','flv','webm')",
                    Audio => "('mp3','flac','wav','aac','ogg','wma','m4a')",
                    Archives => "('zip','rar','7z','gz','tar','bz2')",
                    _ => "('')"
                };
                whereClauses.Add($"EXISTS (SELECT 1 FROM duplicate_group_member m JOIN scanned_file f ON f.id = m.file_id WHERE m.group_id = dg.id AND LOWER(REPLACE(f.file_name, RTRIM(f.file_name, REPLACE(f.file_name, '.', '')), '')) IN {extensions})");
            }
            if (filter.DriveFilter is { Length: > 0 })
                whereClauses.Add("EXISTS (SELECT 1 FROM duplicate_group_member m JOIN scanned_file f ON f.id = m.file_id WHERE m.group_id = dg.id AND f.drive_letter = $driveFilter)");
            if (filter.ReviewStatusFilter.HasValue)
            {
                var statusSql = filter.ReviewStatusFilter.Value switch
                {
                    ReviewStatus.Unreviewed => "NOT EXISTS (SELECT 1 FROM duplicate_group_member m LEFT JOIN review_decisions rd ON rd.file_id = m.file_id WHERE m.group_id = dg.id AND rd.file_id IS NOT NULL)",
                    ReviewStatus.Decided => "NOT EXISTS (SELECT 1 FROM duplicate_group_member m LEFT JOIN review_decisions rd ON rd.file_id = m.file_id WHERE m.group_id = dg.id AND rd.file_id IS NULL)",
                    _ => "(SELECT COUNT(rd.file_id) FROM duplicate_group_member m LEFT JOIN review_decisions rd ON rd.file_id = m.file_id WHERE m.group_id = dg.id AND rd.file_id IS NOT NULL) > 0 AND EXISTS (SELECT 1 FROM duplicate_group_member m LEFT JOIN review_decisions rd ON rd.file_id = m.file_id WHERE m.group_id = dg.id AND rd.file_id IS NULL)"
                };
                whereClauses.Add(statusSql);
            }
            if (filter.MinWastedBytes.HasValue)
                whereClauses.Add("dg.wasted_bytes >= $minWaste");
            if (filter.MaxWastedBytes.HasValue)
                whereClauses.Add("dg.wasted_bytes <= $maxWaste");

            var where = string.Join(" AND ", whereClauses);

            using var countCmd = _conn.CreateCommand();
            countCmd.CommandText = $"SELECT COUNT(*) FROM duplicate_group dg WHERE {where}";
            countCmd.Parameters.AddWithValue("$sessionId", sessionId);
            if (filter.TextSearch is { Length: > 0 })
                countCmd.Parameters.AddWithValue("$textSearch", $"%{filter.TextSearch}%");
            if (filter.DriveFilter is { Length: > 0 })
                countCmd.Parameters.AddWithValue("$driveFilter", filter.DriveFilter);
            if (filter.MinWastedBytes.HasValue)
                countCmd.Parameters.AddWithValue("$minWaste", filter.MinWastedBytes.Value);
            if (filter.MaxWastedBytes.HasValue)
                countCmd.Parameters.AddWithValue("$maxWaste", filter.MaxWastedBytes.Value);

            var totalObj = await countCmd.ExecuteScalarAsync();
            var total = totalObj is long l ? (int)l : 0;

            var items = new List<DbGroupInfo>();
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = $"""
                SELECT dg.id, dg.content_hash, dg.file_size, dg.file_count, dg.wasted_bytes,
                       COALESCE(MIN(sf.file_name), '') AS sample_name
                FROM duplicate_group dg
                LEFT JOIN duplicate_group_member dgm ON dgm.group_id = dg.id
                LEFT JOIN scanned_file sf ON sf.id = dgm.file_id
                WHERE {where}
                GROUP BY dg.id
                ORDER BY {safeSort} {direction}
                LIMIT $limit OFFSET $offset
                """;
            cmd.Parameters.AddWithValue("$sessionId", sessionId);
            cmd.Parameters.AddWithValue("$limit", limit);
            cmd.Parameters.AddWithValue("$offset", offset);
            if (filter.TextSearch is { Length: > 0 })
                cmd.Parameters.AddWithValue("$textSearch", $"%{filter.TextSearch}%");
            if (filter.DriveFilter is { Length: > 0 })
                cmd.Parameters.AddWithValue("$driveFilter", filter.DriveFilter);
            if (filter.MinWastedBytes.HasValue)
                cmd.Parameters.AddWithValue("$minWaste", filter.MinWastedBytes.Value);
            if (filter.MaxWastedBytes.HasValue)
                cmd.Parameters.AddWithValue("$maxWaste", filter.MaxWastedBytes.Value);

            using var reader = await cmd.ExecuteReaderAsync();
            while (await reader.ReadAsync())
            {
                items.Add(new DbGroupInfo
                {
                    GroupId = reader.GetInt64(0),
                    ContentHash = reader.GetInt64(1),
                    FileSize = reader.GetInt64(2),
                    FileCount = (int)reader.GetInt64(3),
                    WastedBytes = reader.GetInt64(4),
                    SampleFileName = reader.GetString(5)
                });
            }

            return new PagedResult<DbGroupInfo>(items, total);
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<(int GroupCount, long WastedBytes)> GetSessionMetricsAsync(long sessionId)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                SELECT COUNT(*), COALESCE(SUM(wasted_bytes), 0)
                FROM duplicate_group
                WHERE session_id = $sessionId
                """;
            cmd.Parameters.AddWithValue("$sessionId", sessionId);
            using var reader = await cmd.ExecuteReaderAsync();
            if (await reader.ReadAsync())
                return (reader.GetInt32(0), reader.GetInt64(1));
            return (0, 0);
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task<IReadOnlyList<ScanProfile>> GetSavedProfilesAsync()
    {
        await _lock.WaitAsync();
        try
        {
            var results = new List<ScanProfile>();
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = "SELECT data FROM scan_profiles ORDER BY updated_at DESC";
            using var reader = await cmd.ExecuteReaderAsync();
            while (await reader.ReadAsync())
            {
                var profile = JsonSerializer.Deserialize<ScanProfile>(reader.GetString(0));
                if (profile != null) results.Add(profile);
            }
            return results;
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task UpsertProfileAsync(ScanProfile profile)
    {
        profile.UpdatedAt = DateTime.UtcNow;
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = """
                INSERT INTO scan_profiles (id, name, data, updated_at)
                VALUES ($id, $name, $data, $updatedAt)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    data = excluded.data,
                    updated_at = excluded.updated_at;
                """;
            cmd.Parameters.AddWithValue("$id", profile.Id);
            cmd.Parameters.AddWithValue("$name", profile.Name);
            cmd.Parameters.AddWithValue("$data", JsonSerializer.Serialize(profile));
            cmd.Parameters.AddWithValue("$updatedAt", profile.UpdatedAt.ToString("O"));
            await cmd.ExecuteNonQueryAsync();
        }
        finally
        {
            _lock.Release();
        }
    }

    public async Task DeleteProfileAsync(string profileId)
    {
        await _lock.WaitAsync();
        try
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = "DELETE FROM scan_profiles WHERE id = $id";
            cmd.Parameters.AddWithValue("$id", profileId);
            await cmd.ExecuteNonQueryAsync();
        }
        finally
        {
            _lock.Release();
        }
    }

    public void Dispose()
    {
        _conn.Dispose();
        _lock.Dispose();
    }
}
