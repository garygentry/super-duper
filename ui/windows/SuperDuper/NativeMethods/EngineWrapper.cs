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

    public void SetScanPaths(string[] paths)
    {
        ThrowIfDisposed();
        var result = sd_engine_set_scan_paths(_handle, paths, (uint)paths.Length);
        ThrowOnError(result, "SetScanPaths");
    }

    public void StartScan()
    {
        ThrowIfDisposed();
        var result = sd_scan_start(_handle);
        ThrowOnError(result, "StartScan");
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
}
