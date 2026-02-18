using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using static SuperDuper.NativeMethods.SuperDuperEngine;

namespace SuperDuper.NativeMethods;

/// <summary>
/// Managed wrapper around the native super_duper_ffi engine.
/// Handles resource lifecycle and marshalling.
/// </summary>
public sealed class EngineWrapper : IDisposable
{
    private ulong _handle;
    private bool _disposed;

    public EngineWrapper(string dbPath = "super_duper.db")
    {
        _handle = sd_engine_create(dbPath);
        if (_handle == 0)
        {
            throw new InvalidOperationException(
                $"Failed to create engine: {GetLastError()}");
        }
    }

    /// <summary>
    /// Validates that the native FFI library can be loaded.
    /// Call once at app startup before creating any EngineWrapper instances.
    /// </summary>
    /// <returns>null if OK, or an error message string if the library failed to load.</returns>
    public static string? ValidateNativeLibrary()
    {
        try
        {
            // Call the simplest FFI function to verify the DLL loads
            var ptr = sd_last_error_message();
            if (ptr != IntPtr.Zero)
                sd_free_string(ptr);
            return null;
        }
        catch (DllNotFoundException ex)
        {
            return $"Native library 'super_duper_ffi' not found.\n\n{ex.Message}";
        }
        catch (EntryPointNotFoundException ex)
        {
            return $"Native library is incompatible (missing exports).\n\n{ex.Message}";
        }
        catch (BadImageFormatException ex)
        {
            return $"Native library architecture mismatch (32/64-bit).\n\n{ex.Message}";
        }
    }

    public void SetScanPaths(string[] paths)
    {
        ThrowIfDisposed();
        var (ptrs, handles) = MarshalUtf8StringArray(paths);
        try
        {
            var result = sd_engine_set_scan_paths(_handle, ptrs, (uint)ptrs.Length);
            ThrowOnError(result, "SetScanPaths");
        }
        finally { FreeUtf8StringArray(handles); }
    }

    public void SetIgnorePatterns(string[] patterns)
    {
        ThrowIfDisposed();
        var (ptrs, handles) = MarshalUtf8StringArray(patterns);
        try
        {
            var result = sd_engine_set_ignore_patterns(_handle, ptrs, (uint)ptrs.Length);
            ThrowOnError(result, "SetIgnorePatterns");
        }
        finally { FreeUtf8StringArray(handles); }
    }


    public void StartScan()
    {
        ThrowIfDisposed();
        var result = sd_scan_start(_handle);
        ThrowOnError(result, "StartScan");
    }

    public void CancelScan()
    {
        ThrowIfDisposed();
        var result = sd_scan_cancel(_handle);
        ThrowOnError(result, "CancelScan");
    }

    private SdProgressCallback? _progressCallbackRef;

    public void SetProgressCallback(SdProgressCallback callback)
    {
        ThrowIfDisposed();
        _progressCallbackRef = callback; // prevent GC
        var result = sd_set_progress_callback(_handle, callback);
        ThrowOnError(result, "SetProgressCallback");
    }

    public void ClearProgressCallback()
    {
        ThrowIfDisposed();
        var result = sd_clear_progress_callback(_handle);
        ThrowOnError(result, "ClearProgressCallback");
        _progressCallbackRef = null;
    }

    public bool IsScanRunning
    {
        get
        {
            ThrowIfDisposed();
            return sd_scan_is_running(_handle);
        }
    }

    public (List<DuplicateGroupInfo> Groups, int TotalAvailable) QueryDuplicateGroups(
        long offset = 0, long limit = 100)
    {
        ThrowIfDisposed();

        var result = sd_query_duplicate_groups(_handle, offset, limit, out var page);
        ThrowOnError(result, "QueryDuplicateGroups");

        var groups = new List<DuplicateGroupInfo>((int)page.Count);
        try
        {
            for (int i = 0; i < page.Count; i++)
            {
                var ptr = page.Groups + i * Marshal.SizeOf<SdDuplicateGroup>();
                var native = Marshal.PtrToStructure<SdDuplicateGroup>(ptr);
                groups.Add(new DuplicateGroupInfo
                {
                    Id = native.Id,
                    ContentHash = native.ContentHash,
                    FileSize = native.FileSize,
                    FileCount = native.FileCount,
                    WastedBytes = native.WastedBytes,
                });
            }
        }
        finally
        {
            sd_free_duplicate_group_page(ref page);
        }

        return (groups, (int)page.TotalAvailable);
    }

    public List<FileInfo> QueryFilesInGroup(long groupId)
    {
        ThrowIfDisposed();

        var result = sd_query_files_in_group(_handle, groupId, out var page);
        ThrowOnError(result, "QueryFilesInGroup");

        var files = new List<FileInfo>((int)page.Count);
        try
        {
            for (int i = 0; i < page.Count; i++)
            {
                var ptr = page.Files + i * Marshal.SizeOf<SdFileRecord>();
                var native = Marshal.PtrToStructure<SdFileRecord>(ptr);
                files.Add(new FileInfo
                {
                    Id = native.Id,
                    CanonicalPath = Marshal.PtrToStringUTF8(native.CanonicalPath) ?? "",
                    FileName = Marshal.PtrToStringUTF8(native.FileName) ?? "",
                    ParentDir = Marshal.PtrToStringUTF8(native.ParentDir) ?? "",
                    FileSize = native.FileSize,
                    ContentHash = native.ContentHash,
                    IsMarkedForDeletion = native.IsMarkedForDeletion != 0,
                });
            }
        }
        finally
        {
            sd_free_file_record_page(ref page);
        }

        return files;
    }

    public void MarkForDeletion(long fileId)
    {
        ThrowIfDisposed();
        var result = sd_mark_file_for_deletion(_handle, fileId);
        ThrowOnError(result, "MarkForDeletion");
    }

    public void UnmarkForDeletion(long fileId)
    {
        ThrowIfDisposed();
        var result = sd_unmark_file_for_deletion(_handle, fileId);
        ThrowOnError(result, "UnmarkForDeletion");
    }

    public List<DirectoryNodeInfo> QueryDirectoryChildren(long parentId, long offset = 0, long limit = 100)
    {
        ThrowIfDisposed();

        var result = sd_query_directory_children(_handle, parentId, offset, limit, out var page);
        ThrowOnError(result, "QueryDirectoryChildren");

        var nodes = new List<DirectoryNodeInfo>((int)page.Count);
        try
        {
            for (int i = 0; i < page.Count; i++)
            {
                var ptr = page.Nodes + i * Marshal.SizeOf<SdDirectoryNode>();
                var native = Marshal.PtrToStructure<SdDirectoryNode>(ptr);
                nodes.Add(new DirectoryNodeInfo
                {
                    Id = native.Id,
                    Path = Marshal.PtrToStringUTF8(native.Path) ?? "",
                    Name = Marshal.PtrToStringUTF8(native.Name) ?? "",
                    ParentId = native.ParentId,
                    TotalSize = native.TotalSize,
                    FileCount = native.FileCount,
                    Depth = native.Depth,
                });
            }
        }
        finally
        {
            sd_free_directory_node_page(ref page);
        }

        return nodes;
    }

    public List<DirectorySimilarityInfo> QuerySimilarDirectories(double minScore = 0.5, long offset = 0, long limit = 100)
    {
        ThrowIfDisposed();

        var result = sd_query_similar_directories(_handle, minScore, offset, limit, out var page);
        ThrowOnError(result, "QuerySimilarDirectories");

        var pairs = new List<DirectorySimilarityInfo>((int)page.Count);
        try
        {
            for (int i = 0; i < page.Count; i++)
            {
                var ptr = page.Pairs + i * Marshal.SizeOf<SdDirectorySimilarity>();
                var native = Marshal.PtrToStructure<SdDirectorySimilarity>(ptr);
                pairs.Add(new DirectorySimilarityInfo
                {
                    Id = native.Id,
                    DirAId = native.DirAId,
                    DirBId = native.DirBId,
                    SimilarityScore = native.SimilarityScore,
                    SharedBytes = native.SharedBytes,
                    MatchType = Marshal.PtrToStringUTF8(native.MatchType) ?? "",
                });
            }
        }
        finally
        {
            sd_free_directory_similarity_page(ref page);
        }

        return pairs;
    }

    public void MarkDirectoryForDeletion(string directoryPath)
    {
        ThrowIfDisposed();
        var result = sd_mark_directory_for_deletion(_handle, directoryPath);
        ThrowOnError(result, "MarkDirectoryForDeletion");
    }

    public void AutoMarkForDeletion()
    {
        ThrowIfDisposed();
        var result = sd_auto_mark_for_deletion(_handle);
        ThrowOnError(result, "AutoMarkForDeletion");
    }

    public (uint SuccessCount, uint ErrorCount) ExecuteDeletionPlan()
    {
        ThrowIfDisposed();
        var result = sd_deletion_execute(_handle, out var deletionResult);
        ThrowOnError(result, "ExecuteDeletionPlan");
        return (deletionResult.SuccessCount, deletionResult.ErrorCount);
    }

    public (long FileCount, long TotalBytes) GetDeletionPlanSummary()
    {
        ThrowIfDisposed();
        var result = sd_deletion_plan_summary(_handle, out var count, out var bytes);
        ThrowOnError(result, "GetDeletionPlanSummary");
        return (count, bytes);
    }

    private void ThrowOnError(SdResultCode code, string operation)
    {
        if (code != SdResultCode.Ok)
        {
            var msg = GetLastError();
            throw new InvalidOperationException(
                $"{operation} failed with {code}: {msg}");
        }
    }

    private void ThrowIfDisposed()
    {
        if (_disposed) throw new ObjectDisposedException(nameof(EngineWrapper));
    }

    public void Dispose()
    {
        if (!_disposed)
        {
            if (_handle != 0)
            {
                sd_engine_destroy(_handle);
                _handle = 0;
            }
            _disposed = true;
        }
    }
}

// ── Managed DTOs ─────────────────────────────────────────────────

public class DuplicateGroupInfo
{
    public long Id { get; set; }
    public long ContentHash { get; set; }
    public long FileSize { get; set; }
    public long FileCount { get; set; }
    public long WastedBytes { get; set; }
}

public class FileInfo
{
    public long Id { get; set; }
    public string CanonicalPath { get; set; } = "";
    public string FileName { get; set; } = "";
    public string ParentDir { get; set; } = "";
    public long FileSize { get; set; }
    public long ContentHash { get; set; }
    public bool IsMarkedForDeletion { get; set; }
}

public class DirectoryNodeInfo
{
    public long Id { get; set; }
    public string Path { get; set; } = "";
    public string Name { get; set; } = "";
    public long ParentId { get; set; }
    public long TotalSize { get; set; }
    public long FileCount { get; set; }
    public long Depth { get; set; }
}

public class DirectorySimilarityInfo
{
    public long Id { get; set; }
    public long DirAId { get; set; }
    public long DirBId { get; set; }
    public double SimilarityScore { get; set; }
    public long SharedBytes { get; set; }
    public string MatchType { get; set; } = "";
}
